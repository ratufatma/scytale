package peer

import (
	"net"
	"testing"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

func TestDynamicPeerDiscoveryExchange(t *testing.T) {
	// Setup Node B listener
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to listen: %v", err)
	}
	defer listener.Close()

	addrB := listener.Addr().String()
	addrC := "127.0.0.1:9999"

	// Node B's AddrBook knows about C
	bookB := NewAddrBook("", true)
	bookB.AddAddress(addrC, "manual")

	// Node A's AddrBook is initially empty
	bookA := NewAddrBook("", true)

	serverDone := make(chan struct{})

	// Node B server loop
	go func() {
		defer close(serverDone)
		connB, err := listener.Accept()
		if err != nil {
			return
		}
		defer connB.Close()

		pB := New(connB, wire.MagicTestnet, wire.MagicTestnet)
		defer pB.Close()

		verB := VersionMsg{
			ProtocolVersion: CurrentProtocolVersion,
			NetworkID:       wire.MagicTestnet,
			Timestamp:       time.Now().Unix(),
		}

		if err := pB.PerformHandshake(verB, false); err != nil {
			t.Errorf("server handshake failed: %v", err)
			return
		}

		// Handle incoming getaddr message
		msg, err := pB.Recv()
		if err != nil {
			t.Errorf("server recv failed: %v", err)
			return
		}

		if msg.Command == wire.CmdGetAddr {
			addrs := bookB.GetAddresses(10)
			var netAddrs []wire.NetAddress
			for _, aStr := range addrs {
				if na, err := wire.NewNetAddressFromString(aStr, 1); err == nil {
					netAddrs = append(netAddrs, *na)
				}
			}
			payload := wire.EncodeAddr(netAddrs)
			if err := pB.Send(wire.CmdAddr, payload); err != nil {
				t.Errorf("server send addr failed: %v", err)
			}
		}
	}()

	// Node A connects as client
	connA, err := net.Dial("tcp", addrB)
	if err != nil {
		t.Fatalf("client dial failed: %v", err)
	}
	defer connA.Close()

	pA := New(connA, wire.MagicTestnet, wire.MagicTestnet)
	defer pA.Close()

	verA := VersionMsg{
		ProtocolVersion: CurrentProtocolVersion,
		NetworkID:       wire.MagicTestnet,
		Timestamp:       time.Now().Unix(),
	}

	if err := pA.PerformHandshake(verA, true); err != nil {
		t.Fatalf("client handshake failed: %v", err)
	}

	// Client sends getaddr
	if err := pA.Send(wire.CmdGetAddr, nil); err != nil {
		t.Fatalf("client send getaddr failed: %v", err)
	}

	// Client receives addr
	msg, err := pA.Recv()
	if err != nil {
		t.Fatalf("client recv addr failed: %v", err)
	}

	if msg.Command != wire.CmdAddr {
		t.Fatalf("expected CmdAddr, got %q", msg.Command)
	}

	decoded, err := wire.DecodeAddr(msg.Payload)
	if err != nil {
		t.Fatalf("client decode addr failed: %v", err)
	}

	if len(decoded) != 1 {
		t.Fatalf("expected 1 address, got %d", len(decoded))
	}

	// Add into Node A's AddrBook
	bookA.AddAddress(decoded[0].String(), pA.Address())

	if bookA.Size() != 1 {
		t.Errorf("expected Node A to have learned 1 address, got %d", bookA.Size())
	}

	<-serverDone
}
