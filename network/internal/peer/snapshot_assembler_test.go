package peer

import (
	"crypto/rand"
	"errors"
	"testing"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

func TestSnapshotAssembler_Assembly(t *testing.T) {
	sa := NewSnapshotAssembler()

	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	var txid1, txid2, txid3 [32]byte
	_, _ = rand.Read(txid1[:])
	_, _ = rand.Read(txid2[:])
	_, _ = rand.Read(txid3[:])

	chunk0 := &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  0,
		TotalChunks: 2,
		Entries: []wire.UtxoWireEntry{
			{TxID: txid1, Index: 0, Value: 100},
			{TxID: txid2, Index: 1, Value: 200},
		},
	}

	chunk1 := &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  1,
		TotalChunks: 2,
		Entries: []wire.UtxoWireEntry{
			{TxID: txid3, Index: 0, Value: 300},
		},
	}

	// Add chunk 1 first (out of order)
	complete, err := sa.AddChunk(chunk1)
	if err != nil {
		t.Fatalf("AddChunk 1 failed: %v", err)
	}
	if complete {
		t.Fatalf("expected incomplete after chunk 1")
	}
	if sa.IsComplete(blockHash) {
		t.Fatalf("IsComplete should be false")
	}

	// Attempt assemble before completion should fail
	_, err = sa.Assemble(blockHash)
	if !errors.Is(err, ErrIncompleteSnapshot) {
		t.Fatalf("expected ErrIncompleteSnapshot, got %v", err)
	}

	// Add chunk 0
	complete, err = sa.AddChunk(chunk0)
	if err != nil {
		t.Fatalf("AddChunk 0 failed: %v", err)
	}
	if !complete {
		t.Fatalf("expected complete after chunk 0")
	}
	if !sa.IsComplete(blockHash) {
		t.Fatalf("IsComplete should be true")
	}

	// Assemble
	entries, err := sa.Assemble(blockHash)
	if err != nil {
		t.Fatalf("Assemble failed: %v", err)
	}
	if len(entries) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(entries))
	}

	// Verify order: chunk 0 entries then chunk 1 entries
	if entries[0].TxID != txid1 || entries[1].TxID != txid2 || entries[2].TxID != txid3 {
		t.Fatalf("entries not assembled in correct order")
	}

	// Calling assemble again should return ErrSnapshotNotFound (state cleared)
	_, err = sa.Assemble(blockHash)
	if !errors.Is(err, ErrSnapshotNotFound) {
		t.Fatalf("expected ErrSnapshotNotFound after assemble, got %v", err)
	}
}

func TestSnapshotAssembler_MismatchedTotalChunks(t *testing.T) {
	sa := NewSnapshotAssembler()

	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	chunk0 := &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  0,
		TotalChunks: 2,
		Entries:     nil,
	}

	chunk1Bad := &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  1,
		TotalChunks: 3, // Conflict with declared 2
		Entries:     nil,
	}

	if _, err := sa.AddChunk(chunk0); err != nil {
		t.Fatalf("AddChunk 0 failed: %v", err)
	}

	_, err := sa.AddChunk(chunk1Bad)
	if !errors.Is(err, ErrTotalChunksMismatch) {
		t.Fatalf("expected ErrTotalChunksMismatch, got %v", err)
	}
}

func TestSnapshotAssembler_OutOfBoundsChunk(t *testing.T) {
	sa := NewSnapshotAssembler()

	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	chunk := &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  5,
		TotalChunks: 3,
		Entries:     nil,
	}

	_, err := sa.AddChunk(chunk)
	if !errors.Is(err, ErrChunkIndexOutOfBounds) {
		t.Fatalf("expected ErrChunkIndexOutOfBounds, got %v", err)
	}
}
