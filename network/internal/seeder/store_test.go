package seeder

import (
	"net"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestStore_IsGood(t *testing.T) {
	medianHeight := uint64(1000)

	// Case 1: Fresh, high success ratio, recent height -> GOOD
	rec1 := &NodeRecord{
		IP:            net.ParseIP("192.168.1.10"),
		Port:          9001,
		LastSuccess:   time.Now().Add(-15 * time.Minute),
		LastAttempt:   time.Now().Add(-15 * time.Minute),
		SuccessCount:  4,
		TotalAttempts: 5,
		BestHeight:    950,
	}
	if !rec1.IsGood(medianHeight) {
		t.Errorf("expected rec1 to be Good, got false")
	}

	// Case 2: Stale node (> 2 hours since last success) -> BAD
	rec2 := &NodeRecord{
		IP:            net.ParseIP("192.168.1.11"),
		Port:          9001,
		LastSuccess:   time.Now().Add(-3 * time.Hour),
		LastAttempt:   time.Now().Add(-10 * time.Minute),
		SuccessCount:  10,
		TotalAttempts: 10,
		BestHeight:    1000,
	}
	if rec2.IsGood(medianHeight) {
		t.Errorf("expected stale rec2 to be rejected, got true")
	}

	// Case 3: Low success ratio (< 70% after 3 attempts) -> BAD
	rec3 := &NodeRecord{
		IP:            net.ParseIP("192.168.1.12"),
		Port:          9001,
		LastSuccess:   time.Now().Add(-10 * time.Minute),
		LastAttempt:   time.Now().Add(-5 * time.Minute),
		SuccessCount:  2,
		TotalAttempts: 4, // 50% < 70%
		BestHeight:    1000,
	}
	if rec3.IsGood(medianHeight) {
		t.Errorf("expected low-ratio rec3 to be rejected, got true")
	}

	// Case 4: Lagging height (> 288 blocks behind median) -> BAD
	rec4 := &NodeRecord{
		IP:            net.ParseIP("192.168.1.13"),
		Port:          9001,
		LastSuccess:   time.Now().Add(-10 * time.Minute),
		LastAttempt:   time.Now().Add(-10 * time.Minute),
		SuccessCount:  5,
		TotalAttempts: 5,
		BestHeight:    700, // 1000 - 288 = 712; 700 < 712 -> lags behind
	}
	if rec4.IsGood(medianHeight) {
		t.Errorf("expected lagging rec4 to be rejected, got true")
	}

	// Case 5: Zero last success -> BAD
	rec5 := &NodeRecord{
		IP:            net.ParseIP("192.168.1.14"),
		Port:          9001,
		SuccessCount:  0,
		TotalAttempts: 2,
		BestHeight:    0,
	}
	if rec5.IsGood(medianHeight) {
		t.Errorf("expected unprobed rec5 to be rejected, got true")
	}
}

func TestStore_AntiSybilSubnetLimit(t *testing.T) {
	store := NewStore()

	// Add 5 nodes in the same /24 IPv4 subnet (198.51.100.0/24)
	for i := 1; i <= 5; i++ {
		ip := net.ParseIP("198.51.100." + string(rune('0'+i)))
		store.AddNode(ip, 9001)
		store.RecordAttempt(ip, 9001)
		store.RecordSuccess(ip, 9001, 1, 1, 500)
	}

	// Add 2 nodes in a different /24 subnet (203.0.113.0/24)
	for i := 1; i <= 2; i++ {
		ip := net.ParseIP("203.0.113." + string(rune('0'+i)))
		store.AddNode(ip, 9001)
		store.RecordAttempt(ip, 9001)
		store.RecordSuccess(ip, 9001, 1, 1, 500)
	}

	goodIPs := store.GetGoodNodes()

	// Count IPs per /24 subnet
	subnetCounts := make(map[string]int)
	for _, ip := range goodIPs {
		sub := SubnetKey(ip)
		subnetCounts[sub]++
	}

	if subnetCounts["ipv4:198.51.100.0/24"] > 2 {
		t.Fatalf("anti-Sybil violated: expected at most 2 IPs for 198.51.100.0/24, got %d",
			subnetCounts["ipv4:198.51.100.0/24"])
	}

	if subnetCounts["ipv4:203.0.113.0/24"] != 2 {
		t.Fatalf("expected 2 IPs for 203.0.113.0/24, got %d",
			subnetCounts["ipv4:203.0.113.0/24"])
	}

	if len(goodIPs) != 4 {
		t.Fatalf("expected total 4 good IPs (2 from each /24), got %d", len(goodIPs))
	}
}

func TestStore_SaveAndLoadAtomic(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "seeder_test_*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer func() { _ = os.RemoveAll(tmpDir) }()

	dataPath := filepath.Join(tmpDir, "nodes.json")

	store1 := NewStore()
	ip1 := net.ParseIP("192.0.2.1")
	ip2 := net.ParseIP("192.0.2.2")

	store1.AddNode(ip1, 9001)
	store1.RecordAttempt(ip1, 9001)
	store1.RecordSuccess(ip1, 9001, 1, 1, 100)

	store1.AddNode(ip2, 9002)
	store1.RecordAttempt(ip2, 9002)
	store1.RecordFailure(ip2, 9002)

	if err := store1.SaveToFile(dataPath); err != nil {
		t.Fatalf("failed to save store: %v", err)
	}

	// Load into fresh store
	store2 := NewStore()
	if err := store2.LoadFromFile(dataPath); err != nil {
		t.Fatalf("failed to load store: %v", err)
	}

	if store2.Size() != 2 {
		t.Fatalf("expected 2 nodes loaded, got %d", store2.Size())
	}

	rec1, ok := store2.GetNode(ip1, 9001)
	if !ok || rec1.SuccessCount != 1 || rec1.BestHeight != 100 {
		t.Errorf("loaded node 1 did not match expected values: %+v", rec1)
	}

	rec2, ok := store2.GetNode(ip2, 9002)
	if !ok || rec2.FailStreak != 1 {
		t.Errorf("loaded node 2 did not match expected values: %+v", rec2)
	}
}

func TestStore_ConcurrentAccess(t *testing.T) {
	store := NewStore()
	var wg sync.WaitGroup

	numGoroutines := 30
	opsPerGoroutine := 100

	for g := 0; g < numGoroutines; g++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			ip := net.ParseIP("10.0.0." + string(rune('1'+id%9)))
			port := uint16(9000 + id%5)

			for i := 0; i < opsPerGoroutine; i++ {
				store.AddNode(ip, port)
				store.RecordAttempt(ip, port)
				if i%2 == 0 {
					store.RecordSuccess(ip, port, 1, 1, uint64(i*10))
				} else {
					store.RecordFailure(ip, port)
				}
				_ = store.GetGoodNodes()
				_ = store.MedianHeight()
				_ = store.Size()
			}
		}(g)
	}

	wg.Wait()
}
