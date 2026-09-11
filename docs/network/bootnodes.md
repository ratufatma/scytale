# Network Bootnodes & Peer Discovery

Dokumen ini memuat daftar bootnode resmi Scytale serta mekanisme penemuan rekan (*peer discovery*) pada jaringan publik.

---

## 1. Bootnode Kanonikal (VPS Production)

Semua node Scytale yang dijalankan tanpa argumen `--bootnodes` akan secara otomatis menghubungi daftar bootnode kanonikal berikut:

### A. Alamat DNS Multiaddr (Rekomendasi)
```text
/dns4/seed.myratu.com/tcp/9000/p2p/12D3KooWMNVcoP79QMoLfg8NKeFyCRfLBq6HngTQpmnkbD9oBWMc
```

### B. Alamat Direct IP Multiaddr (Fallback)
```text
/ip4/116.212.72.89/tcp/9000/p2p/12D3KooWMNVcoP79QMoLfg8NKeFyCRfLBq6HngTQpmnkbD9oBWMc
```

- **Canonical Peer ID**: `12D3KooWMNVcoP79QMoLfg8NKeFyCRfLBq6HngTQpmnkbD9oBWMc`
- **Default Wire Port**: TCP `9000`

---

## 2. Mekanisme Penemuan Peer (Peer Discovery)

1. **Bootstrap Dial**: Node baru menginisialisasi dial ke alamat bootnode kanonikal di atas via TCP + Noise + Yamux.
2. **Kademlia Routing Table**: Begitu terhubung ke bootnode, node memicu kueri bootstrap Kademlia DHT (`/scytale/kad/1.0.0`) untuk menemukan daftar peer lain yang aktif.
3. **Gossipsub Mesh**: Peer yang saling terhubung bergabung ke topik Gossipsub `/scytale/blocks/1.0.0` dan `/scytale/transactions/1.0.0` untuk propagasi real-time latensi rendah.

---

## 3. Deprekasi DNS Seeder Port 53

> [!NOTE]
> Layanan crawler DNS Seeder lama berbasis skrip Python yang mendengarkan di port 53 UDP/TCP telah **dipensiunkan secara permanen**. Node Scytale modern menggunakan resolusi DNS multiaddr native (`libp2p-dns`) dan Kademlia DHT tanpa ketergantungan pada port DNS legacy.
