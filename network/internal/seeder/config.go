package seeder

import (
	"time"
)

// Config defines the runtime configuration parameters for the Scytale DNS Seeder daemon.
type Config struct {
	// Domain is the authoritative domain name served by this DNS seeder (e.g. "seed.scytale.org").
	Domain string

	// Nameserver is the FQDN of the authoritative nameserver (e.g. "ns1.seed.scytale.org").
	Nameserver string

	// ListenAddr is the network address and port for the DNS server to bind to (e.g. ":53" or ":1053").
	ListenAddr string

	// P2PPort is the default Scytale P2P wire port probed by the crawler (default: 9001).
	P2PPort uint16

	// Seeds is the initial list of bootstrap node IPs or host:port strings to seed the crawler.
	Seeds []string

	// DataFile is the filepath where active node records are atomically persisted.
	DataFile string

	// Workers is the number of concurrent crawler worker goroutines (default: 16).
	Workers int

	// ProbeInterval is the duration between successive probes for healthy nodes (default: 15m).
	ProbeInterval time.Duration
}

// DefaultConfig returns a Config struct populated with production-safe defaults.
func DefaultConfig() *Config {
	return &Config{
		Domain:        "seed.scytale.org",
		Nameserver:    "ns1.seed.scytale.org",
		ListenAddr:    ":53",
		P2PPort:       9001,
		Seeds:         []string{},
		DataFile:      "seeder_nodes.json",
		Workers:       16,
		ProbeInterval: 15 * time.Minute,
	}
}
