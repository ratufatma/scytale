# Scytale Protocol — Technical Specification: Task 30
## Human-Readable Bech32 Address Encoding (`scy1...`)

```text
Task ID       : 30
Task Name     : Human-Readable Bech32 Address Encoding (scy1...)
Phase         : Phase 3 — User-Facing Protocol & Client Tooling
Target Crates : crates/scytale-core, apps/scytale-cli, apps/scytale-node
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Error-Detecting BCH Checksum, Zero Float, Backward-Compatible Hex Parser
```

---

## 1. Arsitektur & Prinsip Encoding Bech32

Sebelum Task 30, alamat dompet Scytale berupa string representasi heksadesimal 64 karakter (hash BLAKE3 32-byte dari *public key*). Format *raw hex* ini rentan terhadap kesalahan ketik, sulit dibaca mata manusia, dan tidak dapat dibedakan secara visual dari *transaction ID* (`txid`) atau *block hash*.

Task 30 mengadopsi standar **Bech32** (BIP-173) dengan awalan (*Human-Readable Part* / HRP) **`scy`** untuk memformat seluruh alamat Scytale.

```text
 ┌───────────────────────────────────────────────────────────────────────────┐
 │                      STRUKTUR ALAMAT BECH32 SCYTALE                       │
 │                                                                           │
 │   scy  1  qpzry9x8gf2tvdw0s3jn54khce6mua7l... [32-byte Hash]  [Checksum] │
 │  └──┬─┘│ └──────────────────────┬───────────────────────────┘ └───┬────┘  │
 │    HRP │               Data Payload (5-bit groups)             6 Karakter │
 │        │                                                      BCH Code    │
 │    Separator ('1')                                                        │
 └───────────────────────────────────────────────────────────────────────────┘
```

### Invarian Kritis & Karakteristik:

1. **Human-Readable Part (HRP):**
   * Produksi / Standar: `scy` (menghasilkan string diawali `scy1...`).
   * Devnet / Testing lokal: Tetap kompatibel dengan `scy` (atau parsing fleksibel).

2. **Kapasitas Deteksi Kesalahan (BCH Checksum):**
   * Polinom BCH menjamin pendeteksian hingga 4 kesalahan karakter acak (*burst/substitution errors*) dan kesalahan transposisi karakter bersebelahan.

3. **Konversi Radix (8-bit ke 5-bit):**
   * 32 byte hash BLAKE3 (256 bit) dipecah menjadi array 5-bit (52 elemen 5-bit ditambah padding 4-bit) sebelum dipetakan ke set karakter Bech32 (`qpzry9x8gf2tvdw0s3jn54khce6mua7l`).

4. **Backward-Compatible Multi-Format Parser:**
   * Setiap fungsi input alamat (misal: parameter `--to` pada CLI transfer) menerima format `scy1...` maupun *raw hex* 64-karakter agar skrip automasi pengujian yang sudah ada tidak mengalami regresi.

---

## 2. Struktur Data Rust (`crates/scytale-core/src/address.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    pub hrp: String,
    pub hash: [u8; 32],
}

impl Address {
    pub const DEFAULT_HRP: &'static str = "scy";

    pub fn from_pubkey_hash(hash: [u8; 32]) -> Self {
        Self {
            hrp: Self::DEFAULT_HRP.to_string(),
            hash,
        }
    }

    pub fn to_bech32(&self) -> Result<String, AddressError>;
    pub fn parse(input: &str) -> Result<Self, AddressError>;
}
```
