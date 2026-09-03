package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/seeder"
)

func main() {
	defaultCfg := seeder.DefaultConfig()

	domainFlag := flag.String("domain", defaultCfg.Domain, "Authoritative domain name to serve (e.g. seed.scytale.org)")
	nsFlag := flag.String("nameserver", defaultCfg.Nameserver, "Authoritative nameserver FQDN (e.g. ns1.seed.scytale.org)")
	listenFlag := flag.String("listen", defaultCfg.ListenAddr, "Address and port to bind DNS server (e.g. :53 or :1053)")
	p2pPortFlag := flag.Uint("p2p-port", uint(defaultCfg.P2PPort), "Default target Scytale P2P wire port")
	seedsFlag := flag.String("seeds", "", "Comma-separated list of initial bootstrap seed node addresses")
	dataFileFlag := flag.String("data-file", defaultCfg.DataFile, "Filepath to persist node reputation state (JSON)")
	workersFlag := flag.Int("workers", defaultCfg.Workers, "Number of concurrent crawler worker goroutines")
	intervalFlag := flag.Duration("probe-interval", defaultCfg.ProbeInterval, "Interval between probes for healthy nodes")

	flag.Parse()

	cfg := &seeder.Config{
		Domain:        *domainFlag,
		Nameserver:    *nsFlag,
		ListenAddr:    *listenFlag,
		P2PPort:       uint16(*p2pPortFlag),
		DataFile:      *dataFileFlag,
		Workers:       *workersFlag,
		ProbeInterval: *intervalFlag,
	}

	if *seedsFlag != "" {
		for _, s := range strings.Split(*seedsFlag, ",") {
			trimmed := strings.TrimSpace(s)
			if trimmed != "" {
				cfg.Seeds = append(cfg.Seeds, trimmed)
			}
		}
	}

	log.Printf("[SEEDER] Starting Scytale DNS Seeder v0.3.0...")
	log.Printf("[SEEDER] Serving domain: %s (NS: %s) on %s", cfg.Domain, cfg.Nameserver, cfg.ListenAddr)
	log.Printf("[SEEDER] Target P2P Port: %d | Crawler Workers: %d | Probe Interval: %v",
		cfg.P2PPort, cfg.Workers, cfg.ProbeInterval)

	store := seeder.NewStore()

	// Restore known node records from disk if available
	if err := store.LoadFromFile(cfg.DataFile); err != nil {
		log.Printf("[SEEDER] Warning: failed to load existing node state: %v", err)
	} else {
		log.Printf("[SEEDER] Loaded %d known node records from %s", store.Size(), cfg.DataFile)
	}

	crawler := seeder.NewCrawler(cfg, store)
	crawler.Start()
	log.Printf("[SEEDER] Crawler worker pool started successfully.")

	dnsServer := seeder.NewDNSServer(cfg, store)
	if err := dnsServer.Start(); err != nil {
		crawler.Stop()
		log.Fatalf("[SEEDER] Fatal: failed to start DNS server: %v", err)
	}
	log.Printf("[SEEDER] DNS server listening on UDP and TCP %s.", cfg.ListenAddr)

	// Setup periodic state backup ticker
	saveTicker := time.NewTicker(5 * time.Minute)
	defer saveTicker.Stop()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)

	for {
		select {
		case sig := <-sigCh:
			log.Printf("[SEEDER] Received shutdown signal (%s), stopping services gracefully...", sig)

			// 1. Stop DNS listener
			if err := dnsServer.Shutdown(); err != nil {
				log.Printf("[SEEDER] Error shutting down DNS server: %v", err)
			} else {
				log.Printf("[SEEDER] DNS server shutdown cleanly.")
			}

			// 2. Stop crawler
			crawler.Stop()
			log.Printf("[SEEDER] Crawler stopped cleanly.")

			// 3. Save latest node state to disk
			if err := store.SaveToFile(cfg.DataFile); err != nil {
				log.Printf("[SEEDER] Error saving node state to %s: %v", cfg.DataFile, err)
			} else {
				log.Printf("[SEEDER] Persisted %d node records to %s.", store.Size(), cfg.DataFile)
			}

			log.Printf("[SEEDER] Graceful shutdown complete. Exiting.")
			return

		case <-saveTicker.C:
			if err := store.SaveToFile(cfg.DataFile); err != nil {
				log.Printf("[SEEDER] Periodic backup error: %v", err)
			} else {
				goodNodes := len(store.GetGoodNodes())
				log.Printf("[SEEDER] Periodic state saved: %d known nodes (%d good/routable).",
					store.Size(), goodNodes)
			}
		}
	}
}

func init() {
	log.SetPrefix("")
	log.SetFlags(log.Ldate | log.Ltime | log.Lmicroseconds)
}

func usage() {
	fmt.Fprintf(os.Stderr, "Usage of scytale-seeder:\n")
	flag.PrintDefaults()
}
