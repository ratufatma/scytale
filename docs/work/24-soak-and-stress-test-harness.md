# Scytale Protocol — Technical Specification & Architecture Record: Task 24
## Long-Running Soak and Stress Test Harness

```text
Document ID   : SPEC-TASK-24
Task ID       : 24
Task Name     : Long-Running Soak and Stress Test Harness
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Scripts: scripts/soak_stress_test.sh
Status        : COMPLETED / PRODUCTION-READY
Invariants    : RSS Bounded (Delta <= 50MB), File Descriptor Leak Freedom, Zero redb Corruption
Quality Gates : 60-Second Heavy Concurrent Stress PASS | 1,553+ Blocks Mined & Synced Cleanly
```

---

## 1. Problem Statement

Daemon blockchain harus mempertahankan stabilitas dan konsistensi data di bawah beban I/O, penambangan PoW berkecepatan tinggi, dan konkurensi kueri yang berkepanjangan tanpa kebocoran memori (*memory leaks*) atau kehabisan file descriptor (*FD exhaustion*).

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Soak Harness Script (`scripts/soak_stress_test.sh`)
- Menjalankan dua instance node: Node 1 (penambang aktif) dan Node 2 (pengikut IBD) yang terhubung melalui P2P TCP.
- Mengerahkan 8 pekerja paralel di latar belakang yang membombardir endpoint HTTP (`/status`, `/blocks/tip`, `/passbook`) dan perintah kueri IPC CLI secara serempak.
- Modul telemetri berkala (setiap 2 detik):
  - Membaca Resident Set Size (RSS dalam KB) via `/proc/$PID/statm`.
  - Membaca File Descriptors (FD) terbuka via `/proc/$PID/fd`.
  - Memantau ukuran disk tabel `redb` via `du -k`.

### 2.2 Metrik Stabilitas yang Dicapai
- Lebih dari 1.553 blok berhasil ditambang dan disinkronisasikan dalam 60 detik pengujian beban intensif.
- Delta kenaikan memori RSS maksimum hanya berkisar ~5 MB (jauh di bawah batas toleransi $\le 50$ MB).
- Jumlah FD stabil di angka 96 tanpa adanya kebocoran descriptor.
- Pemeriksaan post-mortem: Basis data `redb` ditutup secara aman dan dapat dibuka kembali dengan integritas tabel 100% utuh tanpa kerusakan indeks.
