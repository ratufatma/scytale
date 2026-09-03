package sync_test

import (
	"bytes"
	"sync"
	"testing"

	"github.com/scytale-network/scytale-p2p/internal/bridge"
	gossip_pkg "github.com/scytale-network/scytale-p2p/internal/gossip"
	sync_pkg "github.com/scytale-network/scytale-p2p/internal/sync"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

type captureSender struct {
	mu   sync.Mutex
	msgs []struct {
		cmd     string
		payload []byte
	}
}

func (c *captureSender) Send(cmd string, payload []byte) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	cp := make([]byte, len(payload))
	copy(cp, payload)
	c.msgs = append(c.msgs, struct {
		cmd     string
		payload []byte
	}{cmd, cp})
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

func makeHash(i int) [32]byte {
	var h [32]byte
	h[0] = byte(i)
	h[1] = byte(i >> 8)
	return h
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

// TestSendGetBlocksSendsLocator verifies that SendGetBlocks encodes the bridge
// locator and sends a `getblocks` message.
func TestSendGetBlocksSendsLocator(t *testing.T) {
	locator := [][32]byte{makeHash(10), makeHash(5), makeHash(0)}
	mock := bridge.NewMockConsensusBridge()
	mock.SetLocator(locator)

	syncer := sync_pkg.New(mock)
	sender := &captureSender{}

	if err := syncer.SendGetBlocks(sender); err != nil {
		t.Fatalf("SendGetBlocks error: %v", err)
	}

	payloads := sender.received(wire.CmdGetBlocks)
	if len(payloads) != 1 {
		t.Fatalf("expected 1 getblocks message, got %d", len(payloads))
	}

	decoded, err := gossip_pkg.DecodeHashList(payloads[0])
	if err != nil {
		t.Fatalf("DecodeHashList error: %v", err)
	}
	if len(decoded) != len(locator) {
		t.Fatalf("locator length mismatch: got %d, want %d", len(decoded), len(locator))
	}
	for i, h := range locator {
		if decoded[i] != h {
			t.Errorf("locator[%d] mismatch", i)
		}
	}
}

// TestBlockSyncBatchCatchup verifies that HandleInvBlocks requests up to MaxBatchSize
// blocks from the announced hashes.
func TestBlockSyncBatchCatchup(t *testing.T) {
	// Build block hashes exceeding MaxBatchSize
	totalHashes := sync_pkg.MaxBatchSize + 10
	var hashes [][32]byte
	for i := 0; i < totalHashes; i++ {
		hashes = append(hashes, makeHash(i))
	}

	mock := bridge.NewMockConsensusBridge()
	syncer := sync_pkg.New(mock)
	sender := &captureSender{}

	invPayload := gossip_pkg.EncodeHashList(hashes)
	count, err := syncer.HandleInvBlocks(sender, invPayload)
	if err != nil {
		t.Fatalf("HandleInvBlocks error: %v", err)
	}

	// Must be capped at MaxBatchSize
	if count != sync_pkg.MaxBatchSize {
		t.Errorf("batch count: got %d, want %d", count, sync_pkg.MaxBatchSize)
	}

	// Must have sent exactly one `getdata`
	getdataPayloads := sender.received(wire.CmdGetData)
	if len(getdataPayloads) != 1 {
		t.Fatalf("expected 1 getdata, got %d", len(getdataPayloads))
	}

	// The getdata must contain exactly MaxBatchSize InvItems
	items, err := gossip_pkg.DecodeInv(getdataPayloads[0])
	if err != nil {
		t.Fatalf("DecodeInv error: %v", err)
	}
	if len(items) != sync_pkg.MaxBatchSize {
		t.Errorf("getdata items: got %d, want %d", len(items), sync_pkg.MaxBatchSize)
	}
	// All items must be InvTypeBlock
	for _, item := range items {
		if item.Type != gossip_pkg.InvTypeBlock {
			t.Errorf("expected InvTypeBlock, got %d", item.Type)
		}
	}
}

// TestHandleGetBlocksRespondsWithInvBlocks verifies that HandleGetBlocks sends an
// `invblocks` response with our known hashes (up to MaxBatchSize).
func TestHandleGetBlocksRespondsWithInvBlocks(t *testing.T) {
	var knownHashes [][32]byte
	for i := 0; i < 10; i++ {
		knownHashes = append(knownHashes, makeHash(i))
	}

	// 1. Without locator: sends all known hashes
	sender1 := &captureSender{}
	if err := sync_pkg.HandleGetBlocks(sender1, knownHashes, nil); err != nil {
		t.Fatalf("HandleGetBlocks error: %v", err)
	}
	payloads1 := sender1.received(wire.CmdInvBlocks)
	if len(payloads1) != 1 {
		t.Fatalf("expected 1 invblocks, got %d", len(payloads1))
	}
	hashes1, err := gossip_pkg.DecodeHashList(payloads1[0])
	if err != nil {
		t.Fatalf("DecodeHashList error: %v", err)
	}
	if len(hashes1) != len(knownHashes) {
		t.Errorf("hash count: got %d, want %d", len(hashes1), len(knownHashes))
	}

	// 2. With locator matching index 0: sends knownHashes[1:]
	sender2 := &captureSender{}
	peerLocator := [][32]byte{makeHash(0)}

	if err := sync_pkg.HandleGetBlocks(sender2, knownHashes, peerLocator); err != nil {
		t.Fatalf("HandleGetBlocks error: %v", err)
	}

	payloads2 := sender2.received(wire.CmdInvBlocks)
	if len(payloads2) != 1 {
		t.Fatalf("expected 1 invblocks, got %d", len(payloads2))
	}

	hashes2, err := gossip_pkg.DecodeHashList(payloads2[0])
	if err != nil {
		t.Fatalf("DecodeHashList error: %v", err)
	}
	expectedDelta := knownHashes[1:]
	if len(hashes2) != len(expectedDelta) {
		t.Errorf("hash count: got %d, want %d", len(hashes2), len(expectedDelta))
	}
	for i, h := range expectedDelta {
		if !bytes.Equal(hashes2[i][:], h[:]) {
			t.Errorf("hash[%d] mismatch", i)
		}
	}
}

// TestHashListEncodeDecodeRoundtrip verifies the hash list codec used in IBD.
func TestHashListEncodeDecodeRoundtrip(t *testing.T) {
	original := [][32]byte{makeHash(1), makeHash(2), makeHash(3)}
	encoded := gossip_pkg.EncodeHashList(original)
	decoded, err := gossip_pkg.DecodeHashList(encoded)
	if err != nil {
		t.Fatalf("DecodeHashList error: %v", err)
	}
	if len(decoded) != len(original) {
		t.Fatalf("length mismatch: got %d, want %d", len(decoded), len(original))
	}
	for i := range original {
		if decoded[i] != original[i] {
			t.Errorf("hash[%d] mismatch", i)
		}
	}
}
