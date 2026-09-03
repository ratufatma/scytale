package wire

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net"
	"strconv"
	"time"
)

const (
	// NetAddressWireSize is the fixed byte length of a single serialized NetAddress:
	// 8 bytes (Timestamp) + 8 bytes (Services) + 16 bytes (IP) + 2 bytes (Port) = 34 bytes.
	NetAddressWireSize = 8 + 8 + 16 + 2

	// MaxAddrsPerMsg is the maximum number of addresses permitted in a single `addr` message.
	MaxAddrsPerMsg = 1000
)

var (
	// ErrTooManyAddresses is returned when an addr message specifies more than MaxAddrsPerMsg.
	ErrTooManyAddresses = errors.New("wire: addr message exceeds maximum address count (1000)")

	// ErrAddrPayloadTooShort is returned when the payload is too short to decode the declared count.
	ErrAddrPayloadTooShort = errors.New("wire: addr payload too short")
)

// NetAddress models an address of a peer on the Scytale network.
type NetAddress struct {
	Timestamp int64  // Unix seconds
	Services  uint64 // Bitfield of features supported by the node
	IP        net.IP // 16-byte representation (IPv4-mapped or IPv6)
	Port      uint16 // Network port (1..65535)
}

// String returns the host:port string representation of the address.
func (na *NetAddress) String() string {
	if na.IP == nil {
		return net.JoinHostPort("0.0.0.0", strconv.Itoa(int(na.Port)))
	}
	return net.JoinHostPort(na.IP.String(), strconv.Itoa(int(na.Port)))
}

// NewNetAddress creates a NetAddress from a TCPAddr.
func NewNetAddress(addr *net.TCPAddr, services uint64) *NetAddress {
	ip := addr.IP
	if ip == nil {
		ip = net.IPv4zero
	}
	return &NetAddress{
		Timestamp: time.Now().Unix(),
		Services:  services,
		IP:        ip.To16(),
		Port:      uint16(addr.Port),
	}
}

// NewNetAddressFromString parses a host:port string into a NetAddress.
func NewNetAddressFromString(addrStr string, services uint64) (*NetAddress, error) {
	host, portStr, err := net.SplitHostPort(addrStr)
	if err != nil {
		return nil, fmt.Errorf("invalid address %q: %w", addrStr, err)
	}

	portNum, err := strconv.ParseUint(portStr, 10, 16)
	if err != nil {
		return nil, fmt.Errorf("invalid port %q: %w", portStr, err)
	}

	ip := net.ParseIP(host)
	if ip == nil {
		// Attempt DNS resolution for hostnames (e.g. Docker container names)
		ips, err := net.LookupIP(host)
		if err != nil || len(ips) == 0 {
			return nil, fmt.Errorf("could not resolve host %q: %w", host, err)
		}
		ip = ips[0]
	}

	return &NetAddress{
		Timestamp: time.Now().Unix(),
		Services:  services,
		IP:        ip.To16(),
		Port:      uint16(portNum),
	}, nil
}

// EncodeAddr serializes a slice of NetAddress records into a wire `addr` payload.
func EncodeAddr(addrs []NetAddress) []byte {
	count := len(addrs)
	if count > MaxAddrsPerMsg {
		count = MaxAddrsPerMsg
	}

	buf := make([]byte, 4+count*NetAddressWireSize)
	binary.LittleEndian.PutUint32(buf[0:4], uint32(count))

	offset := 4
	for i := 0; i < count; i++ {
		addr := addrs[i]

		// 8 bytes: Timestamp
		binary.LittleEndian.PutUint64(buf[offset:offset+8], uint64(addr.Timestamp))
		offset += 8

		// 8 bytes: Services
		binary.LittleEndian.PutUint64(buf[offset:offset+8], addr.Services)
		offset += 8

		// 16 bytes: IP (standardized 16-byte representation)
		ip16 := addr.IP.To16()
		if ip16 == nil {
			ip16 = net.IPv4zero.To16()
		}
		copy(buf[offset:offset+16], ip16)
		offset += 16

		// 2 bytes: Port (BigEndian network byte order)
		binary.BigEndian.PutUint16(buf[offset:offset+2], addr.Port)
		offset += 2
	}

	return buf
}

// DecodeAddr deserializes an `addr` payload into a slice of NetAddress records.
func DecodeAddr(payload []byte) ([]NetAddress, error) {
	if len(payload) < 4 {
		return nil, ErrAddrPayloadTooShort
	}

	count := binary.LittleEndian.Uint32(payload[0:4])
	if count > MaxAddrsPerMsg {
		return nil, ErrTooManyAddresses
	}

	expectedLen := 4 + int(count)*NetAddressWireSize
	if len(payload) < expectedLen {
		return nil, ErrAddrPayloadTooShort
	}

	addrs := make([]NetAddress, 0, count)
	offset := 4

	for i := uint32(0); i < count; i++ {
		ts := int64(binary.LittleEndian.Uint64(payload[offset : offset+8]))
		offset += 8

		services := binary.LittleEndian.Uint64(payload[offset : offset+8])
		offset += 8

		ip := make(net.IP, 16)
		copy(ip, payload[offset:offset+16])
		offset += 16

		port := binary.BigEndian.Uint16(payload[offset : offset+2])
		offset += 2

		addrs = append(addrs, NetAddress{
			Timestamp: ts,
			Services:  services,
			IP:        ip,
			Port:      port,
		})
	}

	return addrs, nil
}
