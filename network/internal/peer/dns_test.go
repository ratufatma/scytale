package peer

import (
	"errors"
	"net"
	"testing"
)

func TestResolveDNSSeeds_MockLookup(t *testing.T) {
	mockLookup := func(host string) ([]net.IP, error) {
		switch host {
		case "seed1.scytale.org":
			return []net.IP{
				net.ParseIP("192.0.2.1"),
				net.ParseIP("192.0.2.2"),
				net.ParseIP("192.0.2.1"), // duplicate
			}, nil
		case "seed2.scytale.org":
			return []net.IP{
				net.ParseIP("2001:db8::1"),
			}, nil
		case "failing.scytale.org":
			return nil, errors.New("lookup timeout")
		default:
			return nil, errors.New("unknown host")
		}
	}

	seeds := []string{
		"seed1.scytale.org",
		"seed2.scytale.org:9005",
		"failing.scytale.org",
		"198.51.100.42:9003",
		"",
	}

	addrs := ResolveDNSSeeds(seeds, 9001, mockLookup)

	expectedMap := map[string]bool{
		"192.0.2.1:9001":      true,
		"192.0.2.2:9001":      true,
		"[2001:db8::1]:9005":  true,
		"198.51.100.42:9003":  true,
	}

	if len(addrs) != len(expectedMap) {
		t.Fatalf("expected %d resolved addresses, got %d (%v)", len(expectedMap), len(addrs), addrs)
	}

	for _, a := range addrs {
		if !expectedMap[a] {
			t.Errorf("unexpected resolved address: %s", a)
		}
	}
}

func TestResolveDNSSeeds_FilterUnroutable(t *testing.T) {
	mockLookup := func(host string) ([]net.IP, error) {
		return []net.IP{
			net.ParseIP("0.0.0.0"),
			net.ParseIP("224.0.0.1"), // Multicast
			net.ParseIP("192.0.2.10"),
		}, nil
	}

	addrs := ResolveDNSSeeds([]string{"seed.scytale.org"}, 9001, mockLookup)

	if len(addrs) != 1 || addrs[0] != "192.0.2.10:9001" {
		t.Fatalf("expected only 192.0.2.10:9001, got %v", addrs)
	}
}
