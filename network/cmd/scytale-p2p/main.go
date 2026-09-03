package main

import (
	"bytes"
	"crypto/sha256"
	"flag"
	"fmt"
	"log"
	"net"
	"os"
	"os/signal"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/bridge"
	"github.com/scytale-network/scytale-p2p/internal/gossip"
	"github.com/scytale-network/scytale-p2p/internal/peer"
	syncer "github.com/scytale-network/scytale-p2p/internal/sync"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

type stringList []string

func (s *stringList) String() string {
	return strings.Join(*s, ",")
}

func (s *stringList) Set(val string) error {
	*s = append(*s, val)
	return nil
}

type Daemon struct {
	bridgeSocket    string
	p2pBind         string
	peers           []string
	networkID       uint32
	allowLocalPeers bool
	peersFile       string
	maxOutbound     int
	fastSync        bool
	dnsSeeds        []string
	noDNSSeeds      bool

	bridge            *bridge.SocketConsensusBridge
	filter            *gossip.Filter
	gossipEngine      *gossip.Engine
	syncManager       *syncer.Syncer
	addrBook          *peer.AddrBook
	snapshotAssembler *peer.SnapshotAssembler
	triggerDial       chan struct{}

	mu       sync.Mutex
	peerPool map[string]*peer.Peer
	shutdown chan struct{}
}

func main() {
	var (
		bridgeSocket    = flag.String("bridge-sock", "", "Path to the Rust node Unix domain socket bridge")
		p2pBind         = flag.String("p2p-bind", "", "TCP address to listen for incoming P2P connections (e.g. 127.0.0.1:9001)")
		networkID       = flag.Uint("network-id", uint(wire.MagicTestnet), "P2P Network ID")
		allowLocalPeers = flag.Bool("allow-local-peers", false, "Allow connection to local/loopback and private network peers")
		peersFile       = flag.String("peers-file", "peers.json", "Path to peers database JSON file")
		maxOutbound     = flag.Int("max-outbound", 8, "Target maximum outbound connections")
		fastSync        = flag.Bool("fast-sync", false, "Enable UTXO snapshot fast sync mode")
		noDNSSeeds      = flag.Bool("no-dns-seeds", false, "Disable querying DNS seeders for peer discovery")
	)
	var peersFlag stringList
	flag.Var(&peersFlag, "peer", "Initial peer address to dial (can be specified multiple times)")

	var dnsSeedsFlag stringList
	flag.Var(&dnsSeedsFlag, "dns-seed", "DNS seed domain to query on cold start (can be specified multiple times)")
	flag.Parse()

	if len(dnsSeedsFlag) == 0 {
		dnsSeedsFlag = []string{"seed.scytale.org"}
	}

	if *bridgeSocket == "" {
		log.Println("Initializing Scytale P2P Network Service...")
		log.Println("P2P Daemon ready: listening for peers and synchronizing network state.")
		_ = os.Stdout.Sync()
		return
	}

	d := &Daemon{
		bridgeSocket:      *bridgeSocket,
		p2pBind:           *p2pBind,
		peers:             peersFlag,
		networkID:         uint32(*networkID),
		allowLocalPeers:   *allowLocalPeers,
		peersFile:         *peersFile,
		maxOutbound:       *maxOutbound,
		fastSync:          *fastSync,
		dnsSeeds:          dnsSeedsFlag,
		noDNSSeeds:        *noDNSSeeds,
		peerPool:          make(map[string]*peer.Peer),
		snapshotAssembler: peer.NewSnapshotAssembler(),
		triggerDial:       make(chan struct{}, 1),
		shutdown:          make(chan struct{}),
	}

	if err := d.Run(); err != nil {
		log.Fatalf("P2P daemon fatal error: %v", err)
	}
}

func (d *Daemon) Run() error {
	log.Printf("[P2P] Connecting to consensus bridge at %s...", d.bridgeSocket)
	br, err := bridge.NewSocketConsensusBridge(d.bridgeSocket)
	if err != nil {
		return fmt.Errorf("connecting to consensus bridge: %w", err)
	}
	d.bridge = br
	defer d.bridge.Close()

	d.filter = gossip.NewFilter()
	d.gossipEngine = gossip.NewEngine(d.filter, d.bridge)
	d.syncManager = syncer.New(d.bridge)
	d.addrBook = peer.NewAddrBook(d.peersFile, d.allowLocalPeers)

	// Seed static peers from CLI into Address Book
	for _, pAddr := range d.peers {
		d.addrBook.AddAddress(pAddr, "cli-seed")
	}

	// Cold-start DNS seed discovery if AddrBook is empty and DNS seeds enabled
	if !d.noDNSSeeds && len(d.dnsSeeds) > 0 && d.addrBook.Size() == 0 {
		go d.queryDNSSeedsAsync()
	}

	log.Printf("[P2P] Consensus bridge established.")

	// Start broadcast events listener from Rust node
	go d.listenBridgeEvents()

	// Start inbound P2P TCP listener if requested
	if d.p2pBind != "" {
		go d.listenInbound()
	}

	// Start outbound dialers for initial static peers
	for _, pAddr := range d.peers {
		go d.dialPeerLoop(pAddr)
	}

	// Start dynamic peer discovery auto-dialer
	go d.autoDialerLoop()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	select {
	case <-sigCh:
		log.Println("[P2P] Shutdown signal received, terminating...")
	case <-d.shutdown:
		log.Println("[P2P] Bridge closed, terminating...")
	}

	d.closeAllPeers()
	_ = d.addrBook.Save()
	return nil
}

func (d *Daemon) queryDNSSeedsAsync() {
	log.Printf("[P2P] Resolving DNS seeds: %s...", strings.Join(d.dnsSeeds, ", "))
	resolved := peer.ResolveDNSSeeds(d.dnsSeeds, 9001, nil)
	if len(resolved) == 0 {
		log.Printf("[P2P] DNS seed resolution returned 0 addresses.")
		return
	}

	added := d.addrBook.AddAddresses(resolved, "dns-seed")
	log.Printf("[P2P] DNS seed discovery added %d new peers (out of %d resolved).", added, len(resolved))
	if added > 0 {
		select {
		case d.triggerDial <- struct{}{}:
		default:
		}
	}
}

func (d *Daemon) autoDialerLoop() {
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	dnsFallbackTicker := time.NewTicker(3 * time.Minute)
	defer dnsFallbackTicker.Stop()

	for {
		select {
		case <-d.shutdown:
			return
		case <-d.triggerDial:
		case <-dnsFallbackTicker.C:
			if !d.noDNSSeeds && len(d.dnsSeeds) > 0 && d.addrBook.Size() == 0 {
				go d.queryDNSSeedsAsync()
			}
		case <-ticker.C:
		}

		d.mu.Lock()
		outboundCount := 0
		connected := make([]string, 0, len(d.peerPool))
		for _, p := range d.peerPool {
			connected = append(connected, p.Address())
			if p.IsOutbound() {
				outboundCount++
			}
		}
		d.mu.Unlock()

			if outboundCount >= d.maxOutbound {
				continue
			}

		candidate, ok := d.addrBook.SelectAddressToDial(connected)
		if ok && candidate != "" {
			d.addrBook.MarkAttempt(candidate)
			go d.ConnectPeer(candidate)
		}
	}
}

func (d *Daemon) listenBridgeEvents() {
	for {
		select {
		case <-d.shutdown:
			return
		case ev, ok := <-d.bridge.Events():
			if !ok {
				close(d.shutdown)
				return
			}
			if ev.Type == "ConnectPeer" {
				log.Printf("[P2P] Dynamic peer connect request to %s", ev.PeerAddr)
				d.addrBook.AddAddress(ev.PeerAddr, "ipc-connect")
				go d.ConnectPeer(ev.PeerAddr)
				continue
			}

			d.filter.MarkSeen(ev.Hash)
			var invType gossip.InvType
			if ev.Type == "BroadcastBlock" {
				invType = gossip.InvTypeBlock
				log.Printf("[P2P] Broadcast event from Rust: Block 0x%x", ev.Hash[:8])
			} else {
				invType = gossip.InvTypeTx
				log.Printf("[P2P] Broadcast event from Rust: Tx 0x%x", ev.Hash[:8])
			}

			payload := gossip.EncodeInv([]gossip.InvItem{{Type: invType, Hash: ev.Hash}})
			d.broadcastToPeers(wire.CmdInv, payload, "")
		}
	}
}

// ConnectPeer dials a remote peer dynamically and initiates handshaking and sync.
func (d *Daemon) ConnectPeer(address string) {
	d.mu.Lock()
	for _, p := range d.peerPool {
		if p.Address() == address {
			d.mu.Unlock()
			log.Printf("[P2P] Already connected to %s", address)
			return
		}
	}
	d.mu.Unlock()

	conn, err := net.DialTimeout("tcp", address, 5*time.Second)
	if err != nil {
		d.addrBook.MarkFailed(address)
		log.Printf("[P2P] Failed to connect to %s: %v", address, err)
		return
	}
	d.addrBook.MarkSuccess(address)
	go d.handleConnection(conn, true)
}

func (d *Daemon) broadcastToPeers(cmd string, payload []byte, exceptID string) {
	d.mu.Lock()
	peers := make([]*peer.Peer, 0, len(d.peerPool))
	for id, p := range d.peerPool {
		if id != exceptID {
			peers = append(peers, p)
		}
	}
	d.mu.Unlock()

	for _, p := range peers {
		_ = p.Send(cmd, payload)
	}
}

func (d *Daemon) listenInbound() {
	l, err := net.Listen("tcp", d.p2pBind)
	if err != nil {
		log.Printf("[P2P] Inbound listener failed on %s: %v", d.p2pBind, err)
		return
	}
	defer l.Close()
	log.Printf("[P2P] Inbound listener active on %s", d.p2pBind)

	for {
		conn, err := l.Accept()
		if err != nil {
			select {
			case <-d.shutdown:
				return
			default:
				log.Printf("[P2P] Accept error: %v", err)
				continue
			}
		}
		go d.handleConnection(conn, false)
	}
}

func (d *Daemon) dialPeerLoop(address string) {
	for {
		select {
		case <-d.shutdown:
			return
		default:
		}

		conn, err := net.DialTimeout("tcp", address, 5*time.Second)
		if err != nil {
			time.Sleep(2 * time.Second)
			continue
		}

		d.handleConnection(conn, true)
		time.Sleep(2 * time.Second)
	}
}

func (d *Daemon) getLocalVersion() peer.VersionMsg {
	var bestHash [32]byte
	locator, err := d.bridge.GetBlockLocator()
	if err == nil && len(locator) > 0 {
		bestHash = locator[0]
	}

	return peer.VersionMsg{
		ProtocolVersion: peer.CurrentProtocolVersion,
		NetworkID:       d.networkID,
		BestHeight:      0,
		BestHash:        bestHash,
		Timestamp:       time.Now().Unix(),
	}
}

func (d *Daemon) handleConnection(conn net.Conn, isInitiator bool) {
	p := peer.New(conn, d.networkID, wire.MagicTestnet)
	defer p.Close()

	localVer := d.getLocalVersion()
	if err := p.PerformHandshake(localVer, isInitiator); err != nil {
		log.Printf("[P2P] Handshake with %s failed: %v", p.Address(), err)
		return
	}

	if isInitiator {
		p.SetOutbound(true)
	}
	d.addrBook.MarkSuccess(p.Address())

	log.Printf("[P2P] Peer %s handshake SUCCESS (Remote BestHash: 0x%x, Outbound: %v)",
		p.Address(), p.RemoteVersion.BestHash[:8], p.IsOutbound())

	d.mu.Lock()
	d.peerPool[p.ID()] = p
	activePeers := len(d.peerPool)
	d.mu.Unlock()
	_ = d.bridge.UpdatePeerCount(activePeers)

	defer func() {
		d.mu.Lock()
		delete(d.peerPool, p.ID())
		activePeers := len(d.peerPool)
		d.mu.Unlock()
		_ = d.bridge.UpdatePeerCount(activePeers)
		log.Printf("[P2P] Peer %s disconnected", p.Address())
	}()

	// Query peer neighborhood via getaddr
	_ = p.Send(wire.CmdGetAddr, nil)

	// Advertise our own listening address to the peer so they can discover and share it
	if d.p2pBind != "" {
		if _, bindPort, err := net.SplitHostPort(d.p2pBind); err == nil {
			if localHost, _, err := net.SplitHostPort(conn.LocalAddr().String()); err == nil {
				myAddrStr := net.JoinHostPort(localHost, bindPort)
				if na, err := wire.NewNetAddressFromString(myAddrStr, 1); err == nil {
					log.Printf("[P2P] Advertising our listening address %s to %s", myAddrStr, p.Address())
					_ = p.Send(wire.CmdAddr, wire.EncodeAddr([]wire.NetAddress{*na}))
				}
			}
		}
	}

	if d.fastSync && p.RemoteVersion.BestHash != [32]byte{} && p.RemoteVersion.BestHash != localVer.BestHash {
		log.Printf("[P2P] Fast sync enabled: requesting UTXO snapshot for block %x from %s",
			p.RemoteVersion.BestHash[:8], p.Address())
		_ = p.SendGetSnapshot(p.RemoteVersion.BestHash, 0)
	}

	// Trigger initial sync locator query
	_ = d.syncManager.SendGetBlocks(p)

	// Periodic sync ticker to ensure continuous catchup if behind
	stopSync := make(chan struct{})
	defer close(stopSync)
	go func() {
		ticker := time.NewTicker(1 * time.Second)
		getAddrTicker := time.NewTicker(2 * time.Second)
		defer ticker.Stop()
		defer getAddrTicker.Stop()
		for {
			select {
			case <-ticker.C:
				_ = d.syncManager.SendGetBlocks(p)
			case <-getAddrTicker.C:
				d.mu.Lock()
				pCount := len(d.peerPool)
				d.mu.Unlock()
				if pCount < 2 {
					_ = p.Send(wire.CmdGetAddr, nil)
				}
			case <-stopSync:
				return
			}
		}
	}()

	var pendingBlocks int32

	// Message handling loop
	for {
		msg, err := p.Recv()
		if err != nil {
			return
		}

		switch msg.Command {
		case wire.CmdGetAddr:
			log.Printf("[P2P] Received CmdGetAddr from %s", p.Address())
			knownAddrs := d.addrBook.GetAddresses(wire.MaxAddrsPerMsg)
			var netAddrs []wire.NetAddress
			for _, aStr := range knownAddrs {
				if na, err := wire.NewNetAddressFromString(aStr, 1); err == nil {
					netAddrs = append(netAddrs, *na)
				}
			}
			if len(netAddrs) > 0 {
				_ = p.Send(wire.CmdAddr, wire.EncodeAddr(netAddrs))
			}

		case wire.CmdAddr:
			log.Printf("[P2P] Received CmdAddr from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			decoded, err := wire.DecodeAddr(msg.Payload)
			if err == nil {
				var addrStrs []string
				for _, na := range decoded {
					addrStrs = append(addrStrs, na.String())
				}
				added := d.addrBook.AddAddresses(addrStrs, p.Address())
				if added > 0 {
					log.Printf("[P2P] Added %d new peer addresses from %s (total known: %d)",
						added, p.Address(), d.addrBook.Size())
					_ = d.addrBook.Save()
					select {
					case d.triggerDial <- struct{}{}:
					default:
					}
				}
			}

		case wire.CmdGetBlocks:
			log.Printf("[P2P] Received CmdGetBlocks from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			hashes, err := d.bridge.GetCanonicalHashes()
			if err == nil {
				peerLocator, _ := gossip.DecodeHashList(msg.Payload)
				log.Printf("[P2P] Handling GetBlocks with %d canonical hashes and %d locator hashes", len(hashes), len(peerLocator))
				_ = syncer.HandleGetBlocks(p, hashes, peerLocator)
			} else {
				log.Printf("[P2P] Failed to get canonical hashes: %v", err)
			}

		case wire.CmdInvBlocks:
			log.Printf("[P2P] Received CmdInvBlocks from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			n, err := d.syncManager.HandleInvBlocks(p, msg.Payload)
			log.Printf("[P2P] Requested %d blocks from %s (err: %v)", n, p.Address(), err)
			if n > 0 {
				atomic.AddInt32(&pendingBlocks, int32(n))
			}

		case wire.CmdInv:
			_ = d.gossipEngine.HandleInv(p, msg.Payload)

		case wire.CmdGetData:
			log.Printf("[P2P] Received CmdGetData from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			_ = d.gossipEngine.HandleGetData(p, msg.Payload)

		case wire.CmdBlock:
			log.Printf("[P2P] Received CmdBlock from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			if err := d.gossipEngine.HandleBlock(msg.Payload); err == nil {
				var blockHash [32]byte
				if len(msg.Payload) >= 32 {
					copy(blockHash[:], msg.Payload[0:32])
				}
				d.filter.MarkSeen(blockHash)
				d.broadcastToPeers(wire.CmdInv, gossip.EncodeInv([]gossip.InvItem{{Type: gossip.InvTypeBlock, Hash: blockHash}}), p.ID())
			} else {
				log.Printf("[P2P] SubmitBlock error: %v", err)
				_ = d.syncManager.SendGetBlocks(p)
			}
			if atomic.LoadInt32(&pendingBlocks) > 0 {
				if atomic.AddInt32(&pendingBlocks, -1) <= 0 {
					_ = d.syncManager.SendGetBlocks(p)
				}
			}

		case wire.CmdTx:
			if err := d.gossipEngine.HandleTx(msg.Payload); err == nil {
				h := sha256.Sum256(msg.Payload)
				d.filter.MarkSeen(h)
				d.broadcastToPeers(wire.CmdInv, gossip.EncodeInv([]gossip.InvItem{{Type: gossip.InvTypeTx, Hash: h}}), p.ID())
			}

		case wire.CmdPing:
			_ = p.Send(wire.CmdPong, msg.Payload)

		case wire.CmdGetSnapshot:
			log.Printf("[P2P] Received CmdGetSnapshot from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			getSnap, err := wire.DecodeGetSnapshot(bytes.NewReader(msg.Payload))
			if err != nil {
				log.Printf("[P2P] Failed to decode getsnap payload from %s: %v", p.Address(), err)
				continue
			}
			if !p.CanServeSnapshot(30*time.Second, getSnap.ChunkIndex) {
				log.Printf("[P2P] Rate limiting getsnap request from %s", p.Address())
				continue
			}
			chunk, err := d.bridge.ExportSnapshotChunk(getSnap.BlockHash, getSnap.ChunkIndex, wire.MaxSnapshotChunkEntries)
			if err != nil {
				log.Printf("[P2P] ExportSnapshotChunk error for %s: %v", p.Address(), err)
				continue
			}
			if err := p.SendSnapshot(chunk); err != nil {
				log.Printf("[P2P] Failed to send snapshot chunk to %s: %v", p.Address(), err)
			}

		case wire.CmdSnapshot:
			log.Printf("[P2P] Received CmdSnapshot from %s (payload %d bytes)", p.Address(), len(msg.Payload))
			snapshotMsg, err := wire.DecodeSnapshot(bytes.NewReader(msg.Payload))
			if err != nil {
				log.Printf("[P2P] Failed to decode snapshot message from %s: %v", p.Address(), err)
				continue
			}
			isComplete, err := d.snapshotAssembler.AddChunk(snapshotMsg)
			if err != nil {
				log.Printf("[P2P] SnapshotAssembler error from %s: %v", p.Address(), err)
				continue
			}
			if !isComplete {
				if snapshotMsg.ChunkIndex+1 < snapshotMsg.TotalChunks {
					_ = p.SendGetSnapshot(snapshotMsg.BlockHash, snapshotMsg.ChunkIndex+1)
				}
			} else {
				log.Printf("[P2P] Snapshot complete for block %x! Assembling entries...", snapshotMsg.BlockHash[:8])
				entries, err := d.snapshotAssembler.Assemble(snapshotMsg.BlockHash)
				if err != nil {
					log.Printf("[P2P] Failed to assemble snapshot entries: %v", err)
					continue
				}
				appliedCount, err := d.bridge.ApplySnapshot(snapshotMsg.BlockHash, entries)
				if err != nil {
					log.Printf("[P2P] CRITICAL: Failed to apply snapshot: %v", err)
				} else {
					log.Printf("[P2P] Successfully applied snapshot for block %x with %d UTXOs", snapshotMsg.BlockHash[:8], appliedCount)
				}
			}
		}
	}
}

func (d *Daemon) closeAllPeers() {
	d.mu.Lock()
	defer d.mu.Unlock()
	for _, p := range d.peerPool {
		p.Close()
	}
	d.peerPool = make(map[string]*peer.Peer)
}
