# Scytale P2P Network Module

Modul ini bertugas menangani peer discovery, handshake, connection lifecycle, dan gossip protocol. Modul ini dilarang mengelola consensus state, menghitung difficulty, atau mengubah state UTXO secara langsung.

## Architectural Boundaries

- **Transport & Networking (Go):** Peer management, TCP connection pooling, wire protocol serialization, message broadcast, and Initial Block Download (IBD) stream orchestration.
- **Ledger & Consensus Isolation (Rust):** The Go P2P daemon communicates strictly with the Rust core runtime via an explicit IPC boundary (`scytale-bridge`).
- **Zero Consensus Mutation:** Network daemons cannot validate PoW thresholds, create block templates, or modify canonical database tables directly.
