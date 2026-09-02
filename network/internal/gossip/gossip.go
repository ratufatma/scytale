// Package gossip implements the two-step inventory gossip protocol for transactions
// and blocks, including duplicate suppression and relay logic.
package gossip

import (
	"encoding/binary"
	"fmt"
	"sync"

	"github.com/scytale-network/scytale-p2p/internal/bridge"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// InvType identifies the kind of object in an inventory announcement.
type InvType uint8

const (
	InvTypeTx    InvType = 1
	InvTypeBlock InvType = 2
)

// InvItem is a single inventory announcement: type + 32-byte hash.
type InvItem struct {
	Type InvType
	Hash [32]byte
}

// EncodeInv serialises a slice of InvItems to wire bytes.
// Format: [ 1-byte count | (1-byte type + 32-byte hash) × N ]
func EncodeInv(items []InvItem) []byte {
	buf := make([]byte, 1+len(items)*33)
	buf[0] = byte(len(items))
	for i, item := range items {
		off := 1 + i*33
		buf[off] = byte(item.Type)
		copy(buf[off+1:off+33], item.Hash[:])
	}
	return buf
}

// DecodeInv deserialises inventory bytes from an `inv` or `getdata` payload.
func DecodeInv(data []byte) ([]InvItem, error) {
	if len(data) < 1 {
		return nil, fmt.Errorf("gossip: inv payload too short")
	}
	count := int(data[0])
	if len(data) < 1+count*33 {
		return nil, fmt.Errorf("gossip: inv payload truncated (expected %d items)", count)
	}
	items := make([]InvItem, count)
	for i := range items {
		off := 1 + i*33
		items[i].Type = InvType(data[off])
		copy(items[i].Hash[:], data[off+1:off+33])
	}
	return items, nil
}

// EncodeHashList serializes a list of 32-byte hashes for getblocks/invblocks.
// Format: [ 4-byte count LE | 32-byte hash × N ]
func EncodeHashList(hashes [][32]byte) []byte {
	buf := make([]byte, 4+len(hashes)*32)
	binary.LittleEndian.PutUint32(buf[0:4], uint32(len(hashes)))
	for i, h := range hashes {
		copy(buf[4+i*32:], h[:])
	}
	return buf
}

// DecodeHashList deserializes a list of 32-byte hashes from getblocks/invblocks.
func DecodeHashList(data []byte) ([][32]byte, error) {
	if len(data) < 4 {
		return nil, fmt.Errorf("gossip: hash list payload too short")
	}
	count := binary.LittleEndian.Uint32(data[0:4])
	if uint32(len(data)) < 4+count*32 {
		return nil, fmt.Errorf("gossip: hash list truncated")
	}
	hashes := make([][32]byte, count)
	for i := range hashes {
		copy(hashes[i][:], data[4+i*32:4+(i+1)*32])
	}
	return hashes, nil
}

// ─────────────────────────────────────────────────────────────────────────────
// Filter — duplicate suppression cache
// ─────────────────────────────────────────────────────────────────────────────

// Filter tracks recently seen TxID and BlockHash announcements to suppress
// duplicate getdata requests. It is safe for concurrent use.
type Filter struct {
	mu   sync.Mutex
	seen map[[32]byte]struct{}
}

// NewFilter creates an empty duplicate suppression filter.
func NewFilter() *Filter {
	return &Filter{seen: make(map[[32]byte]struct{})}
}

// MarkSeen records a hash as seen. Returns true if it was not previously seen.
func (f *Filter) MarkSeen(hash [32]byte) bool {
	f.mu.Lock()
	defer f.mu.Unlock()
	if _, ok := f.seen[hash]; ok {
		return false
	}
	f.seen[hash] = struct{}{}
	return true
}

// HasSeen returns true if the hash has been previously marked.
func (f *Filter) HasSeen(hash [32]byte) bool {
	f.mu.Lock()
	defer f.mu.Unlock()
	_, ok := f.seen[hash]
	return ok
}

// ─────────────────────────────────────────────────────────────────────────────
// Engine — two-step gossip coordinator
// ─────────────────────────────────────────────────────────────────────────────

// Sender abstracts the per-peer send operation used by the gossip Engine.
type Sender interface {
	Send(cmd string, payload []byte) error
}

// Engine coordinates the two-step inventory gossip flow for a single peer session.
// It uses a Filter for duplicate suppression and a ConsensusBridge for data retrieval.
type Engine struct {
	filter *Filter
	bridge bridge.ConsensusBridge
}

// NewEngine creates a gossip Engine with the provided filter and bridge.
func NewEngine(filter *Filter, b bridge.ConsensusBridge) *Engine {
	return &Engine{filter: filter, bridge: b}
}

// HandleInv processes an incoming `inv` message payload.
// For each unknown item, it sends a `getdata` request back to the sender.
func (e *Engine) HandleInv(sender Sender, invPayload []byte) error {
	items, err := DecodeInv(invPayload)
	if err != nil {
		return fmt.Errorf("gossip: decoding inv: %w", err)
	}

	var unknown []InvItem
	for _, item := range items {
		if e.filter.MarkSeen(item.Hash) {
			unknown = append(unknown, item)
		}
	}
	if len(unknown) == 0 {
		return nil // all already known — suppress getdata
	}

	return sender.Send(wire.CmdGetData, EncodeInv(unknown))
}

// HandleGetData processes an incoming `getdata` message, fetching raw object bytes
// from the ConsensusBridge and sending the appropriate `tx` or `block` response.
func (e *Engine) HandleGetData(sender Sender, getDataPayload []byte) error {
	items, err := DecodeInv(getDataPayload)
	if err != nil {
		return fmt.Errorf("gossip: decoding getdata: %w", err)
	}

	for _, item := range items {
		switch item.Type {
		case InvTypeTx:
			data, err := e.bridge.GetTransactionByHash(item.Hash)
			if err != nil {
				continue // object not available; skip without panic
			}
			if err := sender.Send(wire.CmdTx, data); err != nil {
				return err
			}
		case InvTypeBlock:
			data, err := e.bridge.GetBlockByHash(item.Hash)
			if err != nil {
				continue
			}
			if err := sender.Send(wire.CmdBlock, data); err != nil {
				return err
			}
		}
	}
	return nil
}

// HandleTx processes an incoming `tx` payload, forwarding to the Rust bridge.
func (e *Engine) HandleTx(txPayload []byte) error {
	return e.bridge.SubmitTransaction(txPayload)
}

// HandleBlock processes an incoming `block` payload, forwarding to the Rust bridge.
func (e *Engine) HandleBlock(blockPayload []byte) error {
	return e.bridge.SubmitBlock(blockPayload)
}
