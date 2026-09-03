// Package peer implements the Scytale P2P peer connection lifecycle, handshake,
// and address book peer discovery.
package peer

import (
	"encoding/json"
	"math/rand"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"sync"
	"time"
)

// KnownAddress records metadata about a peer network address.
type KnownAddress struct {
	Addr        string    `json:"addr"`
	Src         string    `json:"src"`
	Attempts    int       `json:"attempts"`
	LastAttempt time.Time `json:"last_attempt"`
	LastSuccess time.Time `json:"last_success"`
	LastSeen    time.Time `json:"last_seen"`
}

// AddrBook manages a thread-safe registry of known network peer addresses.
type AddrBook struct {
	mu              sync.RWMutex
	filePath        string
	allowLocalPeers bool
	addresses       map[string]*KnownAddress
	rng             *rand.Rand
}

// NewAddrBook creates a new AddrBook instance, optionally persisting to filePath.
func NewAddrBook(filePath string, allowLocalPeers bool) *AddrBook {
	ab := &AddrBook{
		filePath:        filePath,
		allowLocalPeers: allowLocalPeers,
		addresses:       make(map[string]*KnownAddress),
		rng:             rand.New(rand.NewSource(time.Now().UnixNano())),
	}
	if filePath != "" {
		_ = ab.Load()
	}
	return ab
}

// isPrivateIP checks if an IP belongs to RFC1918 / RFC4193 private network blocks.
func isPrivateIP(ip net.IP) bool {
	if ip4 := ip.To4(); ip4 != nil {
		// 10.0.0.0/8
		if ip4[0] == 10 {
			return true
		}
		// 172.16.0.0/12
		if ip4[0] == 172 && ip4[1] >= 16 && ip4[1] <= 31 {
			return true
		}
		// 192.168.0.0/16
		if ip4[0] == 192 && ip4[1] == 168 {
			return true
		}
		return false
	}

	// IPv6 ULA (fc00::/7) or Link-local (fe80::/10)
	if len(ip) == net.IPv6len {
		if ip[0]&0xfe == 0xfc || (ip[0] == 0xfe && ip[1]&0xc0 == 0x80) {
			return true
		}
	}
	return false
}

// IsRoutable determines if a host:port string is a valid, connectable address.
func IsRoutable(addrStr string, allowLocal bool) bool {
	host, portStr, err := net.SplitHostPort(addrStr)
	if err != nil {
		return false
	}

	port, err := strconv.Atoi(portStr)
	if err != nil || port <= 0 || port > 65535 {
		return false
	}

	ip := net.ParseIP(host)
	if ip == nil {
		// Named host (e.g. docker container or localhost)
		if host == "localhost" && !allowLocal {
			return false
		}
		return len(host) > 0
	}

	// Reject unspecified (0.0.0.0 / ::) and multicast
	if ip.IsUnspecified() || ip.IsMulticast() {
		return false
	}

	// Reject loopback unless allowed
	if ip.IsLoopback() {
		return allowLocal
	}

	// Reject private LAN unless allowed
	if isPrivateIP(ip) {
		return allowLocal
	}

	return true
}

// AddAddress adds or updates an address in the book.
// Returns true if a new address was inserted.
func (ab *AddrBook) AddAddress(addrStr string, src string) bool {
	if !IsRoutable(addrStr, ab.allowLocalPeers) {
		return false
	}

	ab.mu.Lock()
	defer ab.mu.Unlock()

	now := time.Now()
	if existing, exists := ab.addresses[addrStr]; exists {
		if now.After(existing.LastSeen) {
			existing.LastSeen = now
		}
		return false
	}

	ab.addresses[addrStr] = &KnownAddress{
		Addr:     addrStr,
		Src:      src,
		LastSeen: now,
	}
	return true
}

// AddAddresses adds multiple addresses to the address book.
func (ab *AddrBook) AddAddresses(addrStrs []string, src string) int {
	added := 0
	for _, a := range addrStrs {
		if ab.AddAddress(a, src) {
			added++
		}
	}
	return added
}

// GetAddresses returns up to max random known addresses for gossip dissemination.
func (ab *AddrBook) GetAddresses(max int) []string {
	ab.mu.RLock()
	defer ab.mu.RUnlock()

	if len(ab.addresses) == 0 || max <= 0 {
		return nil
	}

	all := make([]string, 0, len(ab.addresses))
	for addr := range ab.addresses {
		all = append(all, addr)
	}

	if len(all) <= max {
		return all
	}

	// Partial Fisher-Yates shuffle
	shuffled := make([]string, len(all))
	copy(shuffled, all)
	for i := 0; i < max; i++ {
		j := i + ab.rng.Intn(len(shuffled)-i)
		shuffled[i], shuffled[j] = shuffled[j], shuffled[i]
	}

	return shuffled[:max]
}

// SelectAddressToDial selects a suitable address to dial that is not currently connected.
// Applies exponential backoff based on previous failed attempts.
func (ab *AddrBook) SelectAddressToDial(connected []string) (string, bool) {
	ab.mu.Lock()
	defer ab.mu.Unlock()

	connectedSet := make(map[string]struct{}, len(connected))
	for _, c := range connected {
		connectedSet[c] = struct{}{}
	}

	now := time.Now()
	var bestAddr string
	var bestScore int64 = -1

	for addr, ka := range ab.addresses {
		if _, isConnected := connectedSet[addr]; isConnected {
			continue
		}

		// Calculate backoff: min(2 hours, 5s * 2^attempts)
		var backoff time.Duration = 0
		if ka.Attempts > 0 {
			multiplier := 1 << ka.Attempts
			if multiplier > 1440 { // ~2 hours max
				multiplier = 1440
			}
			backoff = time.Duration(multiplier) * 5 * time.Second
		}

		if ka.Attempts > 0 && now.Sub(ka.LastAttempt) < backoff {
			continue
		}

		// Prioritize addresses never attempted or least attempted
		score := int64(10000 - ka.Attempts*100)
		if ka.Attempts == 0 {
			score += 5000
		}
		if score > bestScore {
			bestScore = score
			bestAddr = addr
		}
	}

	if bestAddr != "" {
		return bestAddr, true
	}
	return "", false
}

// MarkAttempt marks a dial attempt for the address.
func (ab *AddrBook) MarkAttempt(addrStr string) {
	ab.mu.Lock()
	defer ab.mu.Unlock()

	if ka, ok := ab.addresses[addrStr]; ok {
		ka.Attempts++
		ka.LastAttempt = time.Now()
	}
}

// MarkSuccess marks a successful connection to the address, resetting attempts.
func (ab *AddrBook) MarkSuccess(addrStr string) {
	ab.mu.Lock()
	defer ab.mu.Unlock()

	now := time.Now()
	if ka, ok := ab.addresses[addrStr]; ok {
		ka.Attempts = 0
		ka.LastSuccess = now
		ka.LastSeen = now
	} else {
		ab.addresses[addrStr] = &KnownAddress{
			Addr:        addrStr,
			LastSuccess: now,
			LastSeen:    now,
		}
	}
}

// MarkFailed marks a failed connection attempt.
func (ab *AddrBook) MarkFailed(addrStr string) {
	ab.MarkAttempt(addrStr)
}

// Size returns the total count of known addresses in the book.
func (ab *AddrBook) Size() int {
	ab.mu.RLock()
	defer ab.mu.RUnlock()
	return len(ab.addresses)
}

// Save serializes the address book to disk as JSON atomically.
func (ab *AddrBook) Save() error {
	if ab.filePath == "" {
		return nil
	}

	ab.mu.RLock()
	data, err := json.MarshalIndent(ab.addresses, "", "  ")
	ab.mu.RUnlock()
	if err != nil {
		return err
	}

	dir := filepath.Dir(ab.filePath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	tmpFile := ab.filePath + ".tmp"
	if err := os.WriteFile(tmpFile, data, 0600); err != nil {
		return err
	}

	return os.Rename(tmpFile, ab.filePath)
}

// Load reads previously stored addresses from disk.
func (ab *AddrBook) Load() error {
	if ab.filePath == "" {
		return nil
	}

	data, err := os.ReadFile(ab.filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	var loaded map[string]*KnownAddress
	if err := json.Unmarshal(data, &loaded); err != nil {
		return err
	}

	ab.mu.Lock()
	defer ab.mu.Unlock()

	for k, v := range loaded {
		if IsRoutable(k, ab.allowLocalPeers) {
			ab.addresses[k] = v
		}
	}

	return nil
}
