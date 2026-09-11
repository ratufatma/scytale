# scytale-py — Python Wrapper untuk Scytale CLI

Antarmuka ramah pengguna di atas biner Rust `scytale-cli` (PIN & nomor rekening).
Wrapper ini tidak memiliki ledger lokal dan tidak menyediakan saldo, transaction ID,
atau endpoint jaringan palsu. Semua state transaksi berasal dari `scytale-node` melalui
Rust CLI.

## Alur Produksi

1. Jalankan `scytale-node` dan pastikan gateway HTTP tersedia.
2. Pastikan biner `target/release/scytale-cli` sudah dibangun.
3. Konfigurasikan endpoint node secara eksplisit:

```bash
./tools/scy config --network local --node-url http://127.0.0.1:8332
# Untuk node publik, gunakan URL HTTPS milik deployment Anda:
./tools/scy config --network global --node-url https://node.example.com
```

`scytale-py` tidak memiliki default endpoint publik. `SCYTALE_NODE_URL` dapat dipakai
untuk override per proses. Request yang tidak dapat mengambil state dari node gagal,
bukan diganti data lokal.

## Lingkungan & Versi

- **Python:** 3.14.7 (dikelola `pyenv`), dikunci per-project via `.python-version`.
- **Virtual environment:** `.venv/` (terisolasi penuh, DIABAIKAN git).
- Pastikan `pyenv` tersedia dan shims aktif di shell.

## Aktivasi Virtual Environment

```bash
cd apps/scytale-py
source .venv/bin/activate
python --version   # Python 3.14.7
```

Alternatif tanpa aktivasi (panggil interpreter langsung):

```bash
./.venv/bin/python scy.py --help
```

## Menjalankan CLI

```bash
# Dari dalam apps/scytale-py
../../target/release/scytale-cli --help

# Lewat wrapper
./.venv/bin/python scy.py --help

# Dari root repository
./tools/scy saldo
./tools/scy buat
```

Nominal transfer menggunakan maksimal 8 angka desimal SCY dan dikonversi dengan
aritmetika desimal eksak ke quanta. PIN diminta secara tersembunyi oleh Rust CLI dan
tidak pernah diteruskan sebagai argumen proses atau disimpan oleh wrapper. Config
disimpan atomik dengan permission `0600`.

## Struktur

```
apps/scytale-py/
├── .python-version      # 3.14.7 (dikunci pyenv)
├── .venv/               # virtual environment (tidak dilacak git)
├── scy.py               # wrapper CLI PIN, saldo, dan transfer
└── README.md
```

Launcher `tools/scy` memakai interpreter `.venv/bin/python` secara langsung.
Tidak ada dependensi pip; wrapper hanya menggunakan pustaka standar Python.