// Package bridge defines the ConsensusBridge interface and a MockConsensusBridge
// for testing without a live Rust node binary.
package bridge

import (
	"errors"
	"sync"
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

	// GetBlockByHash retrieves raw canonical block bytes for the given block hash.
	// Returns ErrNotFound if the block is not in storage.
	GetBlockByHash(hash [32]byte) ([]byte, error)

	// GetTransactionByHash retrieves raw transaction bytes from the mempool or confirmed storage.
	// Returns ErrNotFound if the transaction is not available.
	GetTransactionByHash(hash [32]byte) ([]byte, error)
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
	locator      [][32]byte
	// SubmitBlockErr, if non-nil, is returned by SubmitBlock to simulate rejection.
	SubmitBlockErr error
	// SubmitTxErr, if non-nil, is returned by SubmitTransaction to simulate rejection.
	SubmitTxErr error
	// Submitted tracks all bytes forwarded via SubmitBlock for test assertions.
	SubmittedBlocks [][]byte
	// SubmittedTxs tracks all bytes forwarded via SubmitTransaction for test assertions.
	SubmittedTxs [][]byte
}

// NewMockConsensusBridge creates an empty MockConsensusBridge.
func NewMockConsensusBridge() *MockConsensusBridge {
	return &MockConsensusBridge{
		blocks:       make(map[[32]byte][]byte),
		transactions: make(map[[32]byte][]byte),
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
