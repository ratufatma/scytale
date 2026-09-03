package wire

import (
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
)

var (
	// ErrTooManySnapshotEntries is returned when a snapshot message declares more than MaxSnapshotChunkEntries.
	ErrTooManySnapshotEntries = errors.New("wire: snapshot message exceeds maximum entry count (2000)")

	// ErrLockingScriptTooLarge is returned when an entry's locking script exceeds MaxLockingScriptSize.
	ErrLockingScriptTooLarge = errors.New("wire: entry locking script exceeds maximum size (10000)")

	// ErrSnapshotPayloadTooShort is returned when payload bytes end before decoding declared fields.
	ErrSnapshotPayloadTooShort = errors.New("wire: snapshot payload too short")
)

// MsgGetSnapshot is the wire request sent to fetch a chunk of the UTXO state snapshot.
// Fixed wire size: 32 bytes (BlockHash) + 4 bytes (ChunkIndex LE) = 36 bytes.
type MsgGetSnapshot struct {
	BlockHash  [32]byte
	ChunkIndex uint32
}

// Command returns the wire command string.
func (m *MsgGetSnapshot) Command() string {
	return CmdGetSnapshot
}

// Serialize encodes MsgGetSnapshot to a 36-byte slice.
func (m *MsgGetSnapshot) Serialize() []byte {
	buf := make([]byte, 36)
	copy(buf[0:32], m.BlockHash[:])
	binary.LittleEndian.PutUint32(buf[32:36], m.ChunkIndex)
	return buf
}

// EncodeGetSnapshot writes the binary representation of MsgGetSnapshot to w.
func EncodeGetSnapshot(w io.Writer, msg *MsgGetSnapshot) error {
	if msg == nil {
		return errors.New("wire: nil MsgGetSnapshot")
	}
	_, err := w.Write(msg.Serialize())
	return err
}

// DecodeGetSnapshot reads a MsgGetSnapshot from r.
func DecodeGetSnapshot(r io.Reader) (*MsgGetSnapshot, error) {
	var buf [36]byte
	if _, err := io.ReadFull(r, buf[:]); err != nil {
		if errors.Is(err, io.EOF) || errors.Is(err, io.ErrUnexpectedEOF) {
			return nil, ErrSnapshotPayloadTooShort
		}
		return nil, err
	}

	var msg MsgGetSnapshot
	copy(msg.BlockHash[:], buf[0:32])
	msg.ChunkIndex = binary.LittleEndian.Uint32(buf[32:36])
	return &msg, nil
}

// UtxoWireEntry represents a single unspent transaction output on the wire.
type UtxoWireEntry struct {
	TxID          [32]byte
	Index         uint32
	Value         uint64
	LockingScript []byte
}

// MsgSnapshot is the chunked state transfer response containing active UTXOs for a block hash.
// Wire Header Layout:
// [ 32B BlockHash | 4B ChunkIndex (LE) | 4B TotalChunks (LE) | 4B ItemCount (LE) ]
// Followed by ItemCount entries:
// [ 32B TxID | 4B Index (LE) | 8B Value (LE) | 4B ScriptLen (LE) | ScriptLen Bytes ]
type MsgSnapshot struct {
	BlockHash   [32]byte
	ChunkIndex  uint32
	TotalChunks uint32
	Entries     []UtxoWireEntry
}

// Command returns the wire command string.
func (m *MsgSnapshot) Command() string {
	return CmdSnapshot
}

// EncodeSnapshot writes MsgSnapshot to w.
func EncodeSnapshot(w io.Writer, msg *MsgSnapshot) error {
	if msg == nil {
		return errors.New("wire: nil MsgSnapshot")
	}
	if len(msg.Entries) > MaxSnapshotChunkEntries {
		return fmt.Errorf("%w: declared %d", ErrTooManySnapshotEntries, len(msg.Entries))
	}

	var header [44]byte
	copy(header[0:32], msg.BlockHash[:])
	binary.LittleEndian.PutUint32(header[32:36], msg.ChunkIndex)
	binary.LittleEndian.PutUint32(header[36:40], msg.TotalChunks)
	binary.LittleEndian.PutUint32(header[40:44], uint32(len(msg.Entries)))

	if _, err := w.Write(header[:]); err != nil {
		return err
	}

	var entryHeader [48]byte
	for _, entry := range msg.Entries {
		if len(entry.LockingScript) > MaxLockingScriptSize {
			return fmt.Errorf("%w: got %d bytes", ErrLockingScriptTooLarge, len(entry.LockingScript))
		}
		copy(entryHeader[0:32], entry.TxID[:])
		binary.LittleEndian.PutUint32(entryHeader[32:36], entry.Index)
		binary.LittleEndian.PutUint64(entryHeader[36:44], entry.Value)
		binary.LittleEndian.PutUint32(entryHeader[44:48], uint32(len(entry.LockingScript)))

		if _, err := w.Write(entryHeader[:]); err != nil {
			return err
		}
		if len(entry.LockingScript) > 0 {
			if _, err := w.Write(entry.LockingScript); err != nil {
				return err
			}
		}
	}

	return nil
}

// Serialize returns the binary wire encoding of MsgSnapshot.
func (m *MsgSnapshot) Serialize() ([]byte, error) {
	var buf bytes.Buffer
	if err := EncodeSnapshot(&buf, m); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// DecodeSnapshot reads a MsgSnapshot from r.
func DecodeSnapshot(r io.Reader) (*MsgSnapshot, error) {
	var header [44]byte
	if _, err := io.ReadFull(r, header[:]); err != nil {
		if errors.Is(err, io.EOF) || errors.Is(err, io.ErrUnexpectedEOF) {
			return nil, ErrSnapshotPayloadTooShort
		}
		return nil, err
	}

	msg := &MsgSnapshot{}
	copy(msg.BlockHash[:], header[0:32])
	msg.ChunkIndex = binary.LittleEndian.Uint32(header[32:36])
	msg.TotalChunks = binary.LittleEndian.Uint32(header[36:40])
	itemCount := binary.LittleEndian.Uint32(header[40:44])

	if itemCount > MaxSnapshotChunkEntries {
		return nil, fmt.Errorf("%w: declared %d", ErrTooManySnapshotEntries, itemCount)
	}

	msg.Entries = make([]UtxoWireEntry, itemCount)
	var entryHeader [48]byte
	for i := uint32(0); i < itemCount; i++ {
		if _, err := io.ReadFull(r, entryHeader[:]); err != nil {
			if errors.Is(err, io.EOF) || errors.Is(err, io.ErrUnexpectedEOF) {
				return nil, ErrSnapshotPayloadTooShort
			}
			return nil, err
		}

		copy(msg.Entries[i].TxID[:], entryHeader[0:32])
		msg.Entries[i].Index = binary.LittleEndian.Uint32(entryHeader[32:36])
		msg.Entries[i].Value = binary.LittleEndian.Uint64(entryHeader[36:44])
		scriptLen := binary.LittleEndian.Uint32(entryHeader[44:48])

		if scriptLen > MaxLockingScriptSize {
			return nil, fmt.Errorf("%w: got %d bytes", ErrLockingScriptTooLarge, scriptLen)
		}

		if scriptLen > 0 {
			script := make([]byte, scriptLen)
			if _, err := io.ReadFull(r, script); err != nil {
				if errors.Is(err, io.EOF) || errors.Is(err, io.ErrUnexpectedEOF) {
					return nil, ErrSnapshotPayloadTooShort
				}
				return nil, err
			}
			msg.Entries[i].LockingScript = script
		}
	}

	return msg, nil
}
