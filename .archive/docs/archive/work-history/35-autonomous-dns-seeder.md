# Scytale Protocol — Technical Specification & Architecture Record: Task 35
## Autonomous DNS Seeder (`network/cmd/scytale-seeder` & `network/internal/seeder`)

```text
Task ID       : 35
Task Name     : Autonomous DNS Seeder (network/cmd/scytale-seeder & network/internal/seeder)
Phase         : Phase 4 — Network Bootstrap & Production Tooling
Target Modul  : network/cmd/scytale-seeder, network/internal/seeder
Reference     : network/internal/wire, network/cmd/scytale-p2p
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Zero Race Condition, Anti-Sybil Subnet Limit (<= 2 nodes /24), Authoritative DNS Response (TTL 60s), Safe Atomic Persistence
```

---

## 1. Arsitektur DNS Seeder (Cold-Start Bootstrap)

Untuk memecahkan masalah *cold-start bootstrap* pada node baru tanpa ketergantungan pada daftar IP statis yang cepat usang, Scytale menyediakan daemon DNS Seeder otonom yang memantau kesehatan jaringan secara aktif dan menyajikan alamat node sehat melalui protokol standar DNS (RFC 1035):

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           SCYTALE DNS SEEDER DAEMON                             │
│                                                                                 │
│   [ Network Crawler / Prober ]                                                  │
│   • Worker Pool (default: 16 goroutines)                                        │
│   • Dial TCP Port (default: 9001) with 3s timeout                               │
│   • Handshake: Send wire.MsgVersion ──► Recv wire.MsgVerack                     │
│   • Discover: Send wire.MsgGetAddr ──► Recv wire.MsgAddr (Ingest new peers)     │
│   • Reschedule with Exponential Backoff (max 6h) on probe failure               │
│                                                                                 │
│   [ Node Reputation Ledger (Store) ]                                            │
│   • Thread-safe In-Memory Map with sync.RWMutex                                 │
│   • Heuristic "Good Node" Rule:                                                 │
│     - LastSuccess <= 2 hours ago                                                │
│     - Success ratio >= 70% (if TotalAttempts >= 3)                              │
│     - BestHeight within 288 blocks of median network height                     │
│   • Anti-Sybil Filter: Max 2 IPs per /24 subnet (IPv4) or /48 (IPv6)            │
│   • Atomic File Persistence (seeder_nodes.json via tmpfile + rename)            │
│                                                                                 │
│   [ Authoritative DNS Server ]                                                  │
│   • Dual Listener: UDP & TCP port 53 / custom listen port (miekg/dns)           │
│   • Serves Type A (IPv4) and Type AAAA (IPv6) records (TTL 60s)                 │
│   • Serves Type NS records pointing to authoritative Nameserver                 │
│   • Fisher-Yates shuffle with random subset (up to 16 records per query)        │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Spesifikasi Paket & Komponen

### A. Konfigurasi (`network/internal/seeder/config.go`)
- `Domain`: Domain DNS yang dilayani (misal: `"seed.scytale.org"`).
- `Nameserver`: FQDN nameserver otoritatif (misal: `"ns1.seed.scytale.org"`).
- `ListenAddr`: Alamat bind DNS (default `":53"`, mendukung port non-privilese seperti `":1053"`).
- `P2PPort`: Port P2P default target crawler (default `9001`).
- `Seeds`: Daftar alamat IP seed awal / bootstrap.
- `DataFile`: Lokasi file penyimpanan persistensi status node (default `"seeder_nodes.json"`).
- `Workers`: Jumlah worker prober paralel (default `16`).
- `ProbeInterval`: Interval berkala evaluasi ulang node sehat (default `15 * time.Minute`).

### B. Penyimpanan Reputasi & Anti-Sybil (`network/internal/seeder/store.go`)
- Record struktur per node:
  ```go
  type NodeRecord struct {
      IP            net.IP    `json:"ip"`
      Port          uint16    `json:"port"`
      ProtocolVer   uint32    `json:"protocol_ver"`
      Services      uint64    `json:"services"`
      BestHeight    uint64    `json:"best_height"`
      LastSuccess   time.Time `json:"last_success"`
      LastAttempt   time.Time `json:"last_attempt"`
      SuccessCount  int       `json:"success_count"`
      TotalAttempts int       `json:"total_attempts"`
      FailStreak    int       `json:"fail_streak"`
  }
  ```
- Syarat Node Sehat (`IsGood(medianHeight uint64)`):
  1. `LastSuccess` dalam kurun waktu $\le 2$ jam terakhir.
  2. Rasio sukses $\ge 70\%$ jika `TotalAttempts >= 3`.
  3. `BestHeight` $\ge \text{medianHeight} - 288$ blok.
- Mitigasi Serangan Sybil:
  Maksimal **2 alamat IP** per subnet `/24` (IPv4) atau subnet `/48` (IPv6) dikembalikan ke resolver DNS dalam satu kueri.
- Persistensi Atomik:
  Menulis ke file sementara (`.tmp`) dan melakukan `os.Rename` untuk mencegah korupsi file saat crash.

### C. Network Crawler & TCP Prober (`network/internal/seeder/crawler.go`)
- Menjalankan antrean probe dengan worker pool.
- Melakukan TCP connect (`net.DialTimeout` 3 detik).
- Mengirim pesan `wire.MsgVersion` Scytale resmi.
- Menunggu `wire.MsgVerack` untuk konfirmasi keaktifan.
- Mengirim `wire.MsgGetAddr` untuk memperluas penemuan topologi jaringan.
- Memproses balasan `wire.MsgAddr` dan menambahkan kandidat baru ke `Store`.
- Menjadwalkan ulang kegagalan dengan *exponential backoff* hingga maksimal 6 jam.

### D. Authoritative DNS Server (`network/internal/seeder/server.go`)
- Menggunakan pustaka `github.com/miekg/dns`.
- Melayani UDP dan TCP secara simultan.
- Mengimplementasikan `dns.Handler`:
  - Validasi domain kueri terhadap `Config.Domain` (case-insensitive, trailing dot support).
  - Melayani kueri tipe `A`, `AAAA`, dan `NS`.
  - Mengacak IP aktif dengan algoritma Fisher-Yates dan membatasi jawaban hingga 16 record per respons.
  - Menetapkan TTL 60 detik dan bendera `Authoritative = true`.

### E. Binary Entrypoint (`network/cmd/scytale-seeder/main.go`)
- Parsing argumen baris perintah (`flag`).
- Menjalankan Crawler dan DNS Server secara bersamaan (*concurrent*).
- Menangani sinyal terminasi (`os.Interrupt`, `syscall.SIGTERM`) untuk *graceful shutdown* dan penyimpanan data terakhir ke disk.

---

## 3. Quality Gates & Verification Matrix

1. `network/go.mod` memuat dependensi resmi `github.com/miekg/dns`.
2. Unit test `store_test.go` lulus pengujian:
   - Penolakan node usang (*stale nodes*).
   - Penegakan kuota anti-Sybil subnet `/24`.
   - Simpan dan muat atomik JSON roundtrip.
3. Unit test `server_test.go` lulus pengujian respons DNS A/NS dengan format RFC standar.
4. `go test -v -race ./internal/seeder/...` dan `go test -v -race ./...` lulus 100% bebas race condition.
5. Binary `scytale-seeder` berhasil dikompilasi dengan `go build -v ./cmd/scytale-seeder`.

---

## 4. Hasil Verifikasi & Eksekusi Unit Test

```text
=== RUN   TestCrawler_ProbeAndDiscovery
--- PASS: TestCrawler_ProbeAndDiscovery (3.01s)
=== RUN   TestDNSServer_ServeDNS_TypeA
--- PASS: TestDNSServer_ServeDNS_TypeA (0.00s)
=== RUN   TestDNSServer_ServeDNS_TypeNS
--- PASS: TestDNSServer_ServeDNS_TypeNS (0.00s)
=== RUN   TestDNSServer_ServeDNS_ForeignDomain
--- PASS: TestDNSServer_ServeDNS_ForeignDomain (0.00s)
=== RUN   TestDNSServer_LiveExchange
--- PASS: TestDNSServer_LiveExchange (0.00s)
=== RUN   TestStore_IsGood
--- PASS: TestStore_IsGood (0.00s)
=== RUN   TestStore_AntiSybilSubnetLimit
--- PASS: TestStore_AntiSybilSubnetLimit (0.00s)
=== RUN   TestStore_SaveAndLoadAtomic
--- PASS: TestStore_SaveAndLoadAtomic (0.01s)
=== RUN   TestStore_ConcurrentAccess
--- PASS: TestStore_ConcurrentAccess (0.12s)
PASS
ok      github.com/scytale-network/scytale-p2p/internal/seeder  4.150s

Binary Compilation:
go build -v ./cmd/scytale-seeder -> SUCCESS (0 error, clean exit)
```
