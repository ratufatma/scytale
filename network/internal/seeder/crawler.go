package seeder

import (
	"fmt"
	"net"
	"strconv"
	"sync"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/peer"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

type candidateNode struct {
	ip   net.IP
	port uint16
}

// Crawler executes concurrent network probing and topology discovery for the Scytale P2P mesh.
type Crawler struct {
	cfg       *Config
	store     *Store
	workQueue chan candidateNode
	quit      chan struct{}
	wg        sync.WaitGroup
	dialer    func(network, address string, timeout time.Duration) (net.Conn, error)
}

// NewCrawler creates a new Crawler instance with the specified configuration and store.
func NewCrawler(cfg *Config, store *Store) *Crawler {
	workers := cfg.Workers
	if workers <= 0 {
		workers = 16
	}

	return &Crawler{
		cfg:       cfg,
		store:     store,
		workQueue: make(chan candidateNode, workers*4),
		quit:      make(chan struct{}),
		dialer:    net.DialTimeout,
	}
}

// Start begins crawling by injecting seed nodes, launching workers, and running the probe scheduler.
func (c *Crawler) Start() {
	// 1. Inject initial bootstrap seeds into store
	for _, seed := range c.cfg.Seeds {
		c.injectSeed(seed)
	}

	workers := c.cfg.Workers
	if workers <= 0 {
		workers = 16
	}

	// 2. Launch worker pool
	for i := 0; i < workers; i++ {
		c.wg.Add(1)
		go c.workerLoop()
	}

	// 3. Launch periodic scheduler loop
	c.wg.Add(1)
	go c.schedulerLoop()
}

// Stop terminates all crawling activity and waits for active probes to complete.
func (c *Crawler) Stop() {
	select {
	case <-c.quit:
		return // already closed
	default:
		close(c.quit)
	}
	c.wg.Wait()
}

// injectSeed parses an address string (IP or host:port) and adds it to the store.
func (c *Crawler) injectSeed(seed string) {
	host, portStr, err := net.SplitHostPort(seed)
	var port uint16
	if err != nil {
		host = seed
		port = c.cfg.P2PPort
	} else {
		p, err := strconv.ParseUint(portStr, 10, 16)
		if err != nil {
			port = c.cfg.P2PPort
		} else {
			port = uint16(p)
		}
	}

	if ip := net.ParseIP(host); ip != nil {
		c.store.AddNode(ip, port)
		return
	}

	// Resolve hostname if domain name provided
	ips, err := net.LookupIP(host)
	if err == nil {
		for _, ip := range ips {
			c.store.AddNode(ip, port)
		}
	}
}

// schedulerLoop scans known nodes periodically and enqueues overdue nodes for probing.
func (c *Crawler) schedulerLoop() {
	defer c.wg.Done()

	ticker := time.NewTicker(3 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-c.quit:
			return
		case <-ticker.C:
			c.scheduleDueProbes()
		}
	}
}

// scheduleDueProbes evaluates all known nodes and queues those due for a probe attempt.
func (c *Crawler) scheduleDueProbes() {
	now := time.Now()
	nodes := c.store.GetAllNodes()

	for _, rec := range nodes {
		var nextDue time.Time

		if rec.LastAttempt.IsZero() {
			// Unprobed node: probe immediately
			nextDue = time.Time{}
		} else if rec.FailStreak > 0 {
			// Exponential backoff: base * (2^streak), capped at 6 hours
			shift := rec.FailStreak
			if shift > 5 {
				shift = 5
			}
			backoff := c.cfg.ProbeInterval * (1 << shift)
			if backoff > 6*time.Hour {
				backoff = 6 * time.Hour
			}
			nextDue = rec.LastAttempt.Add(backoff)
		} else {
			// Healthy node: regular probe interval
			nextDue = rec.LastAttempt.Add(c.cfg.ProbeInterval)
		}

		if !now.Before(nextDue) {
			select {
			case c.workQueue <- candidateNode{ip: rec.IP, port: rec.Port}:
			default:
				// Work queue full; will be picked up on next tick
			}
		}
	}
}

// workerLoop drains the work queue and executes probes.
func (c *Crawler) workerLoop() {
	defer c.wg.Done()

	for {
		select {
		case <-c.quit:
			return
		case cand := <-c.workQueue:
			c.probeNode(cand.ip, cand.port)
		}
	}
}

// probeNode dials a single node, completes the P2P handshake, queries for addresses, and updates store metrics.
func (c *Crawler) probeNode(ip net.IP, port uint16) {
	c.store.RecordAttempt(ip, port)

	addrStr := net.JoinHostPort(ip.String(), strconv.Itoa(int(port)))
	conn, err := c.dialer("tcp", addrStr, 3*time.Second)
	if err != nil {
		c.store.RecordFailure(ip, port)
		return
	}
	defer func() { _ = conn.Close() }()

	_ = conn.SetDeadline(time.Now().Add(5 * time.Second))

	// 1. Send version message
	localVer := peer.VersionMsg{
		ProtocolVersion: peer.CurrentProtocolVersion,
		NetworkID:       wire.MagicTestnet,
		BestHeight:      0,
		BestHash:        [32]byte{},
		Timestamp:       time.Now().Unix(),
	}

	if err := wire.WriteMessage(conn, wire.MagicTestnet, wire.CmdVersion, peer.EncodeVersion(localVer)); err != nil {
		c.store.RecordFailure(ip, port)
		return
	}

	// 2. Read version response
	msg, err := wire.ReadMessage(conn, wire.MagicTestnet)
	if err != nil || msg.Command != wire.CmdVersion {
		c.store.RecordFailure(ip, port)
		return
	}

	remoteVer, err := peer.DecodeVersion(msg.Payload)
	if err != nil || remoteVer.NetworkID != wire.MagicTestnet {
		c.store.RecordFailure(ip, port)
		return
	}

	// 3. Send verack
	if err := wire.WriteMessage(conn, wire.MagicTestnet, wire.CmdVerack, nil); err != nil {
		c.store.RecordFailure(ip, port)
		return
	}

	// 4. Read remote verack
	msg, err = wire.ReadMessage(conn, wire.MagicTestnet)
	if err != nil || msg.Command != wire.CmdVerack {
		c.store.RecordFailure(ip, port)
		return
	}

	// Handshake successful: record success and state metrics
	c.store.RecordSuccess(ip, port, remoteVer.ProtocolVersion, 1, remoteVer.BestHeight)

	// 5. Discovery: Query peer for neighbors via getaddr
	if err := wire.WriteMessage(conn, wire.MagicTestnet, wire.CmdGetAddr, nil); err != nil {
		return
	}

	// Read responses for up to 2 seconds looking for CmdAddr
	_ = conn.SetDeadline(time.Now().Add(2 * time.Second))
	for {
		resp, err := wire.ReadMessage(conn, wire.MagicTestnet)
		if err != nil {
			break
		}
		if resp.Command == wire.CmdAddr {
			addrs, err := wire.DecodeAddr(resp.Payload)
			if err == nil {
				for _, addr := range addrs {
					if addr.IP != nil && !addr.IP.IsUnspecified() && !addr.IP.IsMulticast() {
						c.store.AddNode(addr.IP, addr.Port)
					}
				}
			}
			break
		}
	}
}

// String provides a human-readable summary of the crawler state.
func (c *Crawler) String() string {
	return fmt.Sprintf("Crawler(workers=%d, known=%d, good=%d)",
		c.cfg.Workers, c.store.Size(), len(c.store.GetGoodNodes()))
}
