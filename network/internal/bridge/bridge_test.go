package bridge

import (
	"crypto/rand"
	"testing"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

func TestMockConsensusBridge_Snapshot(t *testing.T) {
	mock := NewMockConsensusBridge()

	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	var txid1, txid2, txid3 [32]byte
	_, _ = rand.Read(txid1[:])
	_, _ = rand.Read(txid2[:])
	_, _ = rand.Read(txid3[:])

	entries := []wire.UtxoWireEntry{
		{TxID: txid1, Index: 0, Value: 1000},
		{TxID: txid2, Index: 1, Value: 2000},
		{TxID: txid3, Index: 0, Value: 3000},
	}

	mock.SetSnapshotEntries(blockHash, entries)

	// Chunk size 2: total entries 3 -> 2 chunks
	c0, err := mock.ExportSnapshotChunk(blockHash, 0, 2)
	if err != nil {
		t.Fatalf("ExportSnapshotChunk 0 failed: %v", err)
	}
	if c0.TotalChunks != 2 || len(c0.Entries) != 2 {
		t.Fatalf("chunk 0 unexpected: total=%d, entries=%d", c0.TotalChunks, len(c0.Entries))
	}
	if c0.Entries[0].TxID != txid1 || c0.Entries[1].TxID != txid2 {
		t.Fatalf("chunk 0 entry mismatch")
	}

	c1, err := mock.ExportSnapshotChunk(blockHash, 1, 2)
	if err != nil {
		t.Fatalf("ExportSnapshotChunk 1 failed: %v", err)
	}
	if c1.TotalChunks != 2 || len(c1.Entries) != 1 {
		t.Fatalf("chunk 1 unexpected: total=%d, entries=%d", c1.TotalChunks, len(c1.Entries))
	}
	if c1.Entries[0].TxID != txid3 {
		t.Fatalf("chunk 1 entry mismatch")
	}

	// Apply snapshot
	count, err := mock.ApplySnapshot(blockHash, entries)
	if err != nil {
		t.Fatalf("ApplySnapshot failed: %v", err)
	}
	if count != 3 {
		t.Fatalf("expected 3 applied entries, got %d", count)
	}

	applied, ok := mock.AppliedSnapshots[blockHash]
	if !ok || len(applied) != 3 {
		t.Fatalf("applied entries not recorded properly in mock")
	}
}
