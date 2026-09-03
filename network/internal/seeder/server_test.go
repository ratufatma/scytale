package seeder

import (
	"net"
	"testing"

	"github.com/miekg/dns"
)

// mockResponseWriter captures response messages for direct unit testing of ServeDNS.
type mockResponseWriter struct {
	msg *dns.Msg
}

func (m *mockResponseWriter) LocalAddr() net.Addr       { return &net.UDPAddr{IP: net.ParseIP("127.0.0.1"), Port: 53} }
func (m *mockResponseWriter) RemoteAddr() net.Addr      { return &net.UDPAddr{IP: net.ParseIP("127.0.0.1"), Port: 12345} }
func (m *mockResponseWriter) WriteMsg(msg *dns.Msg) error {
	m.msg = msg
	return nil
}
func (m *mockResponseWriter) Write(b []byte) (int, error) { return len(b), nil }
func (m *mockResponseWriter) Close() error               { return nil }
func (m *mockResponseWriter) TsigStatus() error          { return nil }
func (m *mockResponseWriter) TsigTimersOnly(bool)        {}
func (m *mockResponseWriter) Hijack()                    {}

func TestDNSServer_ServeDNS_TypeA(t *testing.T) {
	cfg := &Config{
		Domain:     "seed.scytale.org",
		Nameserver: "ns1.seed.scytale.org",
		ListenAddr: ":1053",
	}
	store := NewStore()

	// Seed 3 good nodes across distinct /24 subnets
	ips := []string{"192.0.2.1", "198.51.100.1", "203.0.113.1"}
	for _, ipStr := range ips {
		ip := net.ParseIP(ipStr)
		store.AddNode(ip, 9001)
		store.RecordAttempt(ip, 9001)
		store.RecordSuccess(ip, 9001, 1, 1, 100)
	}

	server := NewDNSServer(cfg, store)

	// Build query for "seed.scytale.org."
	query := new(dns.Msg)
	query.SetQuestion("seed.scytale.org.", dns.TypeA)

	w := &mockResponseWriter{}
	server.ServeDNS(w, query)

	if w.msg == nil {
		t.Fatalf("expected response message, got nil")
	}

	if !w.msg.Authoritative {
		t.Errorf("expected Authoritative = true")
	}

	if w.msg.Rcode != dns.RcodeSuccess {
		t.Errorf("expected RcodeSuccess, got %d", w.msg.Rcode)
	}

	if len(w.msg.Answer) != 3 {
		t.Fatalf("expected 3 answers, got %d", len(w.msg.Answer))
	}

	for _, ans := range w.msg.Answer {
		aRecord, ok := ans.(*dns.A)
		if !ok {
			t.Fatalf("expected dns.A record, got %T", ans)
		}
		if aRecord.Hdr.Ttl != 60 {
			t.Errorf("expected TTL 60, got %d", aRecord.Hdr.Ttl)
		}
		if aRecord.A == nil || aRecord.A.To4() == nil {
			t.Errorf("unexpected or nil IPv4: %v", aRecord.A)
		}
	}
}

func TestDNSServer_ServeDNS_TypeNS(t *testing.T) {
	cfg := &Config{
		Domain:     "seed.scytale.org",
		Nameserver: "ns1.seed.scytale.org",
		ListenAddr: ":1053",
	}
	store := NewStore()
	server := NewDNSServer(cfg, store)

	query := new(dns.Msg)
	query.SetQuestion("seed.scytale.org.", dns.TypeNS)

	w := &mockResponseWriter{}
	server.ServeDNS(w, query)

	if w.msg == nil || len(w.msg.Answer) != 1 {
		t.Fatalf("expected 1 NS answer, got %v", w.msg)
	}

	nsRecord, ok := w.msg.Answer[0].(*dns.NS)
	if !ok {
		t.Fatalf("expected dns.NS record, got %T", w.msg.Answer[0])
	}

	if nsRecord.Ns != "ns1.seed.scytale.org." {
		t.Errorf("expected ns1.seed.scytale.org., got %s", nsRecord.Ns)
	}
}

func TestDNSServer_ServeDNS_ForeignDomain(t *testing.T) {
	cfg := &Config{
		Domain:     "seed.scytale.org",
		Nameserver: "ns1.seed.scytale.org",
		ListenAddr: ":1053",
	}
	store := NewStore()
	server := NewDNSServer(cfg, store)

	query := new(dns.Msg)
	query.SetQuestion("evil.attacker.com.", dns.TypeA)

	w := &mockResponseWriter{}
	server.ServeDNS(w, query)

	if w.msg == nil {
		t.Fatalf("expected response message, got nil")
	}

	if w.msg.Rcode != dns.RcodeNameError {
		t.Errorf("expected NXDOMAIN (RcodeNameError), got %d", w.msg.Rcode)
	}
}

func TestDNSServer_LiveExchange(t *testing.T) {
	// Pick random high port for testing
	listenAddr := "127.0.0.1:15353"
	cfg := &Config{
		Domain:     "seed.scytale.org",
		Nameserver: "ns1.seed.scytale.org",
		ListenAddr: listenAddr,
	}
	store := NewStore()

	ip := net.ParseIP("198.51.100.42")
	store.AddNode(ip, 9001)
	store.RecordAttempt(ip, 9001)
	store.RecordSuccess(ip, 9001, 1, 1, 500)

	server := NewDNSServer(cfg, store)
	if err := server.Start(); err != nil {
		t.Fatalf("failed to start DNS server: %v", err)
	}
	defer func() { _ = server.Shutdown() }()

	// Query via UDP
	c := new(dns.Client)
	c.Net = "udp"

	m := new(dns.Msg)
	m.SetQuestion("seed.scytale.org.", dns.TypeA)

	r, _, err := c.Exchange(m, listenAddr)
	if err != nil {
		t.Fatalf("dns exchange error: %v", err)
	}

	if r.Rcode != dns.RcodeSuccess {
		t.Fatalf("expected RcodeSuccess, got %d", r.Rcode)
	}

	if len(r.Answer) != 1 {
		t.Fatalf("expected 1 answer, got %d", len(r.Answer))
	}

	aRecord, ok := r.Answer[0].(*dns.A)
	if !ok || !aRecord.A.Equal(ip) {
		t.Fatalf("expected IP %s, got %v", ip, aRecord)
	}
}
