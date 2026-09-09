# Scytale Protocol — Technical Specification & Architecture Record: Task 36
## Dynamic DNS Seeder Client & Cold-Start Mesh Bootstrapping (`network/internal/peer` & `network/cmd/scytale-p2p`)

```text
Task ID       : 36
Task Name     : Dynamic DNS Seeder Client & Cold-Start Mesh Bootstrapping
Phase         : Phase 4 — Network Bootstrap & Production Tooling
Target Modul  : network/internal/peer, network/cmd/scytale-p2p
Reference     : network/internal/seeder, network/internal/peer/addrbook.go
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Non-blocking DNS Resolution, Anti-Stall Fallback, Safe Default Seed List, Zero Data Race
```

---

## 1. Arsitektur Cold-Start Bootstrap Otomatis

Pada node baru atau node yang baru diinstal, `AddrBook` lokal berada dalam kondisi kosong (`Size() == 0`) dan daftar static peer CLI (`--peer`) mungkin tidak diisi oleh pengguna. Tanpa mekanisme bootstrap otomatis, node akan terjebak (*stall*) dan tidak dapat menemukan rekanan (*peers*).

Task 36 mengintegrasikan kapabilitas klien DNS Seeder ke dalam daemon `scytale-p2p` sehingga node secara otonom meminta daftar alamat IP peer aktif dari satu atau lebih domain DNS seeder (misal: `seed.scytale.org`) saat `AddrBook` kosong atau ketika target koneksi keluar (*outbound*) belum terpenuhi.

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                      SCYTALE P2P DAEMON (scytale-p2p)                           │
│                                                                                 │
│   1. Startup & Address Book Check:                                              │
│      • Muat peers.json ──► addrBook.Size() == 0?                                │
│      ├── Ya (Cold Start) ──► Pemicu Resolusi DNS Seeder                         │
│      └── Tidak           ──► Lanjut Auto-Dialer Biasa                           │
│                                                                                 │
│   2. Dynamic DNS Seed Resolver (internal/peer/dns.go):                          │
│      • Query Domain: seed.scytale.org (atau flag --dns-seed)                    │
│      • LookupIP (A & AAAA Records) dengan timeout                               │
│      • Gabungkan dengan default P2P port (9001)                                 │
│      • Ingest hasil ke AddrBook dengan src = "dns-seed"                         │
│                                                                                 │
│   3. Anti-Stall Auto-Dialer Fallback:                                           │
│      • Jika outbound connections < max-outbound dan AddrBook kehabisan         │
│        kandidat yang dapat didial, picu ulang kueri DNS dengan interval         │
│        eksponensial (backoff: 1m, 2m, 5m, 15m)                                  │
│      • Mencegah pemblokiran thread utama (asinkron via goroutine)               │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Spesifikasi Modul & Komponen

### A. Modul Klien DNS Seeder (`network/internal/peer/dns.go` & `dns_test.go`)
- Fungsi Resolver:
  ```go
  type LookupIPFunc func(host string) ([]net.IP, error)

  func ResolveDNSSeeds(seeds []string, defaultPort uint16, lookup LookupIPFunc) []string
  ```
- **Invarian & Fitur**:
  - Mendukung input domain murni (`"seed.scytale.org"`) maupun format `host:port` kustom (`"seed.scytale.org:9002"`).
  - Mengabaikan IP yang tidak dapat dirutekan (unspecified, multicast).
  - Mengembalikan daftar alamat unik (`host:port`).
  - Menerima parameter fungsi injeksi `lookup` untuk memfasilitasi unit testing deterministik tanpa ketergantungan koneksi internet publik.

### B. Integrasi CLI & Daemon (`network/cmd/scytale-p2p/main.go`)
- Penambahan Flag CLI:
  - `--dns-seed`: Alamat domain DNS seeder (dapat ditentukan berulang kali, default: `"seed.scytale.org"`).
  - `--no-dns-seeds`: Opsi boolean untuk menonaktifkan resolusi DNS seeder (berguna untuk isolated cluster atau testing lokal).
- Alur Eksekusi Startup:
  - Setelah `addrBook` diinisialisasi, jika `addrBook.Size() == 0` dan tidak ada flag `--no-dns-seeds`:
    Jalankan resolusi DNS seeder di latar belakang (*background goroutine*).
  - Masukkan alamat-alamat yang ditemukan ke dalam `addrBook.AddAddresses(addrs, "dns-seed")`.
  - Picu channel `triggerDial` agar auto-dialer langsung memulai koneksi keluar ke rekan-rekan baru tersebut.
- Penjaga Anti-Stall (*Periodic Fallback*):
  - Pada `autoDialerLoop()`, jika `outboundCount < d.maxOutbound` dan `addrBook.Size() == 0` (atau seluruh kandidat sedang dalam status cooldown/gagal), jadwalkan ulang resolusi DNS seeder secara berkala (misal: setiap 3 menit).

---

## 3. Quality Gates & Verification Plan

1. Unit test `dns_test.go` di `network/internal/peer/` lulus pengujian:
   - Resolusi multi-seed dengan mock resolver.
   - Penanganan port kustom vs port default.
   - Eliminasi duplikasi IP dan filter alamat tak valid.
   - Penanganan kegagalan DNS gracefully (fallback tanpa panic).
2. Daemon `scytale-p2p` berhasil mengompilasi (`go build -v ./cmd/scytale-p2p`).
3. Seluruh unit test dan race detector di `network/` lulus 100% (`go test -v -race ./...`).

---

## 4. Hasil Verifikasi & Eksekusi

```text
=== RUN   TestResolveDNSSeeds_MockLookup
--- PASS: TestResolveDNSSeeds_MockLookup (0.00s)
=== RUN   TestResolveDNSSeeds_FilterUnroutable
--- PASS: TestResolveDNSSeeds_FilterUnroutable (0.00s)
PASS
ok      github.com/scytale-network/scytale-p2p/internal/peer    11.026s

Full Network Test Suite:
ok      github.com/scytale-network/scytale-p2p/internal/bridge  1.014s
ok      github.com/scytale-network/scytale-p2p/internal/gossip  1.013s
ok      github.com/scytale-network/scytale-p2p/internal/peer    11.026s
ok      github.com/scytale-network/scytale-p2p/internal/seeder  4.150s
ok      github.com/scytale-network/scytale-p2p/internal/sync    1.011s
ok      github.com/scytale-network/scytale-p2p/internal/wire    1.012s

Compilation:
go build -v ./cmd/scytale-p2p    -> SUCCESS (0 error, binary clean)
go build -v ./cmd/scytale-seeder -> SUCCESS (0 error, binary clean)
```
