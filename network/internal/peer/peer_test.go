package peer_test

import (
	"net"
	"testing"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/peer"
	"github.com/scytale-network/scytale-p2p/internal/wire"
)

const (
	testNetworkID uint32 = 0x00000001
	testMagic     uint32 = wire.MagicTestnet
)

func localVersion(networkID uint32) peer.VersionMsg {
	return peer.VersionMsg{
		ProtocolVersion: peer.CurrentProtocolVersion,
		NetworkID:       networkID,
		BestHeight:      0,
		BestHash:        [32]byte{},
		Timestamp:       time.Now().Unix(),
	}
}

// TestHandshakeSuccess verifies that two peers with matching NetworkID complete
// the version/verack handshake and both transition to StateReady.
// Peer A is the initiator (dials), Peer B is the responder (accepts).
func TestHandshakeSuccess(t *testing.T) {
	connA, connB := net.Pipe()
	defer connA.Close()
	defer connB.Close()

	peerA := peer.New(connA, testNetworkID, testMagic)
	peerB := peer.New(connB, testNetworkID, testMagic)

	v := localVersion(testNetworkID)
	doneA := make(chan error, 1)
	doneB := make(chan error, 1)

	go func() { doneA <- peerA.PerformHandshake(v, true) }()   // initiator
	go func() { doneB <- peerB.PerformHandshake(v, false) }()  // responder

	if err := <-doneA; err != nil {
		t.Errorf("peer A (initiator) handshake error: %v", err)
	}
	if err := <-doneB; err != nil {
		t.Errorf("peer B (responder) handshake error: %v", err)
	}
}

// TestHandshakeMismatchedNetworkID verifies that a peer with a different NetworkID
// is rejected during handshake without causing a panic.
func TestHandshakeMismatchedNetworkID(t *testing.T) {
	connA, connB := net.Pipe()
	defer connA.Close()
	defer connB.Close()

	peerA := peer.New(connA, testNetworkID, testMagic)
	// Peer B uses a different Network ID
	peerB := peer.New(connB, 0x00000099, testMagic)

	vA := localVersion(testNetworkID)
	vB := localVersion(0x00000099)

	doneA := make(chan error, 1)
	doneB := make(chan error, 1)

	go func() { doneA <- peerA.PerformHandshake(vA, true) }()  // initiator
	go func() { doneB <- peerB.PerformHandshake(vB, false) }() // responder

	errA := <-doneA
	errB := <-doneB

	// At least one side must detect the Network ID mismatch
	if errA == nil && errB == nil {
		t.Error("expected at least one peer to detect network ID mismatch, but both succeeded")
	}

	// Verify that the specific error is ErrNetworkIDMismatch on at least one side
	hasMismatch := errA == peer.ErrNetworkIDMismatch || errB == peer.ErrNetworkIDMismatch
	if !hasMismatch {
		t.Errorf(
			"expected ErrNetworkIDMismatch on at least one side; got errA=%v, errB=%v",
			errA, errB,
		)
	}
}

// TestVersionMsgEncodeDecodeRoundtrip verifies the VersionMsg binary codec.
func TestVersionMsgEncodeDecodeRoundtrip(t *testing.T) {
	original := peer.VersionMsg{
		ProtocolVersion: 1,
		NetworkID:       42,
		BestHeight:      12345,
		BestHash:        [32]byte{0xAB, 0xCD},
		Timestamp:       1700000000,
	}
	encoded := peer.EncodeVersion(original)
	decoded, err := peer.DecodeVersion(encoded)
	if err != nil {
		t.Fatalf("DecodeVersion error: %v", err)
	}
	if decoded != original {
		t.Errorf("roundtrip mismatch: got %+v, want %+v", decoded, original)
	}
}

// TestPeerStateTransitions verifies state machine transitions during successful handshake.
func TestPeerStateTransitions(t *testing.T) {
	connA, connB := net.Pipe()
	defer connA.Close()
	defer connB.Close()

	peerA := peer.New(connA, testNetworkID, testMagic)
	peerB := peer.New(connB, testNetworkID, testMagic)

	if peerA.State() != peer.StateConnected {
		t.Errorf("initial state want StateConnected, got %s", peerA.State())
	}

	v := localVersion(testNetworkID)
	doneA := make(chan error, 1)
	doneB := make(chan error, 1)
	go func() { doneA <- peerA.PerformHandshake(v, true) }()
	go func() { doneB <- peerB.PerformHandshake(v, false) }()

	if err := <-doneA; err != nil {
		t.Fatalf("peerA handshake: %v", err)
	}
	if err := <-doneB; err != nil {
		t.Fatalf("peerB handshake: %v", err)
	}

	if peerA.State() != peer.StateReady {
		t.Errorf("after handshake want StateReady, got %s", peerA.State())
	}
	if peerB.State() != peer.StateReady {
		t.Errorf("peerB after handshake want StateReady, got %s", peerB.State())
	}
}
