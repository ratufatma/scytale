// Package wire implements the Scytale P2P wire message framing protocol.
//
// Wire Message Layout (24-byte fixed header + variable payload):
//
//	[ 4-byte Magic | 12-byte Command (ASCII, null-padded) | 4-byte PayloadLength (uint32 LE) | 4-byte Checksum | Payload ]
//
// Checksum: first 4 bytes of BLAKE3 hash of payload bytes.
package wire

import (
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
)

// Protocol constants.
const (
	// MagicTestnet is the 4-byte network magic for the Scytale testnet (SCY\x01).
	MagicTestnet uint32 = 0x53435901

	// HeaderSize is the fixed byte length of every wire frame header.
	HeaderSize = 4 + 12 + 4 + 4 // 24 bytes

	// MaxPayloadSize is the hard upper bound on a single frame payload (4 MiB).
	MaxPayloadSize uint32 = 4 * 1024 * 1024

	// CommandLen is the fixed width of the Command field in bytes.
	CommandLen = 12
)

// Supported command strings.
const (
	CmdVersion   = "version"
	CmdVerack    = "verack"
	CmdInv       = "inv"
	CmdGetData   = "getdata"
	CmdTx        = "tx"
	CmdBlock     = "block"
	CmdGetBlocks = "getblocks"
	CmdInvBlocks = "invblocks"
	CmdPing      = "ping"
	CmdPong      = "pong"
)

// ErrMagicMismatch is returned when the received magic bytes do not match the expected network magic.
var ErrMagicMismatch = errors.New("wire: magic bytes mismatch")

// ErrChecksumMismatch is returned when the payload checksum does not match the header checksum.
var ErrChecksumMismatch = errors.New("wire: payload checksum mismatch")

// ErrPayloadTooLarge is returned when the declared payload length exceeds MaxPayloadSize.
var ErrPayloadTooLarge = errors.New("wire: payload exceeds maximum allowed size")

// ErrEmptyCommand is returned when the command field is all zero bytes.
var ErrEmptyCommand = errors.New("wire: command field is empty")

// Message represents a fully decoded wire message.
type Message struct {
	Magic   uint32
	Command string
	Payload []byte
}

// checksum computes the first 4 bytes of the BLAKE3-equivalent (sha256 for stdlib purity) of payload.
// NOTE: The spec calls for BLAKE3. Since Go stdlib does not include BLAKE3, and the task
// prohibits adding external consensus-critical deps without protocol lock, we use SHA-256
// as a structurally equivalent checksum primitive. The BLAKE3 library can be swapped in
// via go.mod once the dependency policy is finalized.
func checksum(payload []byte) [4]byte {
	h := sha256.Sum256(payload)
	var cs [4]byte
	copy(cs[:], h[:4])
	return cs
}

// commandToBytes encodes a command string into the fixed 12-byte field (null-padded, ASCII).
func commandToBytes(cmd string) [CommandLen]byte {
	var b [CommandLen]byte
	copy(b[:], []byte(cmd))
	return b
}

// commandFromBytes decodes a 12-byte command field to a trimmed ASCII string.
func commandFromBytes(b [CommandLen]byte) string {
	end := CommandLen
	for end > 0 && b[end-1] == 0 {
		end--
	}
	return string(b[:end])
}

// WriteMessage serializes a Message and writes it to w.
// The function is safe to call concurrently from multiple goroutines on different writers.
func WriteMessage(w io.Writer, magic uint32, cmd string, payload []byte) error {
	if len(payload) > int(MaxPayloadSize) {
		return ErrPayloadTooLarge
	}

	cs := checksum(payload)
	header := make([]byte, HeaderSize)

	binary.BigEndian.PutUint32(header[0:4], magic)
	cmdBytes := commandToBytes(cmd)
	copy(header[4:16], cmdBytes[:])
	binary.LittleEndian.PutUint32(header[16:20], uint32(len(payload)))
	copy(header[20:24], cs[:])

	if _, err := w.Write(header); err != nil {
		return fmt.Errorf("wire: writing header: %w", err)
	}
	if len(payload) > 0 {
		if _, err := w.Write(payload); err != nil {
			return fmt.Errorf("wire: writing payload: %w", err)
		}
	}
	return nil
}

// ReadMessage deserializes one Message from r.
// Returns ErrMagicMismatch, ErrChecksumMismatch, or ErrPayloadTooLarge for protocol violations.
// All errors are non-panicking; callers must disconnect the peer on any error.
func ReadMessage(r io.Reader, expectedMagic uint32) (*Message, error) {
	header := make([]byte, HeaderSize)
	if _, err := io.ReadFull(r, header); err != nil {
		return nil, fmt.Errorf("wire: reading header: %w", err)
	}

	// Validate magic
	magic := binary.BigEndian.Uint32(header[0:4])
	if magic != expectedMagic {
		return nil, ErrMagicMismatch
	}

	// Parse command
	var cmdBytes [CommandLen]byte
	copy(cmdBytes[:], header[4:16])
	cmd := commandFromBytes(cmdBytes)
	if cmd == "" {
		return nil, ErrEmptyCommand
	}

	// Parse payload length
	payloadLen := binary.LittleEndian.Uint32(header[16:20])
	if payloadLen > MaxPayloadSize {
		return nil, ErrPayloadTooLarge
	}

	// Parse header checksum
	var expectedCS [4]byte
	copy(expectedCS[:], header[20:24])

	// Read payload
	payload := make([]byte, payloadLen)
	if payloadLen > 0 {
		if _, err := io.ReadFull(r, payload); err != nil {
			return nil, fmt.Errorf("wire: reading payload: %w", err)
		}
	}

	// Verify checksum
	actualCS := checksum(payload)
	if actualCS != expectedCS {
		return nil, ErrChecksumMismatch
	}

	return &Message{
		Magic:   magic,
		Command: cmd,
		Payload: payload,
	}, nil
}
