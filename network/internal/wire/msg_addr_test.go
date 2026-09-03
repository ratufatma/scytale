package wire

import (
	"bytes"
	"net"
	"testing"
	"time"
)

func TestAddrEncodeDecodeRoundtrip(t *testing.T) {
	now := time.Now().Unix()

	original := []NetAddress{
		{
			Timestamp: now,
			Services:  1,
			IP:        net.ParseIP("192.168.1.100").To16(),
			Port:      9001,
		},
		{
			Timestamp: now + 50,
			Services:  0,
			IP:        net.ParseIP("2001:db8::1").To16(),
			Port:      9002,
		},
		{
			Timestamp: now - 100,
			Services:  3,
			IP:        net.ParseIP("127.0.0.1").To16(),
			Port:      8333,
		},
	}

	encoded := EncodeAddr(original)
	decoded, err := DecodeAddr(encoded)
	if err != nil {
		t.Fatalf("DecodeAddr failed: %v", err)
	}

	if len(decoded) != len(original) {
		t.Fatalf("expected %d addrs, got %d", len(original), len(decoded))
	}

	for i := range original {
		if decoded[i].Timestamp != original[i].Timestamp {
			t.Errorf("[%d] expected Timestamp %d, got %d", i, original[i].Timestamp, decoded[i].Timestamp)
		}
		if decoded[i].Services != original[i].Services {
			t.Errorf("[%d] expected Services %d, got %d", i, original[i].Services, decoded[i].Services)
		}
		if !bytes.Equal(decoded[i].IP, original[i].IP) {
			t.Errorf("[%d] expected IP %v, got %v", i, original[i].IP, decoded[i].IP)
		}
		if decoded[i].Port != original[i].Port {
			t.Errorf("[%d] expected Port %d, got %d", i, original[i].Port, decoded[i].Port)
		}
	}
}

func TestAddrEmptyPayload(t *testing.T) {
	encoded := EncodeAddr(nil)
	if len(encoded) != 4 {
		t.Fatalf("expected 4-byte count prefix for empty slice, got %d", len(encoded))
	}

	decoded, err := DecodeAddr(encoded)
	if err != nil {
		t.Fatalf("DecodeAddr on empty slice failed: %v", err)
	}
	if len(decoded) != 0 {
		t.Fatalf("expected 0 addrs, got %d", len(decoded))
	}
}

func TestAddrShortPayloadError(t *testing.T) {
	// Less than 4 bytes
	if _, err := DecodeAddr([]byte{1, 2}); err == nil {
		t.Errorf("expected error for payload < 4 bytes")
	}

	// Declared 1 addr (needs 4 + 34 bytes), but only 10 bytes provided
	truncated := make([]byte, 10)
	truncated[0] = 1 // count = 1
	if _, err := DecodeAddr(truncated); err == nil {
		t.Errorf("expected ErrAddrPayloadTooShort for truncated payload")
	}
}

func TestNewNetAddressFromString(t *testing.T) {
	na, err := NewNetAddressFromString("127.0.0.1:9001", 1)
	if err != nil {
		t.Fatalf("NewNetAddressFromString failed: %v", err)
	}
	if na.Port != 9001 {
		t.Errorf("expected port 9001, got %d", na.Port)
	}
	if !na.IP.Equal(net.ParseIP("127.0.0.1")) {
		t.Errorf("expected IP 127.0.0.1, got %s", na.IP)
	}

	if na.String() != "127.0.0.1:9001" {
		t.Errorf("expected string 127.0.0.1:9001, got %s", na.String())
	}
}
