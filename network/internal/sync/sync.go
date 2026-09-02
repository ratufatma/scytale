// Package sync implements the Initial Block Download (IBD) synchronization manager.
// It negotiates a common ancestor via block locator hashes, then downloads missing
// blocks in bounded batches with backpressure control.
package sync

import (
	"fmt"

	"github.com/scytale-network/scytale-p2p/internal/bridge"
	"github.com/scytale-network/scytale-p2p/internal/gossip"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// MaxBatchSize is the maximum number of blocks requested per IBD batch.
const MaxBatchSize = 50

// BlockSender is a minimal interface for sending wire messages to a peer during IBD.
type BlockSender interface {
	Send(cmd string, payload []byte) error
}

// Syncer manages the IBD block locator negotiation and batch download.
type Syncer struct {
	bridge bridge.ConsensusBridge
}

// New creates a Syncer backed by the given ConsensusBridge.
func New(b bridge.ConsensusBridge) *Syncer {
	return &Syncer{bridge: b}
}

// SendGetBlocks sends a `getblocks` request to a peer containing the local block locator.
// The peer responds with an `invblocks` listing hashes we are missing.
func (s *Syncer) SendGetBlocks(sender BlockSender) error {
	locator, err := s.bridge.GetBlockLocator()
	if err != nil {
		return fmt.Errorf("sync: fetching block locator: %w", err)
	}
	payload := gossip.EncodeHashList(locator)
	return sender.Send(wire.CmdGetBlocks, payload)
}

// HandleInvBlocks processes an `invblocks` response from a peer.
// It fetches up to MaxBatchSize block hashes and requests the data for each.
// All received block bytes are forwarded to the ConsensusBridge for validation.
func (s *Syncer) HandleInvBlocks(sender BlockSender, invBlocksPayload []byte) (int, error) {
	hashes, err := gossip.DecodeHashList(invBlocksPayload)
	if err != nil {
		return 0, fmt.Errorf("sync: decoding invblocks: %w", err)
	}

	// Apply batch size cap
	if len(hashes) > MaxBatchSize {
		hashes = hashes[:MaxBatchSize]
	}

	// Build a getdata request for all announced block hashes
	items := make([]gossip.InvItem, len(hashes))
	for i, h := range hashes {
		items[i] = gossip.InvItem{Type: gossip.InvTypeBlock, Hash: h}
	}
	if len(items) == 0 {
		return 0, nil
	}
	return len(items), sender.Send(wire.CmdGetData, gossip.EncodeInv(items))
}

// HandleGetBlocks processes an incoming `getblocks` request from a syncing peer.
// It builds an invblocks response listing the hashes the remote peer is missing.
// knownHashes is the set of block hashes this node currently has; peerLocator
// contains the peer's block locator (tip-to-genesis).
func HandleGetBlocks(sender BlockSender, knownHashes [][32]byte, _ [][32]byte) error {
	// Simple implementation: send all our known hashes as invblocks.
	// A production implementation would find the common ancestor and send only the delta.
	if len(knownHashes) == 0 {
		return nil
	}

	// Cap response to MaxBatchSize
	if len(knownHashes) > MaxBatchSize {
		knownHashes = knownHashes[:MaxBatchSize]
	}
	payload := gossip.EncodeHashList(knownHashes)
	return sender.Send(wire.CmdInvBlocks, payload)
}
