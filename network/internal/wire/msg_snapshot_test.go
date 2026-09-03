package wire

import (
	"bytes"
	"crypto/rand"
	"errors"
	"testing"
)

func TestMsgGetSnapshot_Roundtrip(t *testing.T) {
	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	original := &MsgGetSnapshot{
		BlockHash:  blockHash,
		ChunkIndex: 42,
	}

	if original.Command() != CmdGetSnapshot {
		t.Fatalf("expected command %s, got %s", CmdGetSnapshot, original.Command())
	}

	data := original.Serialize()
	if len(data) != 36 {
		t.Fatalf("expected serialized length 36, got %d", len(data))
	}

	var buf bytes.Buffer
	if err := EncodeGetSnapshot(&buf, original); err != nil {
		t.Fatalf("EncodeGetSnapshot failed: %v", err)
	}

	decoded, err := DecodeGetSnapshot(&buf)
	if err != nil {
		t.Fatalf("DecodeGetSnapshot failed: %v", err)
	}

	if decoded.BlockHash != original.BlockHash {
		t.Fatalf("block hash mismatch")
	}
	if decoded.ChunkIndex != original.ChunkIndex {
		t.Fatalf("chunk index mismatch: got %d, expected %d", decoded.ChunkIndex, original.ChunkIndex)
	}
}

func TestMsgGetSnapshot_Truncated(t *testing.T) {
	buf := bytes.NewReader([]byte{0x01, 0x02, 0x03})
	_, err := DecodeGetSnapshot(buf)
	if !errors.Is(err, ErrSnapshotPayloadTooShort) {
		t.Fatalf("expected ErrSnapshotPayloadTooShort, got %v", err)
	}
}

func TestMsgSnapshot_Roundtrip(t *testing.T) {
	var blockHash [32]byte
	_, _ = rand.Read(blockHash[:])

	var txid1, txid2 [32]byte
	_, _ = rand.Read(txid1[:])
	_, _ = rand.Read(txid2[:])

	original := &MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  1,
		TotalChunks: 5,
		Entries: []UtxoWireEntry{
			{
				TxID:          txid1,
				Index:         0,
				Value:         50000000,
				LockingScript: []byte{0x76, 0xa9, 0x14, 0x01, 0x02, 0x88, 0xac},
			},
			{
				TxID:          txid2,
				Index:         2,
				Value:         100000000,
				LockingScript: []byte{}, // Empty script test
			},
		},
	}

	if original.Command() != CmdSnapshot {
		t.Fatalf("expected command %s, got %s", CmdSnapshot, original.Command())
	}

	raw, err := original.Serialize()
	if err != nil {
		t.Fatalf("Serialize failed: %v", err)
	}

	decoded, err := DecodeSnapshot(bytes.NewReader(raw))
	if err != nil {
		t.Fatalf("DecodeSnapshot failed: %v", err)
	}

	if decoded.BlockHash != original.BlockHash {
		t.Fatalf("block hash mismatch")
	}
	if decoded.ChunkIndex != original.ChunkIndex || decoded.TotalChunks != original.TotalChunks {
		t.Fatalf("chunk index/total mismatch")
	}
	if len(decoded.Entries) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(decoded.Entries))
	}

	if decoded.Entries[0].TxID != txid1 || decoded.Entries[0].Index != 0 || decoded.Entries[0].Value != 50000000 {
		t.Fatalf("entry 0 field mismatch")
	}
	if !bytes.Equal(decoded.Entries[0].LockingScript, original.Entries[0].LockingScript) {
		t.Fatalf("entry 0 script mismatch")
	}

	if decoded.Entries[1].TxID != txid2 || decoded.Entries[1].Index != 2 || decoded.Entries[1].Value != 100000000 {
		t.Fatalf("entry 1 field mismatch")
	}
	if len(decoded.Entries[1].LockingScript) != 0 {
		t.Fatalf("entry 1 empty script mismatch")
	}
}

func TestMsgSnapshot_MaxEntriesLimit(t *testing.T) {
	msg := &MsgSnapshot{
		Entries: make([]UtxoWireEntry, MaxSnapshotChunkEntries+1),
	}

	var buf bytes.Buffer
	err := EncodeSnapshot(&buf, msg)
	if !errors.Is(err, ErrTooManySnapshotEntries) {
		t.Fatalf("expected ErrTooManySnapshotEntries on encode, got %v", err)
	}
}

func TestMsgSnapshot_OversizedLockingScript(t *testing.T) {
	msg := &MsgSnapshot{
		Entries: []UtxoWireEntry{
			{
				LockingScript: make([]byte, MaxLockingScriptSize+1),
			},
		},
	}

	var buf bytes.Buffer
	err := EncodeSnapshot(&buf, msg)
	if !errors.Is(err, ErrLockingScriptTooLarge) {
		t.Fatalf("expected ErrLockingScriptTooLarge on encode, got %v", err)
	}
}

func TestMsgSnapshot_TruncatedPayload(t *testing.T) {
	var blockHash [32]byte
	msg := &MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  0,
		TotalChunks: 1,
		Entries: []UtxoWireEntry{
			{
				Index:         0,
				Value:         100,
				LockingScript: []byte{0x01, 0x02, 0x03, 0x04},
			},
		},
	}

	data, err := msg.Serialize()
	if err != nil {
		t.Fatalf("Serialize failed: %v", err)
	}

	// Truncate by 2 bytes
	truncated := data[:len(data)-2]
	_, err = DecodeSnapshot(bytes.NewReader(truncated))
	if !errors.Is(err, ErrSnapshotPayloadTooShort) {
		t.Fatalf("expected ErrSnapshotPayloadTooShort, got %v", err)
	}
}
