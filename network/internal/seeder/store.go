package seeder

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"sync"
	"time"
)

// NodeRecord tracks connection history, metrics, and health status for a single peer.
type NodeRecord struct {
	IP            net.IP    `json:"ip"`
	Port          uint16    `json:"port"`
	ProtocolVer   uint32    `json:"protocol_ver"`
	Services      uint64    `json:"services"`
	BestHeight    uint64    `json:"best_height"`
	LastSuccess   time.Time `json:"last_success"`
	LastAttempt   time.Time `json:"last_attempt"`
	SuccessCount  int       `json:"success_count"`
	TotalAttempts int       `json:"total_attempts"`
	FailStreak    int       `json:"fail_streak"`
}

// IsGood checks whether the node meets Scytale DNS seeder quality invariants.
// A node is considered "Good" if:
// 1. It had a successful probe within the last 2 hours.
// 2. Its success ratio is at least 70% if probed 3 or more times (or at least 1 success if < 3).
// 3. Its best height does not lag more than 288 blocks behind the network median height.
func (r *NodeRecord) IsGood(medianHeight uint64) bool {
	if r.LastSuccess.IsZero() {
		return false
	}

	// 1. Freshness: Successful probe within last 2 hours.
	if time.Since(r.LastSuccess) > 2*time.Hour {
		return false
	}

	// 2. Reliability: Success ratio >= 70% if TotalAttempts >= 3, or at least 1 success if < 3.
	if r.TotalAttempts >= 3 {
		// Integer arithmetic: SuccessCount / TotalAttempts >= 7 / 10
		if r.SuccessCount*10 < r.TotalAttempts*7 {
			return false
		}
	} else if r.SuccessCount <= 0 {
		return false
	}

	// 3. Height: BestHeight within 288 blocks of median network height.
	if medianHeight > 288 {
		if r.BestHeight < medianHeight-288 {
			return false
		}
	}

	return true
}

// SubnetKey computes an anti-Sybil subnet identifier:
// - IPv4: /24 prefix (first 3 bytes)
// - IPv6: /48 prefix (first 6 bytes)
func SubnetKey(ip net.IP) string {
	if ipv4 := ip.To4(); ipv4 != nil {
		mask := net.CIDRMask(24, 32)
		network := ipv4.Mask(mask)
		return fmt.Sprintf("ipv4:%s/24", network.String())
	}
	ipv6 := ip.To16()
	if ipv6 != nil {
		mask := net.CIDRMask(48, 128)
		network := ipv6.Mask(mask)
		return fmt.Sprintf("ipv6:%s/48", network.String())
	}
	return "unknown"
}

// Store maintains an in-memory, thread-safe ledger of known Scytale nodes and reputation history.
type Store struct {
	mu    sync.RWMutex
	nodes map[string]*NodeRecord // key: host:port
}

// NewStore initializes a new empty Store.
func NewStore() *Store {
	return &Store{
		nodes: make(map[string]*NodeRecord),
	}
}

// key helper returns canonical host:port string.
func nodeKey(ip net.IP, port uint16) string {
	return net.JoinHostPort(ip.String(), strconv.Itoa(int(port)))
}

// AddNode adds a new node record to the store if it does not already exist.
// Returns true if the node was newly added, false if already present.
func (s *Store) AddNode(ip net.IP, port uint16) bool {
	if ip == nil || ip.IsUnspecified() || ip.IsMulticast() {
		return false
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	k := nodeKey(ip, port)
	if _, exists := s.nodes[k]; exists {
		return false
	}

	s.nodes[k] = &NodeRecord{
		IP:   ip,
		Port: port,
	}
	return true
}

// RecordAttempt records that a probe attempt has commenced for the specified node.
func (s *Store) RecordAttempt(ip net.IP, port uint16) {
	s.mu.Lock()
	defer s.mu.Unlock()

	k := nodeKey(ip, port)
	rec, exists := s.nodes[k]
	if !exists {
		rec = &NodeRecord{
			IP:   ip,
			Port: port,
		}
		s.nodes[k] = rec
	}

	rec.LastAttempt = time.Now()
	rec.TotalAttempts++
}

// RecordSuccess marks a probe attempt as successful, updating the node's metrics and height.
func (s *Store) RecordSuccess(ip net.IP, port uint16, protocolVer uint32, services uint64, bestHeight uint64) {
	s.mu.Lock()
	defer s.mu.Unlock()

	k := nodeKey(ip, port)
	rec, exists := s.nodes[k]
	if !exists {
		rec = &NodeRecord{
			IP:   ip,
			Port: port,
		}
		s.nodes[k] = rec
	}

	now := time.Now()
	rec.LastSuccess = now
	rec.LastAttempt = now
	rec.SuccessCount++
	rec.FailStreak = 0
	rec.ProtocolVer = protocolVer
	rec.Services = services
	if bestHeight > rec.BestHeight {
		rec.BestHeight = bestHeight
	}
}

// RecordFailure records an unsuccessful probe attempt, increasing the node's fail streak.
func (s *Store) RecordFailure(ip net.IP, port uint16) {
	s.mu.Lock()
	defer s.mu.Unlock()

	k := nodeKey(ip, port)
	rec, exists := s.nodes[k]
	if !exists {
		rec = &NodeRecord{
			IP:   ip,
			Port: port,
		}
		s.nodes[k] = rec
	}

	rec.LastAttempt = time.Now()
	rec.FailStreak++
}

// MedianHeight calculates the median BestHeight among all nodes that have had at least one successful probe.
func (s *Store) MedianHeight() uint64 {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var heights []uint64
	for _, rec := range s.nodes {
		if rec.SuccessCount > 0 && rec.BestHeight > 0 {
			heights = append(heights, rec.BestHeight)
		}
	}

	if len(heights) == 0 {
		return 0
	}

	sort.Slice(heights, func(i, j int) bool {
		return heights[i] < heights[j]
	})

	mid := len(heights) / 2
	if len(heights)%2 == 0 {
		return (heights[mid-1] + heights[mid]) / 2
	}
	return heights[mid]
}

// GetGoodNodes returns a list of healthy peer IP addresses filtered by reputation and anti-Sybil rules.
// Anti-Sybil rule: At most 2 IP addresses per /24 subnet (IPv4) or /48 subnet (IPv6) are returned.
func (s *Store) GetGoodNodes() []net.IP {
	medianH := s.MedianHeight()

	s.mu.RLock()
	defer s.mu.RUnlock()

	var good []*NodeRecord
	for _, rec := range s.nodes {
		if rec.IsGood(medianH) {
			good = append(good, rec)
		}
	}

	// Sort candidate nodes by most recent successful probe descending
	sort.Slice(good, func(i, j int) bool {
		return good[i].LastSuccess.After(good[j].LastSuccess)
	})

	subnetCounts := make(map[string]int)
	var selected []net.IP

	for _, rec := range good {
		sub := SubnetKey(rec.IP)
		if subnetCounts[sub] < 2 {
			selected = append(selected, rec.IP)
			subnetCounts[sub]++
		}
	}

	return selected
}

// GetAllNodes returns a slice of all stored node records.
func (s *Store) GetAllNodes() []*NodeRecord {
	s.mu.RLock()
	defer s.mu.RUnlock()

	res := make([]*NodeRecord, 0, len(s.nodes))
	for _, rec := range s.nodes {
		cp := *rec
		res = append(res, &cp)
	}
	return res
}

// GetNode returns the node record for a given IP and port, if found.
func (s *Store) GetNode(ip net.IP, port uint16) (*NodeRecord, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rec, ok := s.nodes[nodeKey(ip, port)]
	if !ok {
		return nil, false
	}
	cp := *rec
	return &cp, true
}

// Size returns the total count of known nodes in the store.
func (s *Store) Size() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.nodes)
}

// SaveToFile atomically writes the store state to a JSON file on disk.
func (s *Store) SaveToFile(path string) error {
	s.mu.RLock()
	nodes := make([]*NodeRecord, 0, len(s.nodes))
	for _, rec := range s.nodes {
		nodes = append(nodes, rec)
	}
	s.mu.RUnlock()

	data, err := json.MarshalIndent(nodes, "", "  ")
	if err != nil {
		return fmt.Errorf("seeder store: marshal error: %w", err)
	}

	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("seeder store: mkdir error: %w", err)
	}

	tmpFile := path + ".tmp"
	f, err := os.OpenFile(tmpFile, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0644)
	if err != nil {
		return fmt.Errorf("seeder store: create tmp file error: %w", err)
	}

	if _, err := f.Write(data); err != nil {
		_ = f.Close()
		_ = os.Remove(tmpFile)
		return fmt.Errorf("seeder store: write tmp file error: %w", err)
	}

	if err := f.Sync(); err != nil {
		_ = f.Close()
		_ = os.Remove(tmpFile)
		return fmt.Errorf("seeder store: sync tmp file error: %w", err)
	}

	if err := f.Close(); err != nil {
		_ = os.Remove(tmpFile)
		return fmt.Errorf("seeder store: close tmp file error: %w", err)
	}

	if err := os.Rename(tmpFile, path); err != nil {
		_ = os.Remove(tmpFile)
		return fmt.Errorf("seeder store: atomic rename error: %w", err)
	}

	return nil
}

// LoadFromFile restores store state from a JSON file on disk.
// Returns nil if the file does not exist (clean start).
func (s *Store) LoadFromFile(path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return fmt.Errorf("seeder store: read file error: %w", err)
	}

	var nodes []*NodeRecord
	if err := json.Unmarshal(data, &nodes); err != nil {
		return fmt.Errorf("seeder store: unmarshal error: %w", err)
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	for _, rec := range nodes {
		if rec != nil && rec.IP != nil {
			k := nodeKey(rec.IP, rec.Port)
			s.nodes[k] = rec
		}
	}

	return nil
}
