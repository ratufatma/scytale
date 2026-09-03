// Package bridge defines the ConsensusBridge interface and a MockConsensusBridge
// for testing without a live Rust node binary.
package bridge

import (
	"errors"
	"sync"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// ConsensusBridge is the explicit IPC boundary between the Go P2P layer and the Rust Core.
// The Go network daemon MUST NOT perform consensus validation; all raw block and transaction
// bytes are forwarded through this interface for the Rust engine to evaluate.
type ConsensusBridge interface {
	// SubmitBlock forwards raw canonical block bytes to the Rust consensus engine for validation
	// and potential chain tip advancement. Returns an error if validation fails.
	SubmitBlock(blockBytes []byte) error

	// SubmitTransaction forwards a raw transaction to the Rust mempool admission pipeline.
	SubmitTransaction(txBytes []byte) error

	// GetBlockLocator returns the local chain's block locator hashes (tip-to-genesis, exponential
	// spacing) for IBD sync negotiation.
	GetBlockLocator() ([][32]byte, error)

	// GetCanonicalHashes returns all canonical block hashes in ascending order from genesis to tip.
	GetCanonicalHashes() ([][32]byte, error)

	// GetBlockByHash retrieves raw canonical block bytes for the given block hash.
	// Returns ErrNotFound if the block is not in storage.
	GetBlockByHash(hash [32]byte) ([]byte, error)

	// GetTransactionByHash retrieves raw transaction bytes from the mempool or confirmed storage.
	// Returns ErrNotFound if the transaction is not available.
	GetTransactionByHash(hash [32]byte) ([]byte, error)

	// ExportSnapshotChunk requests a chunk of the authenticated UTXO state snapshot for the given block hash.
	ExportSnapshotChunk(blockHash [32]byte, chunkIndex uint32, chunkSize uint32) (*wire.MsgSnapshot, error)

	// ApplySnapshot applies a complete reconstructed UTXO state snapshot for the given block hash.
	ApplySnapshot(blockHash [32]byte, entries []wire.UtxoWireEntry) (int, error)

	// UpdatePeerCount notifies the node of the current connected peer count.
	UpdatePeerCount(count int) error
}

// ErrNotFound is returned when a requested block or transaction is not available locally.
var ErrNotFound = errors.New("bridge: object not found")

// ─────────────────────────────────────────────────────────────────────────────
// MockConsensusBridge — in-memory test double, safe for concurrent use.
// ─────────────────────────────────────────────────────────────────────────────

// MockConsensusBridge is a thread-safe in-memory implementation of ConsensusBridge
// for unit and integration tests that do not require a running Rust binary.
type MockConsensusBridge struct {
	mu           sync.RWMutex
	blocks       map[[32]byte][]byte
	transactions map[[32]byte][]byte
	snapshots    map[[32]byte][]wire.UtxoWireEntry
	locator      [][32]byte
	canonical    [][32]byte
	// SubmitBlockErr, if non-nil, is returned by SubmitBlock to simulate rejection.
	SubmitBlockErr error
	// SubmitTxErr, if non-nil, is returned by SubmitTransaction to simulate rejection.
	SubmitTxErr error
	// ApplySnapshotErr, if non-nil, is returned by ApplySnapshot to simulate rejection.
	ApplySnapshotErr error
	// Submitted tracks all bytes forwarded via SubmitBlock for test assertions.
	SubmittedBlocks [][]byte
	// SubmittedTxs tracks all bytes forwarded via SubmitTransaction for test assertions.
	SubmittedTxs [][]byte
	// AppliedSnapshots tracks all snapshots applied.
	AppliedSnapshots map[[32]byte][]wire.UtxoWireEntry
}

// NewMockConsensusBridge creates an empty MockConsensusBridge.
func NewMockConsensusBridge() *MockConsensusBridge {
	return &MockConsensusBridge{
		blocks:           make(map[[32]byte][]byte),
		transactions:     make(map[[32]byte][]byte),
		snapshots:        make(map[[32]byte][]wire.UtxoWireEntry),
		AppliedSnapshots: make(map[[32]byte][]wire.UtxoWireEntry),
	}
}

// AddBlock seeds the mock with a block to serve via GetBlockByHash.
func (m *MockConsensusBridge) AddBlock(hash [32]byte, data []byte) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.blocks[hash] = data
}

// AddTransaction seeds the mock with a transaction to serve via GetTransactionByHash.
func (m *MockConsensusBridge) AddTransaction(hash [32]byte, data []byte) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.transactions[hash] = data
}

// SetLocator seeds the block locator returned by GetBlockLocator.
func (m *MockConsensusBridge) SetLocator(locator [][32]byte) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.locator = locator
}

// SetCanonicalHashes seeds the canonical hashes returned by GetCanonicalHashes.
func (m *MockConsensusBridge) SetCanonicalHashes(hashes [][32]byte) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.canonical = hashes
}

// SubmitBlock implements ConsensusBridge.
func (m *MockConsensusBridge) SubmitBlock(blockBytes []byte) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.SubmitBlockErr != nil {
		return m.SubmitBlockErr
	}
	cp := make([]byte, len(blockBytes))
	copy(cp, blockBytes)
	m.SubmittedBlocks = append(m.SubmittedBlocks, cp)
	return nil
}

// SubmitTransaction implements ConsensusBridge.
func (m *MockConsensusBridge) SubmitTransaction(txBytes []byte) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.SubmitTxErr != nil {
		return m.SubmitTxErr
	}
	cp := make([]byte, len(txBytes))
	copy(cp, txBytes)
	m.SubmittedTxs = append(m.SubmittedTxs, cp)
	return nil
}

// GetBlockLocator implements ConsensusBridge.
func (m *MockConsensusBridge) GetBlockLocator() ([][32]byte, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([][32]byte, len(m.locator))
	copy(result, m.locator)
	return result, nil
}

// GetCanonicalHashes implements ConsensusBridge.
func (m *MockConsensusBridge) GetCanonicalHashes() ([][32]byte, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([][32]byte, len(m.canonical))
	copy(result, m.canonical)
	return result, nil
}

// GetBlockByHash implements ConsensusBridge.
func (m *MockConsensusBridge) GetBlockByHash(hash [32]byte) ([]byte, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	data, ok := m.blocks[hash]
	if !ok {
		return nil, ErrNotFound
	}
	cp := make([]byte, len(data))
	copy(cp, data)
	return cp, nil
}

// GetTransactionByHash implements ConsensusBridge.
func (m *MockConsensusBridge) GetTransactionByHash(hash [32]byte) ([]byte, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	data, ok := m.transactions[hash]
	if !ok {
		return nil, ErrNotFound
	}
	cp := make([]byte, len(data))
	copy(cp, data)
	return cp, nil
}

// SetSnapshotEntries seeds the mock with UTXO snapshot entries for a block hash.
func (m *MockConsensusBridge) SetSnapshotEntries(hash [32]byte, entries []wire.UtxoWireEntry) {
	m.mu.Lock()
	defer m.mu.Unlock()
	cp := make([]wire.UtxoWireEntry, len(entries))
	copy(cp, entries)
	m.snapshots[hash] = cp
}

// ExportSnapshotChunk implements ConsensusBridge.
func (m *MockConsensusBridge) ExportSnapshotChunk(blockHash [32]byte, chunkIndex uint32, chunkSize uint32) (*wire.MsgSnapshot, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	entries, ok := m.snapshots[blockHash]
	if !ok {
		return nil, ErrNotFound
	}

	if chunkSize == 0 {
		chunkSize = wire.MaxSnapshotChunkEntries
	}

	total := uint32(len(entries))
	totalChunks := uint32(1)
	if total > 0 {
		totalChunks = (total + chunkSize - 1) / chunkSize
	}

	start := chunkIndex * chunkSize
	if start >= total {
		return &wire.MsgSnapshot{
			BlockHash:   blockHash,
			ChunkIndex:  chunkIndex,
			TotalChunks: totalChunks,
			Entries:     nil,
		}, nil
	}

	end := start + chunkSize
	if end > total {
		end = total
	}

	chunkEntries := make([]wire.UtxoWireEntry, end-start)
	copy(chunkEntries, entries[start:end])

	return &wire.MsgSnapshot{
		BlockHash:   blockHash,
		ChunkIndex:  chunkIndex,
		TotalChunks: totalChunks,
		Entries:     chunkEntries,
	}, nil
}

// ApplySnapshot implements ConsensusBridge.
func (m *MockConsensusBridge) ApplySnapshot(blockHash [32]byte, entries []wire.UtxoWireEntry) (int, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.ApplySnapshotErr != nil {
		return 0, m.ApplySnapshotErr
	}

	cp := make([]wire.UtxoWireEntry, len(entries))
	copy(cp, entries)
	m.AppliedSnapshots[blockHash] = cp
	return len(entries), nil
}

// UpdatePeerCount implements ConsensusBridge.
func (m *MockConsensusBridge) UpdatePeerCount(count int) error {
	return nil
}
