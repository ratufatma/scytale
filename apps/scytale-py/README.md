# scytale-py — Python Wrapper untuk Scytale CLI

Antarmuka ramah pengguna di atas biner Rust `scytale-cli` (PIN & nomor rekening).

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