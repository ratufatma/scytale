package seeder

import (
	"net"
	"testing"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/peer"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// mockPeerListener spins up a temporary TCP server that completes version/verack handshake and serves an addr message.
func startMockPeerListener(t *testing.T, height uint64, advertisedIP net.IP, advertisedPort uint16) (string, func()) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to start mock peer listener: %v", err)
	}

	stopCh := make(chan struct{})

	go func() {
		for {
			conn, err := ln.Accept()
			if err != nil {
				select {
				case <-stopCh:
					return
				default:
					return
				}
			}

			go func(c net.Conn) {
				defer func() { _ = c.Close() }()
				_ = c.SetDeadline(time.Now().Add(5 * time.Second))

				// 1. Read version
				msg, err := wire.ReadMessage(c, wire.MagicTestnet)
				if err != nil || msg.Command != wire.CmdVersion {
					return
				}

				// 2. Send version
				localVer := peer.VersionMsg{
					ProtocolVersion: peer.CurrentProtocolVersion,
					NetworkID:       wire.MagicTestnet,
					BestHeight:      height,
					BestHash:        [32]byte{1, 2, 3},
					Timestamp:       time.Now().Unix(),
				}
				if err := wire.WriteMessage(c, wire.MagicTestnet, wire.CmdVersion, peer.EncodeVersion(localVer)); err != nil {
					return
				}

				// 3. Read verack
				msg, err = wire.ReadMessage(c, wire.MagicTestnet)
				if err != nil || msg.Command != wire.CmdVerack {
					return
				}

				// 4. Send verack
				if err := wire.WriteMessage(c, wire.MagicTestnet, wire.CmdVerack, nil); err != nil {
					return
				}

				// 5. Read getaddr
				msg, err = wire.ReadMessage(c, wire.MagicTestnet)
				if err != nil || msg.Command != wire.CmdGetAddr {
					return
				}

				// 6. Send addr
				if advertisedIP != nil {
					addrs := []wire.NetAddress{
						{
							Timestamp: time.Now().Unix(),
							Services:  1,
							IP:        advertisedIP,
							Port:      advertisedPort,
						},
					}
					_ = wire.WriteMessage(c, wire.MagicTestnet, wire.CmdAddr, wire.EncodeAddr(addrs))
				}
			}(conn)
		}
	}()

	return ln.Addr().String(), func() {
		close(stopCh)
		_ = ln.Close()
	}
}

func TestCrawler_ProbeAndDiscovery(t *testing.T) {
	advertisedIP := net.ParseIP("198.51.100.99")
	advertisedPort := uint16(9001)

	mockAddr, cleanup := startMockPeerListener(t, 1234, advertisedIP, advertisedPort)
	defer cleanup()

	host, _, err := net.SplitHostPort(mockAddr)
	if err != nil {
		t.Fatalf("failed to split hostport: %v", err)
	}

	cfg := &Config{
		Domain:        "seed.scytale.org",
		Nameserver:    "ns1.seed.scytale.org",
		ListenAddr:    ":1053",
		P2PPort:       9001,
		Seeds:         []string{mockAddr},
		Workers:       2,
		ProbeInterval: 1 * time.Second,
	}

	store := NewStore()
	crawler := NewCrawler(cfg, store)

	crawler.Start()
	defer crawler.Stop()

	// Wait for crawler to probe the mock peer and ingest the advertised peer
	var rec *NodeRecord
	var ok bool
	for i := 0; i < 20; i++ {
		time.Sleep(150 * time.Millisecond)
		ip := net.ParseIP(host)
		// check mock peer
		for _, n := range store.GetAllNodes() {
			if n.IP.Equal(ip) && n.SuccessCount > 0 {
				rec = n
				ok = true
				break
			}
		}
		if ok && store.Size() >= 2 {
			break
		}
	}

	if !ok || rec == nil {
		t.Fatalf("expected crawler to successfully probe mock peer, known nodes: %d", store.Size())
	}

	if rec.BestHeight != 1234 {
		t.Errorf("expected BestHeight 1234, got %d", rec.BestHeight)
	}

	// Verify that the advertised peer was discovered and added to the store
	discovered, found := store.GetNode(advertisedIP, advertisedPort)
	if !found || discovered == nil {
		t.Fatalf("expected advertised peer %s:%d to be added to store", advertisedIP, advertisedPort)
	}
}
