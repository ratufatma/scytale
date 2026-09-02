# Scytale Architecture

Scytale is a modular, lightweight, pure-Rust, headless blockchain engine designed around a UTXO ledger model, Proof-of-Work (PoW) consensus, and embedded ACID-compliant storage via `redb`.

---

## 1. Modular Crate Architecture

The engine is decoupled into focused, single-responsibility crates organized with a clear, unidirectional dependency flow:

```text
scytale-core
    ↓
scytale-storage
    ↓
scytale-consensus
    ↓
scytale-mempool
    ↓
scytale-network
    ↓
scytale-node
```

### Dependency Hierarchy & Crate Responsibilities

```
+-----------------------------------------------------------------------+
|                             scytale-node                              |
|   - CLI & Daemon Runtime (clap)                                       |
|   - Node Orchestration & RPC/Interface Stub                          |
+-------------------+-----------------------------------+---------------+
                    |                                   |
                    v                                   v
+-----------------------------------+   +-------------------------------+
|          scytale-network          |   |        scytale-mempool        |
|   - Peer Connection Management    |   |   - Pending Transaction Pool  |
|   - P2P Wire Protocol (Gossip)    |   |   - Eviction & Prioritization |
|   - Block & Tx Synchronization    |   |   - Admission Verification    |
+-------------------+---------------+   +---------------+---------------+
                    |                                   |
                    +-----------------+-----------------+
                                      |
                                      v
                        +-------------------------------+
                        |       scytale-consensus       |
                        |   - Proof-of-Work Validation  |
                        |   - Block & Tx Rule Checks    |
                        |   - Emission & Subsidy Curve  |
                        |   - Coinbase Value Validation |
                        +---------------+---------------+
                                        |
                                        v
                        +-------------------------------+
                        |        scytale-storage        |
                        |   - Redb Embedded Database    |
                        |   - UTXO Set Persistence      |
                        |   - Block & Index Stores      |
                        |   - Atomic Batch Operations   |
                        +---------------+---------------+
                                        |
                                        v
                        +-------------------------------+
                        |         scytale-core          |
                        |   - Ledger Primitives         |
                        |   - Tx, TxIn, TxOut, OutPoint |
                        |   - Blake3 Cryptographic Hash |
                        |   - Deterministic Types       |
                        +-------------------------------+
```

---

## 2. Crate Responsibilities

### `scytale-core`
- **Role:** Base domain primitives and cryptography.
- **Scope:** 
  - Defines the core ledger types: `OutPoint`, `TxIn`, `TxOut`, `Tx`, `BlockHeader`, and `Block`.
  - Provides deterministic hashing routines (powered by **Blake3**).
  - Common error types, numeric encodings, and deterministic serialization logic.
  - Zero knowledge of storage mechanisms, network protocols, or node lifecycles.

### `scytale-storage`
- **Role:** State persistence and storage engine abstraction.
- **Scope:**
  - Manages the embedded **`redb`** key-value database.
  - Persists the active UTXO state table (`OutPoint -> TxOut`).
  - Stores immutable block records, transaction indices, and chain metadata.
  - Enforces ACID transactions for state transitions during block connection and disconnection.

### `scytale-consensus`
- **Role:** Protocol validation and consensus rules.
- **Scope:**
  - Validates Proof-of-Work (target difficulty, hash comparison).
  - Calculates protocol emission curves and block subsidies based on chain height.
  - Verifies block structural integrity and individual transaction validity against UTXO view.
  - Validates the coinbase transaction to ensure minted coins do not exceed `Block Reward + Total Transaction Fees`.
  - Fully deterministic and stateless with respect to node policies.

### `scytale-mempool`
- **Role:** In-memory unconfirmed transaction management.
- **Scope:**
  - Stores valid candidate transactions waiting to be mined into a block.
  - Enforces admission rules (e.g., minimum relay fee, maximum pool capacity, non-conflicting inputs).
  - Organizes transactions for efficient retrieval by block construction modules based on fee prioritization.
  - Evicts invalidated transactions upon receipt of newly confirmed blocks.

### `scytale-network`
- **Role:** Peer-to-peer networking layer.
- **Scope:**
  - Manages peer node discovery, handshakes, heartbeat (`Ping`/`Pong`), and connection lifecycles.
  - Relays unconfirmed transactions and announced block headers across the network.
  - Provides synchronization routines for initial block download (IBD).

### `scytale-node`
- **Role:** Node daemon and CLI application entrypoint.
- **Scope:**
  - Orchestrates storage, mempool, consensus, and network into a cohesive runtime.
  - Exposes a command-line interface (powered by `clap`) for node control, status reporting, and configuration.
  - Drives block assembly, mining threads (when enabled), and peer event loops.

---

## 3. Separation of Consensus Rules from Miner Policy

A fundamental design principle of Scytale is the strict boundary between **Consensus Rules** and **Miner Policy**:

| Dimension | Consensus Rules (`scytale-consensus`) | Miner Policy (`scytale-mempool` / `scytale-node`) |
| :--- | :--- | :--- |
| **Authority** | Network-wide, immutable, non-negotiable. | Local to individual node or miner operator. |
| **Enforcement** | Any block violating these rules is immediately rejected and dropped by all peers. | Affects only which valid transactions the miner chooses to include. |
| **Examples** | - Input UTXOs must exist and be unspent.<br>- $\sum \text{Output Values} \le \sum \text{Input Values}$.<br>- Coinbase output value $\le \text{Subsidy} + \text{Fees}$.<br>- Block PoW meets target difficulty.<br>- Block size limits. | - Fee rate prioritization ordering.<br>- Minimum fee-per-byte threshold for mempool inclusion.<br>- Transaction selection algorithm for block templates.<br>- Custom transaction filtering or inclusion preferences. |

A miner is free to select any combination of valid transactions or even mine empty blocks; the consensus layer only verifies that whatever block is submitted adheres strictly to protocol validity rules.
