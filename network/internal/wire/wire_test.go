package wire_test

import (
	"bytes"
	"testing"

	"github.com/scytale-network/scytale-p2p/internal/wire"
)

// TestFrameEncodeDecodeValid verifies that a valid message round-trips through
// WriteMessage / ReadMessage with identical fields and checksum.
func TestFrameEncodeDecodeValid(t *testing.T) {
	payload := []byte("hello scytale wire protocol")
	var buf bytes.Buffer

	if err := wire.WriteMessage(&buf, wire.MagicTestnet, wire.CmdTx, payload); err != nil {
		t.Fatalf("WriteMessage error: %v", err)
	}

	msg, err := wire.ReadMessage(&buf, wire.MagicTestnet)
	if err != nil {
		t.Fatalf("ReadMessage error: %v", err)
	}

	if msg.Magic != wire.MagicTestnet {
		t.Errorf("magic mismatch: got %08x, want %08x", msg.Magic, wire.MagicTestnet)
	}
	if msg.Command != wire.CmdTx {
		t.Errorf("command mismatch: got %q, want %q", msg.Command, wire.CmdTx)
	}
	if !bytes.Equal(msg.Payload, payload) {
		t.Errorf("payload mismatch: got %q, want %q", msg.Payload, payload)
	}
}

// TestFrameEncodeDecodeEmptyPayload verifies that a message with zero-byte payload round-trips.
func TestFrameEncodeDecodeEmptyPayload(t *testing.T) {
	var buf bytes.Buffer
	if err := wire.WriteMessage(&buf, wire.MagicTestnet, wire.CmdVerack, nil); err != nil {
		t.Fatalf("WriteMessage error: %v", err)
	}
	msg, err := wire.ReadMessage(&buf, wire.MagicTestnet)
	if err != nil {
		t.Fatalf("ReadMessage error: %v", err)
	}
	if msg.Command != wire.CmdVerack {
		t.Errorf("command mismatch: got %q, want %q", msg.Command, wire.CmdVerack)
	}
	if len(msg.Payload) != 0 {
		t.Errorf("expected empty payload, got %d bytes", len(msg.Payload))
	}
}

// TestRejectMismatchedMagic verifies that ReadMessage returns ErrMagicMismatch when
// the magic in the frame does not match the expected magic.
func TestRejectMismatchedMagic(t *testing.T) {
	var buf bytes.Buffer
	// Write with a different magic
	wrongMagic := uint32(0xDEADBEEF)
	if err := wire.WriteMessage(&buf, wrongMagic, wire.CmdPing, []byte("ping")); err != nil {
		t.Fatalf("WriteMessage error: %v", err)
	}

	_, err := wire.ReadMessage(&buf, wire.MagicTestnet)
	if err != wire.ErrMagicMismatch {
		t.Errorf("expected ErrMagicMismatch, got: %v", err)
	}
}

// TestRejectCorruptedChecksum verifies that a single corrupted payload byte causes
// ReadMessage to return ErrChecksumMismatch without panicking.
func TestRejectCorruptedChecksum(t *testing.T) {
	payload := []byte("block payload bytes")
	var buf bytes.Buffer

	if err := wire.WriteMessage(&buf, wire.MagicTestnet, wire.CmdBlock, payload); err != nil {
		t.Fatalf("WriteMessage error: %v", err)
	}

	// Corrupt the last byte of the payload in the buffer
	raw := buf.Bytes()
	if len(raw) > 0 {
		raw[len(raw)-1] ^= 0xFF
	}

	corrupted := bytes.NewReader(raw)
	_, err := wire.ReadMessage(corrupted, wire.MagicTestnet)
	if err != wire.ErrChecksumMismatch {
		t.Errorf("expected ErrChecksumMismatch, got: %v", err)
	}
}

// TestRejectOversizedPayload verifies that WriteMessage rejects payloads that exceed
// MaxPayloadSize without allocating the oversized buffer.
func TestRejectOversizedPayload(t *testing.T) {
	oversized := make([]byte, wire.MaxPayloadSize+1)
	var buf bytes.Buffer
	err := wire.WriteMessage(&buf, wire.MagicTestnet, wire.CmdBlock, oversized)
	if err != wire.ErrPayloadTooLarge {
		t.Errorf("expected ErrPayloadTooLarge, got: %v", err)
	}
}

// TestAllCommandsRoundTrip verifies that all defined command strings survive a
// full encode/decode cycle without truncation or garbling.
func TestAllCommandsRoundTrip(t *testing.T) {
	commands := []string{
		wire.CmdVersion, wire.CmdVerack, wire.CmdInv, wire.CmdGetData,
		wire.CmdTx, wire.CmdBlock, wire.CmdGetBlocks, wire.CmdInvBlocks,
		wire.CmdPing, wire.CmdPong,
	}
	for _, cmd := range commands {
		var buf bytes.Buffer
		if err := wire.WriteMessage(&buf, wire.MagicTestnet, cmd, []byte("data")); err != nil {
			t.Fatalf("WriteMessage(%q) error: %v", cmd, err)
		}
		msg, err := wire.ReadMessage(&buf, wire.MagicTestnet)
		if err != nil {
			t.Fatalf("ReadMessage(%q) error: %v", cmd, err)
		}
		if msg.Command != cmd {
			t.Errorf("command roundtrip failed: got %q, want %q", msg.Command, cmd)
		}
	}
}
