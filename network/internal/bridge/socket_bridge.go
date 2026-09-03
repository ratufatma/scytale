package bridge

import (
	"bufio"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"sync"
	"sync/atomic"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// SocketConsensusBridge connects to the Rust node supervisor over a Unix domain socket.
type SocketConsensusBridge struct {
	conn       net.Conn
	reader     *bufio.Reader
	mu         sync.Mutex
	writeMu    sync.Mutex
	reqID      uint64
	pending    map[uint64]chan *BridgeResponse
	eventCh    chan BroadcastEvent
	isClosed   bool
	closeCh    chan struct{}
}

// BroadcastEvent represents an asynchronous broadcast from Rust to the P2P layer.
type BroadcastEvent struct {
	Type     string // "BroadcastBlock", "BroadcastTransaction", or "ConnectPeer"
	Data     []byte
	Hash     [32]byte
	PeerAddr string
}

type bridgeMessage struct {
	Kind     string          `json:"kind"` // "Request", "Response", "Event"
	ID       uint64          `json:"id,omitempty"`
	Request  *bridgeRequest  `json:"request,omitempty"`
	Response *BridgeResponse `json:"response,omitempty"`
	Event    *bridgeEvent    `json:"event,omitempty"`
}

type bridgeRequest struct {
	Type    string      `json:"type"`
	Payload interface{} `json:"payload,omitempty"`
}

type BridgeResponse struct {
	Type    string          `json:"type"`
	Payload json.RawMessage `json:"payload,omitempty"`
}

type bridgeEvent struct {
	Type    string          `json:"type"`
	Payload json.RawMessage `json:"payload,omitempty"`
}

// NewSocketConsensusBridge connects to the supervisor's Unix domain socket.
func NewSocketConsensusBridge(socketPath string) (*SocketConsensusBridge, error) {
	conn, err := net.Dial("unix", socketPath)
	if err != nil {
		return nil, fmt.Errorf("bridge: failed to connect to unix socket %s: %w", socketPath, err)
	}

	b := &SocketConsensusBridge{
		conn:     conn,
		reader:   bufio.NewReader(conn),
		pending:  make(map[uint64]chan *BridgeResponse),
		eventCh:  make(chan BroadcastEvent, 2048),
		closeCh:  make(chan struct{}),
	}

	go b.readLoop()
	return b, nil
}

// Events returns the channel of incoming broadcast events from the Rust node.
func (b *SocketConsensusBridge) Events() <-chan BroadcastEvent {
	return b.eventCh
}

func (b *SocketConsensusBridge) readLoop() {
	defer func() {
		b.mu.Lock()
		b.isClosed = true
		for _, ch := range b.pending {
			close(ch)
		}
		b.pending = make(map[uint64]chan *BridgeResponse)
		b.mu.Unlock()
		_ = b.conn.Close()
		close(b.closeCh)
	}()

	for {
		line, err := b.reader.ReadBytes('\n')
		if err != nil {
			return
		}

		var msg bridgeMessage
		if err := json.Unmarshal(line, &msg); err != nil {
			continue
		}

		switch msg.Kind {
		case "Response":
			b.mu.Lock()
			ch, ok := b.pending[msg.ID]
			if ok {
				delete(b.pending, msg.ID)
				ch <- msg.Response
			}
			b.mu.Unlock()

		case "Event":
			if msg.Event != nil {
				b.handleEvent(msg.Event)
			}
		}
	}
}

func (b *SocketConsensusBridge) handleEvent(ev *bridgeEvent) {
	switch ev.Type {
	case "BroadcastBlock":
		var p struct {
			BlockHex string `json:"block_hex"`
			HashHex  string `json:"hash_hex"`
		}
		if err := json.Unmarshal(ev.Payload, &p); err == nil {
			blockBytes, _ := hex.DecodeString(p.BlockHex)
			hashBytes, _ := hex.DecodeString(p.HashHex)
			var h [32]byte
			copy(h[:], hashBytes)
			select {
			case b.eventCh <- BroadcastEvent{Type: "BroadcastBlock", Data: blockBytes, Hash: h}:
			default:
			}
		}

	case "BroadcastTransaction":
		var p struct {
			TxHex   string `json:"tx_hex"`
			TxidHex string `json:"txid_hex"`
		}
		if err := json.Unmarshal(ev.Payload, &p); err == nil {
			txBytes, _ := hex.DecodeString(p.TxHex)
			txidBytes, _ := hex.DecodeString(p.TxidHex)
			var h [32]byte
			copy(h[:], txidBytes)
			select {
			case b.eventCh <- BroadcastEvent{Type: "BroadcastTransaction", Data: txBytes, Hash: h}:
			default:
			}
		}

	case "ConnectPeer":
		var p struct {
			Addr string `json:"addr"`
		}
		if err := json.Unmarshal(ev.Payload, &p); err == nil {
			select {
			case b.eventCh <- BroadcastEvent{Type: "ConnectPeer", PeerAddr: p.Addr}:
			default:
			}
		}
	}
}

func (b *SocketConsensusBridge) call(reqType string, payload interface{}) (*BridgeResponse, error) {
	b.mu.Lock()
	if b.isClosed {
		b.mu.Unlock()
		return nil, errors.New("bridge: connection closed")
	}
	id := atomic.AddUint64(&b.reqID, 1)
	respCh := make(chan *BridgeResponse, 1)
	b.pending[id] = respCh
	b.mu.Unlock()

	msg := bridgeMessage{
		Kind: "Request",
		ID:   id,
		Request: &bridgeRequest{
			Type:    reqType,
			Payload: payload,
		},
	}

	data, err := json.Marshal(msg)
	if err != nil {
		b.mu.Lock()
		delete(b.pending, id)
		b.mu.Unlock()
		return nil, err
	}
	data = append(data, '\n')

	b.writeMu.Lock()
	_, err = b.conn.Write(data)
	b.writeMu.Unlock()
	if err != nil {
		b.mu.Lock()
		delete(b.pending, id)
		b.mu.Unlock()
		return nil, err
	}

	resp, ok := <-respCh
	if !ok || resp == nil {
		return nil, errors.New("bridge: request cancelled or connection closed")
	}
	return resp, nil
}

// SubmitBlock implements ConsensusBridge.
func (b *SocketConsensusBridge) SubmitBlock(blockBytes []byte) error {
	payload := map[string]string{
		"block_hex": hex.EncodeToString(blockBytes),
	}
	resp, err := b.call("SubmitBlock", payload)
	if err != nil {
		return err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return errors.New(p.Message)
	}
	return nil
}

// SubmitTransaction implements ConsensusBridge.
func (b *SocketConsensusBridge) SubmitTransaction(txBytes []byte) error {
	payload := map[string]string{
		"tx_hex": hex.EncodeToString(txBytes),
	}
	resp, err := b.call("SubmitTransaction", payload)
	if err != nil {
		return err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return errors.New(p.Message)
	}
	return nil
}

// GetBlockLocator implements ConsensusBridge.
func (b *SocketConsensusBridge) GetBlockLocator() ([][32]byte, error) {
	resp, err := b.call("GetBlockLocator", nil)
	if err != nil {
		return nil, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return nil, errors.New(p.Message)
	}

	var p struct {
		HashesHex []string `json:"hashes_hex"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return nil, err
	}

	res := make([][32]byte, 0, len(p.HashesHex))
	for _, hHex := range p.HashesHex {
		hBytes, err := hex.DecodeString(hHex)
		if err == nil && len(hBytes) == 32 {
			var h [32]byte
			copy(h[:], hBytes)
			res = append(res, h)
		}
	}
	return res, nil
}

// GetCanonicalHashes implements ConsensusBridge.
func (b *SocketConsensusBridge) GetCanonicalHashes() ([][32]byte, error) {
	resp, err := b.call("GetCanonicalHashes", nil)
	if err != nil {
		return nil, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return nil, errors.New(p.Message)
	}

	var p struct {
		HashesHex []string `json:"hashes_hex"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return nil, err
	}

	res := make([][32]byte, 0, len(p.HashesHex))
	for _, hHex := range p.HashesHex {
		hBytes, err := hex.DecodeString(hHex)
		if err == nil && len(hBytes) == 32 {
			var h [32]byte
			copy(h[:], hBytes)
			res = append(res, h)
		}
	}
	return res, nil
}

// GetBlockByHash implements ConsensusBridge.
func (b *SocketConsensusBridge) GetBlockByHash(hash [32]byte) ([]byte, error) {
	payload := map[string]string{
		"hash_hex": hex.EncodeToString(hash[:]),
	}
	resp, err := b.call("GetBlockByHash", payload)
	if err != nil {
		return nil, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return nil, errors.New(p.Message)
	}

	var p struct {
		BlockHex *string `json:"block_hex"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return nil, err
	}
	if p.BlockHex == nil {
		return nil, ErrNotFound
	}
	return hex.DecodeString(*p.BlockHex)
}

// GetTransactionByHash implements ConsensusBridge.
func (b *SocketConsensusBridge) GetTransactionByHash(hash [32]byte) ([]byte, error) {
	payload := map[string]string{
		"hash_hex": hex.EncodeToString(hash[:]),
	}
	resp, err := b.call("GetTransactionByHash", payload)
	if err != nil {
		return nil, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return nil, errors.New(p.Message)
	}

	var p struct {
		TxHex *string `json:"tx_hex"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return nil, err
	}
	if p.TxHex == nil {
		return nil, ErrNotFound
	}
	return hex.DecodeString(*p.TxHex)
}

// ExportSnapshotChunk implements ConsensusBridge.
func (b *SocketConsensusBridge) ExportSnapshotChunk(blockHash [32]byte, chunkIndex uint32, chunkSize uint32) (*wire.MsgSnapshot, error) {
	payload := map[string]interface{}{
		"block_hash_hex": hex.EncodeToString(blockHash[:]),
		"chunk_index":    chunkIndex,
		"chunk_size":     chunkSize,
	}
	resp, err := b.call("ExportSnapshotChunk", payload)
	if err != nil {
		return nil, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return nil, errors.New(p.Message)
	}

	var p struct {
		BlockHashHex string `json:"block_hash_hex"`
		ChunkIndex   uint32 `json:"chunk_index"`
		TotalChunks  uint32 `json:"total_chunks"`
		Entries      []struct {
			TxidHex          string `json:"txid_hex"`
			Index            uint32 `json:"index"`
			ValueQuanta      uint64 `json:"value_quanta"`
			LockingScriptHex string `json:"locking_script_hex"`
		} `json:"entries"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return nil, err
	}

	entries := make([]wire.UtxoWireEntry, 0, len(p.Entries))
	for _, e := range p.Entries {
		txidBytes, err := hex.DecodeString(e.TxidHex)
		if err != nil || len(txidBytes) != 32 {
			return nil, errors.New("bridge: invalid txid hex in snapshot entry")
		}
		var txid [32]byte
		copy(txid[:], txidBytes)

		scriptBytes, err := hex.DecodeString(e.LockingScriptHex)
		if err != nil {
			return nil, errors.New("bridge: invalid locking script hex in snapshot entry")
		}

		entries = append(entries, wire.UtxoWireEntry{
			TxID:          txid,
			Index:         e.Index,
			Value:         e.ValueQuanta,
			LockingScript: scriptBytes,
		})
	}

	var parsedHash [32]byte
	hBytes, err := hex.DecodeString(p.BlockHashHex)
	if err == nil && len(hBytes) == 32 {
		copy(parsedHash[:], hBytes)
	} else {
		parsedHash = blockHash
	}

	return &wire.MsgSnapshot{
		BlockHash:   parsedHash,
		ChunkIndex:  p.ChunkIndex,
		TotalChunks: p.TotalChunks,
		Entries:     entries,
	}, nil
}

// ApplySnapshot implements ConsensusBridge.
func (b *SocketConsensusBridge) ApplySnapshot(blockHash [32]byte, entries []wire.UtxoWireEntry) (int, error) {
	type entryDto struct {
		TxidHex          string `json:"txid_hex"`
		Index            uint32 `json:"index"`
		ValueQuanta      uint64 `json:"value_quanta"`
		LockingScriptHex string `json:"locking_script_hex"`
	}

	entryDtos := make([]entryDto, len(entries))
	for i, e := range entries {
		entryDtos[i] = entryDto{
			TxidHex:          hex.EncodeToString(e.TxID[:]),
			Index:            e.Index,
			ValueQuanta:      e.Value,
			LockingScriptHex: hex.EncodeToString(e.LockingScript),
		}
	}

	payload := map[string]interface{}{
		"block_hash_hex": hex.EncodeToString(blockHash[:]),
		"entries":        entryDtos,
	}

	resp, err := b.call("ApplySnapshot", payload)
	if err != nil {
		return 0, err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return 0, errors.New(p.Message)
	}

	var p struct {
		BlockHashHex string `json:"block_hash_hex"`
		UtxoCount    int    `json:"utxo_count"`
	}
	if err := json.Unmarshal(resp.Payload, &p); err != nil {
		return 0, err
	}

	return p.UtxoCount, nil
}

// UpdatePeerCount implements ConsensusBridge.
func (b *SocketConsensusBridge) UpdatePeerCount(count int) error {
	payload := map[string]interface{}{
		"count": count,
	}
	resp, err := b.call("UpdatePeerCount", payload)
	if err != nil {
		return err
	}
	if resp.Type == "Error" {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(resp.Payload, &p)
		return errors.New(p.Message)
	}
	return nil
}

// Close terminates the bridge socket connection.
func (b *SocketConsensusBridge) Close() error {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.isClosed {
		return nil
	}
	b.isClosed = true
	return b.conn.Close()
}
