// Package peer implements the Scytale P2P peer connection lifecycle state machine,
// handshake negotiation, and session framing.
package peer

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net"
	"sync"
	"time"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// State represents the lifecycle state of a peer connection.
type State int

const (
	StateDisconnected State = iota
	StateConnecting
	StateConnected
	StateHandshake
	StateReady
	StateActive
	StateClosing
)

// String returns a human-readable state name.
func (s State) String() string {
	switch s {
	case StateDisconnected:
		return "DISCONNECTED"
	case StateConnecting:
		return "CONNECTING"
	case StateConnected:
		return "CONNECTED"
	case StateHandshake:
		return "HANDSHAKE"
	case StateReady:
		return "READY"
	case StateActive:
		return "ACTIVE"
	case StateClosing:
		return "CLOSING"
	default:
		return "UNKNOWN"
	}
}

// VersionMsg is the payload carried in the `version` handshake message.
type VersionMsg struct {
	ProtocolVersion uint32
	NetworkID       uint32
	BestHeight      uint64
	BestHash        [32]byte
	Timestamp       int64
}

// EncodeVersion serialises a VersionMsg to bytes.
func EncodeVersion(v VersionMsg) []byte {
	buf := make([]byte, 4+4+8+32+8)
	binary.LittleEndian.PutUint32(buf[0:4], v.ProtocolVersion)
	binary.LittleEndian.PutUint32(buf[4:8], v.NetworkID)
	binary.LittleEndian.PutUint64(buf[8:16], v.BestHeight)
	copy(buf[16:48], v.BestHash[:])
	binary.LittleEndian.PutUint64(buf[48:56], uint64(v.Timestamp))
	return buf
}

// DecodeVersion deserialises a VersionMsg from bytes.
func DecodeVersion(data []byte) (VersionMsg, error) {
	if len(data) < 56 {
		return VersionMsg{}, errors.New("peer: version payload too short")
	}
	var v VersionMsg
	v.ProtocolVersion = binary.LittleEndian.Uint32(data[0:4])
	v.NetworkID = binary.LittleEndian.Uint32(data[4:8])
	v.BestHeight = binary.LittleEndian.Uint64(data[8:16])
	copy(v.BestHash[:], data[16:48])
	v.Timestamp = int64(binary.LittleEndian.Uint64(data[48:56]))
	return v, nil
}

// Protocol constants.
const (
	CurrentProtocolVersion uint32 = 1
	HandshakeTimeout              = 10 * time.Second
)

// ErrNetworkIDMismatch is returned when the remote peer reports a different Network ID.
var ErrNetworkIDMismatch = errors.New("peer: network ID mismatch — rejecting peer")

// ErrProtocolVersionIncompatible is returned when the remote protocol version is unsupported.
var ErrProtocolVersionIncompatible = errors.New("peer: incompatible protocol version")

// Peer models a single remote peer connection with its associated state.
type Peer struct {
	mu        sync.Mutex
	conn      net.Conn
	id        string
	address   string
	state     State
	networkID uint32
	magic     uint32

	RemoteVersion VersionMsg
}

// New creates a Peer wrapping an established TCP connection.
func New(conn net.Conn, networkID, magic uint32) *Peer {
	return &Peer{
		conn:      conn,
		id:        conn.RemoteAddr().String(),
		address:   conn.RemoteAddr().String(),
		state:     StateConnected,
		networkID: networkID,
		magic:     magic,
	}
}

// ID returns the peer's unique identifier (remote address).
func (p *Peer) ID() string { return p.id }

// Address returns the peer's remote address string.
func (p *Peer) Address() string { return p.address }

// State returns the current lifecycle state of the peer.
func (p *Peer) State() State {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.state
}

func (p *Peer) setState(s State) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.state = s
}

// Close transitions the peer to StateClosing and closes the underlying TCP connection.
func (p *Peer) Close() {
	p.setState(StateClosing)
	_ = p.conn.Close()
	p.setState(StateDisconnected)
}

// Send writes a wire message to the peer's connection.
func (p *Peer) Send(cmd string, payload []byte) error {
	return wire.WriteMessage(p.conn, p.magic, cmd, payload)
}

// Recv reads the next wire message from the peer's connection.
func (p *Peer) Recv() (*wire.Message, error) {
	return wire.ReadMessage(p.conn, p.magic)
}

// PerformHandshake executes the version/verack handshake with the remote peer.
//
// Set isInitiator=true for the party that dials the connection (sends version first).
// Set isInitiator=false for the party that accepts the connection (reads version first).
//
// Returns ErrNetworkIDMismatch or ErrProtocolVersionIncompatible if the peer
// is incompatible; the caller must call Close() after any error.
func (p *Peer) PerformHandshake(localVersion VersionMsg, isInitiator bool) error {
	p.setState(StateHandshake)

	if err := p.conn.SetDeadline(time.Now().Add(HandshakeTimeout)); err != nil {
		return fmt.Errorf("peer: setting handshake deadline: %w", err)
	}
	defer func() { _ = p.conn.SetDeadline(time.Time{}) }()

	var remote VersionMsg

	if isInitiator {
		// Initiator: send version → read version → validate → send verack → read verack
		if err := p.Send(wire.CmdVersion, EncodeVersion(localVersion)); err != nil {
			return fmt.Errorf("peer: sending version: %w", err)
		}
		msg, err := p.Recv()
		if err != nil {
			return fmt.Errorf("peer: receiving version: %w", err)
		}
		if msg.Command != wire.CmdVersion {
			return fmt.Errorf("peer: expected version, got %q", msg.Command)
		}
		remote, err = DecodeVersion(msg.Payload)
		if err != nil {
			return err
		}
	} else {
		// Responder: read version → validate → send version → send verack immediately
		msg, err := p.Recv()
		if err != nil {
			return fmt.Errorf("peer: receiving version: %w", err)
		}
		if msg.Command != wire.CmdVersion {
			return fmt.Errorf("peer: expected version, got %q", msg.Command)
		}
		remote, err = DecodeVersion(msg.Payload)
		if err != nil {
			return err
		}
		if err := p.Send(wire.CmdVersion, EncodeVersion(localVersion)); err != nil {
			return fmt.Errorf("peer: sending version: %w", err)
		}
	}

	// Validate Network ID (applies to both roles).
	if remote.NetworkID != p.networkID {
		_ = p.Send(wire.CmdVerack, nil) // best-effort reject signal
		return ErrNetworkIDMismatch
	}

	// Validate protocol version.
	if remote.ProtocolVersion != CurrentProtocolVersion {
		return ErrProtocolVersionIncompatible
	}

	p.mu.Lock()
	p.RemoteVersion = remote
	p.mu.Unlock()

	if isInitiator {
		// Initiator: send verack → read verack
		if err := p.Send(wire.CmdVerack, nil); err != nil {
			return fmt.Errorf("peer: sending verack: %w", err)
		}
		ack, err := p.Recv()
		if err != nil {
			return fmt.Errorf("peer: receiving verack: %w", err)
		}
		if ack.Command != wire.CmdVerack {
			return fmt.Errorf("peer: expected verack, got %q", ack.Command)
		}
	} else {
		// Responder: read verack → send verack
		ack, err := p.Recv()
		if err != nil {
			return fmt.Errorf("peer: receiving verack: %w", err)
		}
		if ack.Command != wire.CmdVerack {
			return fmt.Errorf("peer: expected verack, got %q", ack.Command)
		}
		if err := p.Send(wire.CmdVerack, nil); err != nil {
			return fmt.Errorf("peer: sending verack: %w", err)
		}
	}

	p.setState(StateReady)
	return nil
}
