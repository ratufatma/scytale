package peer

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestAddrBookFiltering(t *testing.T) {
	// Without local peers allowed
	ab := NewAddrBook("", false)

	// Public routable IP
	if !ab.AddAddress("8.8.8.8:9001", "src") {
		t.Errorf("expected 8.8.8.8:9001 to be accepted")
	}

	// Loopback should be rejected
	if ab.AddAddress("127.0.0.1:9001", "src") {
		t.Errorf("expected 127.0.0.1:9001 to be rejected when allowLocalPeers is false")
	}

	// Private IP (192.168.1.1) should be rejected
	if ab.AddAddress("192.168.1.1:9001", "src") {
		t.Errorf("expected 192.168.1.1:9001 to be rejected when allowLocalPeers is false")
	}

	// With local peers allowed
	abLocal := NewAddrBook("", true)
	if !abLocal.AddAddress("127.0.0.1:9001", "src") {
		t.Errorf("expected 127.0.0.1:9001 to be accepted when allowLocalPeers is true")
	}
	if !abLocal.AddAddress("192.168.1.1:9001", "src") {
		t.Errorf("expected 192.168.1.1:9001 to be accepted when allowLocalPeers is true")
	}
	if !abLocal.AddAddress("node1:9001", "src") {
		t.Errorf("expected node1:9001 to be accepted when allowLocalPeers is true")
	}
}

func TestAddrBookSelectAddress(t *testing.T) {
	ab := NewAddrBook("", true)
	ab.AddAddress("127.0.0.1:9001", "src")
	ab.AddAddress("127.0.0.1:9002", "src")

	// Neither is connected
	selected, ok := ab.SelectAddressToDial(nil)
	if !ok || (selected != "127.0.0.1:9001" && selected != "127.0.0.1:9002") {
		t.Fatalf("expected valid candidate, got %q (ok: %v)", selected, ok)
	}

	// If 9001 is already connected, 9002 should be selected
	selected, ok = ab.SelectAddressToDial([]string{"127.0.0.1:9001"})
	if !ok || selected != "127.0.0.1:9002" {
		t.Fatalf("expected 127.0.0.1:9002, got %q (ok: %v)", selected, ok)
	}

	// If both connected, none should be selected
	_, ok = ab.SelectAddressToDial([]string{"127.0.0.1:9001", "127.0.0.1:9002"})
	if ok {
		t.Fatalf("expected no address when all are connected")
	}
}

func TestAddrBookPersistence(t *testing.T) {
	dir, err := os.MkdirTemp("", "addrbook_test")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	path := filepath.Join(dir, "peers.json")

	ab1 := NewAddrBook(path, true)
	ab1.AddAddress("127.0.0.1:9001", "static")
	ab1.AddAddress("127.0.0.1:9002", "static")
	ab1.MarkSuccess("127.0.0.1:9001")

	if err := ab1.Save(); err != nil {
		t.Fatalf("failed to save addrbook: %v", err)
	}

	// Load from new instance
	ab2 := NewAddrBook(path, true)
	if ab2.Size() != 2 {
		t.Fatalf("expected 2 addresses loaded, got %d", ab2.Size())
	}

	ab2.mu.RLock()
	ka := ab2.addresses["127.0.0.1:9001"]
	ab2.mu.RUnlock()

	if ka == nil || ka.LastSuccess.IsZero() {
		t.Errorf("expected 127.0.0.1:9001 to retain LastSuccess")
	}
}

func TestAddrBookConcurrentAccess(t *testing.T) {
	ab := NewAddrBook("", true)
	var wg sync.WaitGroup

	for i := 0; i < 20; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			addr := "127.0.0.1:900" + string(rune('0'+idx%10))
			ab.AddAddress(addr, "test")
			ab.MarkAttempt(addr)
			ab.SelectAddressToDial([]string{})
			ab.GetAddresses(5)
			time.Sleep(1 * time.Millisecond)
			ab.MarkSuccess(addr)
		}(i)
	}

	wg.Wait()
	if ab.Size() == 0 {
		t.Errorf("expected addresses recorded under concurrency")
	}
}
