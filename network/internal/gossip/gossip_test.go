package gossip_test

import (
	"bytes"
	"sync"
	"testing"

	"github.com/scytale-network/scytale-p2p/internal/bridge"
	"github.com/scytale-network/scytale-p2p/internal/gossip"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

// captureSender records all sent messages for test assertions.
type captureSender struct {
	mu   sync.Mutex
	msgs []struct{ cmd string; payload []byte }
}

func (c *captureSender) Send(cmd string, payload []byte) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	cp := make([]byte, len(payload))
	copy(cp, payload)
	c.msgs = append(c.msgs, struct{ cmd string; payload []byte }{cmd, cp})
	return nil
}

func (c *captureSender) received(cmd string) [][]byte {
	c.mu.Lock()
	defer c.mu.Unlock()
	var out [][]byte
	for _, m := range c.msgs {
		if m.cmd == cmd {
			out = append(out, m.payload)
		}
	}
	return out
}

func hash(seed byte) [32]byte {
	var h [32]byte
	h[0] = seed
	return h
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

// TestInvEncodeDecodeRoundtrip verifies the InvItem binary codec.
func TestInvEncodeDecodeRoundtrip(t *testing.T) {
	items := []gossip.InvItem{
		{Type: gossip.InvTypeTx, Hash: hash(0xAA)},
		{Type: gossip.InvTypeBlock, Hash: hash(0xBB)},
	}
	encoded := gossip.EncodeInv(items)
	decoded, err := gossip.DecodeInv(encoded)
	if err != nil {
		t.Fatalf("DecodeInv error: %v", err)
	}
	if len(decoded) != len(items) {
		t.Fatalf("length mismatch: got %d, want %d", len(decoded), len(items))
	}
	for i, item := range items {
		if decoded[i] != item {
			t.Errorf("item[%d] mismatch: got %+v, want %+v", i, decoded[i], item)
		}
	}
}

// TestTwoStepGossipFlow simulates the full inv → getdata → block relay flow.
func TestTwoStepGossipFlow(t *testing.T) {
	blockHash := hash(0x01)
	blockData := []byte("raw_block_bytes_from_consensus")

	mock := bridge.NewMockConsensusBridge()
	mock.AddBlock(blockHash, blockData)

	filter := gossip.NewFilter()
	engine := gossip.NewEngine(filter, mock)
	sender := &captureSender{}

	// Step 1: receive an `inv` for a block we have not seen
	inv := gossip.EncodeInv([]gossip.InvItem{{Type: gossip.InvTypeBlock, Hash: blockHash}})
	if err := engine.HandleInv(sender, inv); err != nil {
		t.Fatalf("HandleInv error: %v", err)
	}

	// Step 2: verify we sent a `getdata`
	getdataPayloads := sender.received(wire.CmdGetData)
	if len(getdataPayloads) != 1 {
		t.Fatalf("expected 1 getdata message, got %d", len(getdataPayloads))
	}

	// Step 3: simulate the peer responding with a `block` — we call HandleGetData
	// on behalf of our node to fetch and relay
	if err := engine.HandleGetData(sender, getdataPayloads[0]); err != nil {
		t.Fatalf("HandleGetData error: %v", err)
	}

	blockPayloads := sender.received(wire.CmdBlock)
	if len(blockPayloads) != 1 {
		t.Fatalf("expected 1 block message, got %d", len(blockPayloads))
	}
	if !bytes.Equal(blockPayloads[0], blockData) {
		t.Errorf("block payload mismatch: got %q, want %q", blockPayloads[0], blockData)
	}
}

// TestDuplicateSuppression verifies that a second `inv` with the same hash does NOT
// trigger a second `getdata` request.
func TestDuplicateSuppression(t *testing.T) {
	txHash := hash(0x42)

	mock := bridge.NewMockConsensusBridge()
	filter := gossip.NewFilter()
	engine := gossip.NewEngine(filter, mock)
	sender := &captureSender{}

	inv := gossip.EncodeInv([]gossip.InvItem{{Type: gossip.InvTypeTx, Hash: txHash}})

	// First inv — should generate a getdata
	if err := engine.HandleInv(sender, inv); err != nil {
		t.Fatalf("first HandleInv error: %v", err)
	}

	// Second inv with the same hash — must be suppressed
	if err := engine.HandleInv(sender, inv); err != nil {
		t.Fatalf("second HandleInv error: %v", err)
	}

	getdataPayloads := sender.received(wire.CmdGetData)
	if len(getdataPayloads) != 1 {
		t.Errorf("expected exactly 1 getdata (duplicate suppressed), got %d", len(getdataPayloads))
	}
}

// TestHandleTxForwardsToBridge verifies that HandleTx forwards the raw bytes to the bridge.
func TestHandleTxForwardsToBridge(t *testing.T) {
	txData := []byte("raw_tx_bytes")
	mock := bridge.NewMockConsensusBridge()
	filter := gossip.NewFilter()
	engine := gossip.NewEngine(filter, mock)

	if err := engine.HandleTx(txData); err != nil {
		t.Fatalf("HandleTx error: %v", err)
	}

	if len(mock.SubmittedTxs) != 1 {
		t.Errorf("expected 1 submitted tx, got %d", len(mock.SubmittedTxs))
	}
	if !bytes.Equal(mock.SubmittedTxs[0], txData) {
		t.Errorf("submitted tx mismatch: got %q, want %q", mock.SubmittedTxs[0], txData)
	}
}

// TestHandleBlockForwardsToBridge verifies that HandleBlock forwards the raw bytes to the bridge.
func TestHandleBlockForwardsToBridge(t *testing.T) {
	blockData := []byte("raw_block_bytes")
	mock := bridge.NewMockConsensusBridge()
	filter := gossip.NewFilter()
	engine := gossip.NewEngine(filter, mock)

	if err := engine.HandleBlock(blockData); err != nil {
		t.Fatalf("HandleBlock error: %v", err)
	}
	if len(mock.SubmittedBlocks) != 1 {
		t.Errorf("expected 1 submitted block, got %d", len(mock.SubmittedBlocks))
	}
	if !bytes.Equal(mock.SubmittedBlocks[0], blockData) {
		t.Errorf("submitted block mismatch: got %q, want %q", mock.SubmittedBlocks[0], blockData)
	}
}
