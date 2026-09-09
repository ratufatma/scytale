# Scytale Protocol — Technical Specification & Architecture Record: Task 29
## Dynamic P2P Peer Discovery (`getaddr` / `addr` Wire Protocol & Go Daemon AddrBook)

```text
Document ID   : SPEC-TASK-29
Module        : network/cmd/scytale-p2p, network/internal/wire, network/internal/peer
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Autonomous Discovery, Zero-Race Concurrency (-race), Anti-Poisoning IP Filtering, Atomic Disk Persistence
Quality Gates : 100% Go Race Detector PASS | 122 Rust Workspace Tests PASS | Live 2-Node & Fork Reorg PASS
```

---

## 1. Latar Belakang & Masalah (Problem Statement)

Pada implementasi awal sebelum Task 29, topologi jaringan Scytale (`network/cmd/scytale-p2p`) bersifat sepenuhnya statis:
1. **Ketergantungan Konfigurasi Manual:** Setiap node hanya dapat terhubung ke peer yang didefinisikan secara eksplisit lewat argumen baris perintah `--peer <addr>`.
2. **Kerapuhan Topologi & Resiko Partisi Jaringan:** Jika node bootstrap atau peer perantara mengalami kegagalan (*crash* atau *network partition*), node baru tidak memiliki cara untuk mengetahui alamat node lain di jaringan secara mandiri (*autonomous neighbor discovery*).
3. **Ketiadaan Repositori Alamat Persisten:** Alamat peer yang berhasil terhubung tidak disimpan ke disk. Saat node dimatikan (*restart*), seluruh riwayat jaringan hilang dan node harus mengulang dial statis dari awal.
4. **Resiko Peracunan Rute (Route Poisoning / Sybil Injection):** Tanpa mekanisme validasi alamat IP publik, aktor jahat dapat mengirimkan alamat *loopback* (`127.0.0.1`) atau subnet lokal privat (RFC 1918) untuk membelokkan koneksi node atau melumpuhkan kemampuan *routing*.

Task 29 mengatasi masalah-masalah ini dengan mengadopsi mekanisme pertukaran alamat standar industri (*Bitcoin-style address gossip*) berbasis pesan `getaddr` dan `addr`, buku alamat lokal (`AddrBook`) dengan persistensi atomik ke `peers.json`, serta loop *auto-dialer* otonom.

---

## 2. Invarian Arsitektur & Prinsip Desain

### Invarian 1: Spesifikasi Pesan Wire Kanonikal (`getaddr` & `addr`)
* **`CmdGetAddr` (`"getaddr"`):** Pesan query dengan muatan kosong (0 byte) yang dikirimkan oleh node segera setelah proses *handshake* (`version` / `verack`) selesai untuk meminta daftar tetangga remote peer.
* **`CmdAddr` (`"addr"`):** Pesan balasan pembawa daftar alamat jaringan (`NetAddress`).
* **Layout Biner 34 Byte per Entri Alamat:**
  ```text
  [ 8-byte Timestamp (LE) | 8-byte Services (LE) | 16-byte IP (IPv4-mapped/IPv6) | 2-byte Port (BE) ]
  ```
* **Batas Keras Muatan:**
  * `NetAddressWireSize = 34` byte.
  * `MaxAddrsPerMsg = 1000` entri per frame (maksimal muatan biner: $4 + 1.000 \times 34 = 34.004$ byte).
  * Frame dilindungi oleh 4-byte checksum BLAKE3/SHA-256 pada header wire Scytale.

### Invarian 2: Anti-Poisoning & Filtrasi Routabilitas IP
Untuk mencegah serangan *route poisoning* dan isolasi node (*eclipse attacks*), fungsi `IsRoutable(addrStr, allowLocal)` menerapkan filter ketat pada setiap alamat yang diterima:
* Menolak alamat yang tidak memiliki pasangan *host:port* valid (port 1..65535).
* Menolak alamat tidak spesifik (`0.0.0.0`, `::`) dan alamat multicast.
* Menolak alamat *loopback* (`127.0.0.0/8`, `::1`) dan rentang IP privat RFC 1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`, `fe80::/10`), **kecuali** jika flag `--allow-local-peers` diaktifkan secara eksplisit (misalnya pada *local testnet* atau kluster *Docker* internal).

### Invarian 3: Seleksi Alamat dengan Exponential Backoff
Agar node tidak membanjiri (*spam dial*) peer yang sedang offline:
* Alamat yang belum pernah dicoba (`Attempts == 0`) mendapatkan prioritas tertinggi.
* Alamat yang mengalami kegagalan koneksi diatur oleh jeda waktu eksponensial (*exponential backoff*):
  $$\text{Backoff} = \min\left(2\text{ jam}, \; 5\text{s} \times 2^{\text{attempts}}\right)$$
* Koneksi yang berhasil (`MarkSuccess`) mereset counter `Attempts` menjadi 0 dan memperbarui `LastSeen` serta `LastSuccess`.

### Invarian 4: Persistensi Atomik Bebas Korupsi (`peers.json`)
Penyimpanan state `AddrBook` ke disk dilakukan secara atomik:
1. Data alamat dan riwayat telemetri di-marshal ke JSON berformat rapi (*indented*).
2. Data ditulis ke berkas temporer: `<path>.tmp` dengan izin akses aman POSIX `0600`.
3. Mengganti berkas target utama secara atomik via `os.Rename(tmpFile, path)`. Hal ini menjamin berkas `peers.json` tidak akan pernah rusak (*corrupted*) meskipun node dihentikan secara tiba-tiba (*crash/kill*).

### Invarian 5: Konkurensi Murni Tanpa Data Race
Seluruh struktur data `AddrBook`, map `peerPool`, dan penanda `isOutbound` dilindungi oleh `sync.RWMutex` dan `sync.Mutex`. Tidak ada goroutine yang mengakses pointer bersama secara mutable tanpa lock. Seluruh paket diverifikasi 100% lulus pada pengujian Go Race Detector (`go test -race`).

---

## 3. Rancang Bangun Komponen & Kode Golang

### A. Protokol Wire (`network/internal/wire/`)

#### 1. Perluasan Perintah Wire (`wire.go`)
```go
const (
    CmdVersion   = "version"
    CmdVerack    = "verack"
    CmdInv       = "inv"
    CmdGetData   = "getdata"
    CmdTx        = "tx"
    CmdBlock     = "block"
    CmdGetBlocks = "getblocks"
    CmdInvBlocks = "invblocks"
    CmdGetAddr   = "getaddr"
    CmdAddr      = "addr"
    CmdPing      = "ping"
    CmdPong      = "pong"
)
```

#### 2. Serialisasi & Deserialisasi Alamat (`msg_addr.go`)
```go
type NetAddress struct {
    Timestamp int64  // Unix seconds
    Services  uint64 // Feature bitfield
    IP        net.IP // 16-byte IPv4-mapped / IPv6
    Port      uint16 // BigEndian network byte order
}

func EncodeAddr(addrs []NetAddress) []byte {
    count := len(addrs)
    if count > MaxAddrsPerMsg {
        count = MaxAddrsPerMsg
    }
    buf := make([]byte, 4+count*NetAddressWireSize)
    binary.LittleEndian.PutUint32(buf[0:4], uint32(count))
    offset := 4
    for i := 0; i < count; i++ {
        binary.LittleEndian.PutUint64(buf[offset:offset+8], uint64(addrs[i].Timestamp))
        binary.LittleEndian.PutUint64(buf[offset+8:offset+16], addrs[i].Services)
        copy(buf[offset+16:offset+32], addrs[i].IP.To16())
        binary.BigEndian.PutUint16(buf[offset+32:offset+34], addrs[i].Port)
        offset += NetAddressWireSize
    }
    return buf
}
```

---

### B. Komponen Address Book (`network/internal/peer/addrbook.go`)

```go
type KnownAddress struct {
    Addr        string    `json:"addr"`
    Src         string    `json:"src"`
    Attempts    int       `json:"attempts"`
    LastAttempt time.Time `json:"last_attempt"`
    LastSuccess time.Time `json:"last_success"`
    LastSeen    time.Time `json:"last_seen"`
}

type AddrBook struct {
    mu              sync.RWMutex
    filePath        string
    allowLocalPeers bool
    addresses       map[string]*KnownAddress
    rng             *rand.Rand
}
```

Metode Utama `AddrBook`:
* `AddAddress(addrStr, src string) bool`: Menyaring via `IsRoutable` dan mendaftarkan alamat baru.
* `GetAddresses(max int) []string`: Mengambil snapshot alamat acak (*Fisher-Yates shuffle*) hingga batas `max` untuk respon pesan `addr`.
* `SelectAddressToDial(connected []string) (string, bool)`: Memilih kandidat dial dengan bobot prioritas tertinggi yang tidak sedang terhubung dan telah melewati masa backoff.
* `Save()` / `Load()`: Persistensi atomik JSON ke `peers.json`.

---

### C. Loop Event P2P & Goroutine Auto-Dialer (`network/cmd/scytale-p2p/main.go`)

```text
┌────────────────────────────────────────────────────────┐
│               Scytale Go P2P Daemon                    │
│                                                        │
│  ┌─────────────────┐       TCP Handshake               │
│  │   AutoDialer    │ ─────────────────────────► Remote │
│  │  (Ticker: 3s)   │                            Peer   │
│  └────────┬────────┘                                   │
│           │ Dial Candidate                             │
│           ▼                                            │
│  ┌─────────────────┐       CmdGetAddr                  │
│  │    AddrBook     │ ─────────────────────────► Remote │
│  │  (peers.json)   │                                   │
│  │                 │ ◄───────────────────────── Remote │
│  └─────────────────┘        CmdAddr                    │
│                             (Decode & Store)           │
└────────────────────────────────────────────────────────┘
```

1. **Pertukaran Pasca-Handshake:**
   ```go
   if isInitiator {
       p.SetOutbound(true)
   }
   d.addrBook.MarkSuccess(p.Address())
   _ = p.Send(wire.CmdGetAddr, nil)
   ```
2. **Penanganan Pesan `getaddr` & `addr`:**
   ```go
   case wire.CmdGetAddr:
       knownAddrs := d.addrBook.GetAddresses(wire.MaxAddrsPerMsg)
       // Encode dan kirim pesan CmdAddr ke remote peer
   case wire.CmdAddr:
       decoded, err := wire.DecodeAddr(msg.Payload)
       // Daftarkan seluruh alamat yang diterima ke AddrBook dan simpan ke disk
   ```
3. **Goroutine `autoDialerLoop`:**
   Secara periodik memeriksa apakah jumlah koneksi keluar (*outbound connections*) berada di bawah `maxOutbound` (default: 8). Jika masih di bawah batas, dial kandidat terbaik dari `AddrBook`.

---

## 4. Opsi Baris Perintah Daemon (`scytale-p2p`)

| Flag | Tipe | Default | Deskripsi |
| :--- | :--- | :--- | :--- |
| `--allow-local-peers` | `bool` | `false` | Izinkan alamat peer loopback (`127.0.0.1`) dan subnet privat RFC 1918. |
| `--peers-file` | `string` | `"peers.json"` | Path lokasi berkas penyimpanan basis data peer lokal. |
| `--max-outbound` | `int` | `8` | Target jumlah koneksi outbound aktif yang dikelola oleh auto-dialer. |
| `--peer` | `string` | `[]` | Alamat peer bootstrap statis (dapat didefinisikan berulang kali). |
| `--p2p-bind` | `string` | `""` | Alamat TCP listener inbound (misal: `127.0.0.1:9001`). |

---

## 5. Hasil Verifikasi & Pengujian Mutu (Quality Gates)

### A. Pengujian Unit & Race Detector Golang (`network/`)
* **`network/internal/wire/msg_addr_test.go`**:
  * `TestAddrEncodeDecodeRoundtrip`: Verifikasi serialisasi/deserialisasi identik untuk alamat IPv4 dan IPv6.
  * `TestAddrEmptyPayload`: Verifikasi encoding/decoding payload kosong 4-byte.
  * `TestAddrShortPayloadError`: Verifikasi deteksi dini pemotongan payload (*truncation guard*).
  * `TestNewNetAddressFromString`: Parsing dan normalisasi alamat `host:port`.
* **`network/internal/peer/addrbook_test.go`**:
  * `TestAddrBookFiltering`: Uji filtrasi loopback dan RFC 1918 pada mode publik vs mode lokal.
  * `TestAddrBookSelectAddress`: Validasi seleksi dial dan penolakan peer yang telah terhubung.
  * `TestAddrBookPersistence`: Uji coba simpan-baca berkas `peers.json`.
  * `TestAddrBookConcurrentAccess`: Pengujian beban konkurensi 20 goroutine di bawah `-race`.
* **`network/internal/peer/discovery_test.go`**:
  * `TestDynamicPeerDiscoveryExchange`: Pengujian integrasi live end-to-end simulasi 2 node di mana Node A mengirim `getaddr` ke Node B dan berhasil mempelajari serta menyimpan alamat Node C dari Node B.
* **Hasil:** `go test -v -race ./...` $\rightarrow$ **100% PASS (0 data race warnings)**.

### B. Integrasi Supervisor Rust & Skrip Testnet
* Supervisor Rust ([`apps/scytale-node/src/p2p_supervisor.rs`](file:///mnt/ssd/scytale-lab/scytale/apps/scytale-node/src/p2p_supervisor.rs)) secara otomatis menyuntikkan flag `--allow-local-peers` saat memutar child process daemon Go.
* `cargo test --workspace --all-targets` $\rightarrow$ **122 Tests PASS**.
* `./scripts/testnet_2node.sh` $\rightarrow$ **PASS**: Sinkronisasi konsensus dan buku tabungan 100% identik.
* `./scripts/testnet_fork_reorg.sh` $\rightarrow$ **PASS**: Reorganisasi rantai percabangan (*chain fork reorg*) berjalan atomik dan stabil.
