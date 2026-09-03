package seeder

import (
	"crypto/rand"
	"fmt"
	"math/big"
	"net"
	"strings"
	"sync"

	"github.com/miekg/dns"
)

// DNSServer provides an authoritative, lightweight DNS service responding to A, AAAA, and NS queries.
type DNSServer struct {
	cfg       *Config
	store     *Store
	udpServer *dns.Server
	tcpServer *dns.Server
	wg        sync.WaitGroup
}

// NewDNSServer creates a new DNSServer configured with the provided settings and store.
func NewDNSServer(cfg *Config, store *Store) *DNSServer {
	s := &DNSServer{
		cfg:   cfg,
		store: store,
	}

	s.udpServer = &dns.Server{
		Addr:    cfg.ListenAddr,
		Net:     "udp",
		Handler: s,
	}

	s.tcpServer = &dns.Server{
		Addr:    cfg.ListenAddr,
		Net:     "tcp",
		Handler: s,
	}

	return s
}

// Start launches both UDP and TCP DNS listeners concurrently.
func (s *DNSServer) Start() error {
	udpReady := make(chan error, 1)
	tcpReady := make(chan error, 1)

	// Launch UDP listener
	s.wg.Add(1)
	go func() {
		defer s.wg.Done()
		s.udpServer.NotifyStartedFunc = func() {
			udpReady <- nil
		}
		if err := s.udpServer.ListenAndServe(); err != nil {
			select {
			case udpReady <- err:
			default:
			}
		}
	}()

	// Launch TCP listener
	s.wg.Add(1)
	go func() {
		defer s.wg.Done()
		s.tcpServer.NotifyStartedFunc = func() {
			tcpReady <- nil
		}
		if err := s.tcpServer.ListenAndServe(); err != nil {
			select {
			case tcpReady <- err:
			default:
			}
		}
	}()

	// Wait for listeners to start or fail
	for i := 0; i < 2; i++ {
		select {
		case err := <-udpReady:
			if err != nil {
				_ = s.Shutdown()
				return fmt.Errorf("seeder dns: failed to start UDP listener on %s: %w", s.cfg.ListenAddr, err)
			}
		case err := <-tcpReady:
			if err != nil {
				_ = s.Shutdown()
				return fmt.Errorf("seeder dns: failed to start TCP listener on %s: %w", s.cfg.ListenAddr, err)
			}
		}
	}

	return nil
}

// Shutdown gracefully stops both UDP and TCP listeners.
func (s *DNSServer) Shutdown() error {
	var errUDP, errTCP error
	if s.udpServer != nil {
		errUDP = s.udpServer.Shutdown()
	}
	if s.tcpServer != nil {
		errTCP = s.tcpServer.Shutdown()
	}
	s.wg.Wait()

	if errUDP != nil {
		return errUDP
	}
	return errTCP
}

// matchesDomain checks whether qName matches the configured domain (case-insensitive, trailing dot normalized).
func matchesDomain(qName, targetDomain string) bool {
	normQ := strings.ToLower(strings.TrimSuffix(qName, "."))
	normTarget := strings.ToLower(strings.TrimSuffix(targetDomain, "."))
	return normQ == normTarget
}

// secureShuffle performs a Fisher-Yates shuffle on an IP slice using cryptographically secure randomness.
func secureShuffle(ips []net.IP) {
	n := len(ips)
	for i := n - 1; i > 0; i-- {
		nBig := big.NewInt(int64(i + 1))
		jBig, err := rand.Int(rand.Reader, nBig)
		if err != nil {
			continue
		}
		j := int(jBig.Int64())
		ips[i], ips[j] = ips[j], ips[i]
	}
}

// ServeDNS implements the miekg/dns Handler interface.
func (s *DNSServer) ServeDNS(w dns.ResponseWriter, r *dns.Msg) {
	m := new(dns.Msg)
	m.SetReply(r)
	m.Authoritative = true

	if len(r.Question) == 0 {
		_ = w.WriteMsg(m)
		return
	}

	q := r.Question[0]

	// Verify query domain matches configured seed domain
	if !matchesDomain(q.Name, s.cfg.Domain) {
		m.Rcode = dns.RcodeNameError
		_ = w.WriteMsg(m)
		return
	}

	switch q.Qtype {
	case dns.TypeA:
		goodIPs := s.store.GetGoodNodes()
		var v4List []net.IP
		for _, ip := range goodIPs {
			if v4 := ip.To4(); v4 != nil {
				v4List = append(v4List, v4)
			}
		}

		secureShuffle(v4List)
		if len(v4List) > 16 {
			v4List = v4List[:16]
		}

		for _, ip := range v4List {
			rr := &dns.A{
				Hdr: dns.RR_Header{
					Name:   q.Name,
					Rrtype: dns.TypeA,
					Class:  dns.ClassINET,
					Ttl:    60,
				},
				A: ip,
			}
			m.Answer = append(m.Answer, rr)
		}

	case dns.TypeAAAA:
		goodIPs := s.store.GetGoodNodes()
		var v6List []net.IP
		for _, ip := range goodIPs {
			if ip.To4() == nil && ip.To16() != nil {
				v6List = append(v6List, ip.To16())
			}
		}

		secureShuffle(v6List)
		if len(v6List) > 16 {
			v6List = v6List[:16]
		}

		for _, ip := range v6List {
			rr := &dns.AAAA{
				Hdr: dns.RR_Header{
					Name:   q.Name,
					Rrtype: dns.TypeAAAA,
					Class:  dns.ClassINET,
					Ttl:    60,
				},
				AAAA: ip,
			}
			m.Answer = append(m.Answer, rr)
		}

	case dns.TypeNS:
		nsTarget := s.cfg.Nameserver
		if !strings.HasSuffix(nsTarget, ".") {
			nsTarget += "."
		}
		rr := &dns.NS{
			Hdr: dns.RR_Header{
				Name:   q.Name,
				Rrtype: dns.TypeNS,
				Class:  dns.ClassINET,
				Ttl:    60,
			},
			Ns: nsTarget,
		}
		m.Answer = append(m.Answer, rr)

	default:
		// Unsupported query type on seed domain returns NOERROR with empty answer
	}

	_ = w.WriteMsg(m)
}
