package peer

import (
	"errors"
	"fmt"
	"sync"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

var (
	// ErrTotalChunksMismatch is returned when a chunk declares a different total chunks count.
	ErrTotalChunksMismatch = errors.New("snapshot: total chunks mismatch for active block hash")

	// ErrChunkIndexOutOfBounds is returned when chunk index is >= total chunks.
	ErrChunkIndexOutOfBounds = errors.New("snapshot: chunk index out of bounds")

	// ErrIncompleteSnapshot is returned when attempting to assemble an incomplete snapshot.
	ErrIncompleteSnapshot = errors.New("snapshot: not all chunks have been received")

	// ErrSnapshotNotFound is returned when querying a block hash with no active snapshot chunks.
	ErrSnapshotNotFound = errors.New("snapshot: no chunks found for target block hash")
)

// blockSnapshotState tracks the assembly of a single block's UTXO state snapshot.
type blockSnapshotState struct {
	totalChunks uint32
	chunks      map[uint32][]wire.UtxoWireEntry
}

// SnapshotAssembler manages the progressive accumulation and reconstruction
// of chunked state snapshots received over the P2P wire protocol.
type SnapshotAssembler struct {
	mu     sync.Mutex
	states map[[32]byte]*blockSnapshotState
}

// NewSnapshotAssembler constructs an empty SnapshotAssembler.
func NewSnapshotAssembler() *SnapshotAssembler {
	return &SnapshotAssembler{
		states: make(map[[32]byte]*blockSnapshotState),
	}
}

// AddChunk adds a received MsgSnapshot chunk into the assembler state.
// Returns (isComplete, error).
func (sa *SnapshotAssembler) AddChunk(msg *wire.MsgSnapshot) (bool, error) {
	if msg == nil {
		return false, errors.New("snapshot: nil MsgSnapshot")
	}

	sa.mu.Lock()
	defer sa.mu.Unlock()

	state, exists := sa.states[msg.BlockHash]
	if !exists {
		if msg.TotalChunks == 0 {
			return false, errors.New("snapshot: total chunks cannot be 0")
		}
		if msg.ChunkIndex >= msg.TotalChunks {
			return false, fmt.Errorf("%w: chunk %d >= total %d", ErrChunkIndexOutOfBounds, msg.ChunkIndex, msg.TotalChunks)
		}
		state = &blockSnapshotState{
			totalChunks: msg.TotalChunks,
			chunks:      make(map[uint32][]wire.UtxoWireEntry),
		}
		sa.states[msg.BlockHash] = state
	} else {
		if msg.TotalChunks != state.totalChunks {
			return false, fmt.Errorf("%w: expected %d, got %d", ErrTotalChunksMismatch, state.totalChunks, msg.TotalChunks)
		}
		if msg.ChunkIndex >= state.totalChunks {
			return false, fmt.Errorf("%w: chunk %d >= total %d", ErrChunkIndexOutOfBounds, msg.ChunkIndex, state.totalChunks)
		}
	}

	// Store copy of chunk entries
	copied := make([]wire.UtxoWireEntry, len(msg.Entries))
	copy(copied, msg.Entries)
	state.chunks[msg.ChunkIndex] = copied

	isComplete := uint32(len(state.chunks)) == state.totalChunks
	return isComplete, nil
}

// IsComplete returns true if all declared chunks for the block hash have been received.
func (sa *SnapshotAssembler) IsComplete(blockHash [32]byte) bool {
	sa.mu.Lock()
	defer sa.mu.Unlock()

	state, exists := sa.states[blockHash]
	if !exists {
		return false
	}
	return uint32(len(state.chunks)) == state.totalChunks
}

// Assemble concatenates all chunk entries in sequential index order (0 .. totalChunks-1).
// Upon successful assembly, internal chunk memory for the block hash is released.
func (sa *SnapshotAssembler) Assemble(blockHash [32]byte) ([]wire.UtxoWireEntry, error) {
	sa.mu.Lock()
	defer sa.mu.Unlock()

	state, exists := sa.states[blockHash]
	if !exists {
		return nil, ErrSnapshotNotFound
	}

	if uint32(len(state.chunks)) != state.totalChunks {
		return nil, fmt.Errorf("%w: received %d of %d chunks", ErrIncompleteSnapshot, len(state.chunks), state.totalChunks)
	}

	var totalEntries int
	for i := uint32(0); i < state.totalChunks; i++ {
		chunk, ok := state.chunks[i]
		if !ok {
			return nil, fmt.Errorf("%w: missing chunk %d", ErrIncompleteSnapshot, i)
		}
		totalEntries += len(chunk)
	}

	assembled := make([]wire.UtxoWireEntry, 0, totalEntries)
	for i := uint32(0); i < state.totalChunks; i++ {
		assembled = append(assembled, state.chunks[i]...)
	}

	delete(sa.states, blockHash)
	return assembled, nil
}

// Clear drops any partial snapshot state for a given block hash.
func (sa *SnapshotAssembler) Clear(blockHash [32]byte) {
	sa.mu.Lock()
	defer sa.mu.Unlock()
	delete(sa.states, blockHash)
}
