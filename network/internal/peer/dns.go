package peer

import (
	"net"
	"strconv"
	"strings"
)

// LookupIPFunc abstracts DNS IP address resolution for testability.
type LookupIPFunc func(host string) ([]net.IP, error)

// ResolveDNSSeeds resolves a list of DNS seed hostnames into formatted "host:port" strings.
// It supports:
// 1. Bare domain names (e.g. "seed.scytale.org") using defaultPort.
// 2. Domain names with custom ports (e.g. "seed.scytale.org:9002").
// 3. Literal IP addresses.
// It deduplicates results and filters out unroutable addresses (unspecified and multicast).
// If lookup is nil, net.LookupIP is used by default.
func ResolveDNSSeeds(seeds []string, defaultPort uint16, lookup LookupIPFunc) []string {
	if lookup == nil {
		lookup = net.LookupIP
	}

	seen := make(map[string]struct{})
	var results []string

	for _, seed := range seeds {
		seed = strings.TrimSpace(seed)
		if seed == "" {
			continue
		}

		host := seed
		port := defaultPort

		if h, pStr, err := net.SplitHostPort(seed); err == nil {
			host = h
			if p, err := strconv.ParseUint(pStr, 10, 16); err == nil && p > 0 {
				port = uint16(p)
			}
		}

		// Check if seed is a literal IP address
		if ip := net.ParseIP(host); ip != nil {
			if !ip.IsUnspecified() && !ip.IsMulticast() {
				addrStr := net.JoinHostPort(ip.String(), strconv.Itoa(int(port)))
				if _, exists := seen[addrStr]; !exists {
					seen[addrStr] = struct{}{}
					results = append(results, addrStr)
				}
			}
			continue
		}

		// Perform DNS query
		ips, err := lookup(host)
		if err != nil {
			continue
		}

		for _, ip := range ips {
			if ip == nil || ip.IsUnspecified() || ip.IsMulticast() {
				continue
			}

			addrStr := net.JoinHostPort(ip.String(), strconv.Itoa(int(port)))
			if _, exists := seen[addrStr]; !exists {
				seen[addrStr] = struct{}{}
				results = append(results, addrStr)
			}
		}
	}

	return results
}
