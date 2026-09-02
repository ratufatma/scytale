# Scytale P2P Network Specification

This document defines the formal specification for the **Peer-to-Peer (P2P) Networking Layer** in Scytale. It establishes the architectural separation between the **Go-based P2P networking subsystem** and the **Rust-based core protocol/ledger engine**, defining peer discovery, connection lifecycles, transaction/block propagation, initial chain synchronization, and fault isolation.

---

## 1. Purpose & Architectural Mandate

The P2P network layer provides the decentralized communications backbone for Scytale:

- **Peer Discovery & Topology Management:** Locates, connects, and maintains connections across distributed nodes.
- **Data Transport:** Disseminates unconfirmed transactions and newly mined blocks across the network.
- **Initial Chain Synchronization (Sync):** Facilitates high-throughput historical block retrieval for newly joined or restarting nodes.
- **Network Shielding:** Rejects malformed transport frames and rate-limits abusive peers before wasting consensus computation.

> **Foundational Axiom:** *The network layer transports protocol data; the consensus engine independently determines whether that data is valid.*

```text
                  Scytale Architectural Boundary
┌─────────────────────────────────────────────────────────────────┐
│                      Go P2P Network Layer                       │
│  - Peer Discovery, Connection Pooling, Framing, Relay Routing   │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                   (Protocol Message Boundary)
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Rust Protocol Engine                          │
│  - Consensus, UTXO State, Storage (redb), Mempool, Mining       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Technology Partitioning: Go & Rust

Scytale enforces a clean, multi-language architectural division:

| Domain | Language / Runtime | Architectural Rationale |
| :--- | :--- | :--- |
| **P2P Networking Subsystem** | **Go** | Concurrency primitives (goroutines/channels), high-throughput I/O multiplexing, long-lived peer connection management, and mature networking ecosystems. |
| **Core Protocol & Ledger Engine** | **Rust** | Memory safety without garbage collection pauses, bit-level deterministic execution, zero-copy serialization, and redb ACID storage integration. |

- **Language Independence:** The Scytale wire protocol is strictly language-independent. The wire format can be consumed by any compliant client regardless of programming language.
- `P2P Library / Framework: TBD` (Libp2p vs. custom TCP/QUIC daemon).
- `Transport Protocol: TBD` (TCP, QUIC, or multiplexed streams).

---

## 3. Strict Boundary of Authority

The P2P network subsystem is **strictly a transport conduit**:

### The Network Layer CANNOT:
- Declare a transaction or block valid.
- Elect or switch the canonical chain tip.
- Apply mutations to the `UTXO_SET` or `CHAIN_STATE`.
- Create new currency or alter monetary issuance rules.

All semantic validation, Proof-of-Work checks, signature evaluations, and UTXO state transitions occur exclusively within the **Rust consensus engine**.

---

## 4. Peer Identity vs. Asset Ownership

Scytale enforces strict cryptographic isolation between network identity and ledger value ownership:

$$\text{P2P Peer Identity} \ne \text{SCY Asset Ownership Keys}$$

- **Peer Identity:** Used solely to identify transport endpoints, authenticate secure sessions, route messages, and maintain connection reputation scores.
- **Zero Asset Coupling:** A peer's network key carries zero monetary value and cannot authorize transaction spending.
- `Peer Identity Format: TBD` (e.g., Ed25519 / Secp256k1 network public key digests).

---

## 5. Peer Discovery Mechanics

Nodes discover peers through multiple redundant channels:

```text
                        Peer Discovery Sources
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         ▼                         ▼                         ▼
Configured Static Peers     DNS Bootstrap Nodes       Peer Exchange (PEX)
         │                         │                         │
         └─────────────────────────┼─────────────────────────┘
                                   ▼
                       Discovered Address Pool
                                   ↓
                         Outbound Dialer Loop
```

- **Specification Status:** `Peer Discovery Mechanism: TBD`, `Bootstrap Nodes: TBD`, `Address Discovery: TBD`.

---

## 6. Peer Connection Lifecycle

A connection transitions through explicit operational phases:

```text
              [ Disconnected ]
                     │
                     ▼ Dial / Accept
              [ Connecting ]
                     │
                     ▼ TCP / Transport Handshake
              [ Connected ]
                     │
                     ▼ Protocol Handshake (Version, Genesis, Height)
              [ Handshake Pending ]
                     │
                     ▼ Capabilities Verified & Genesis Matched
              [ Active / Ready ] <───> [ Normal Relay & Sync ]
                     │
                     ▼ Error / Timeout / Ban
              [ Closing ]
                     │
                     ▼ Socket Teardown
              [ Disconnected ]
```

---

## 7. Protocol Handshake & Network Isolation

Upon establishing a transport connection, nodes must immediately exchange a canonical Handshake message before routing application traffic:

```text
Handshake Payload:
├── protocol_version      : Active Wire Protocol Version
├── network_identifier    : Network ID (Mainnet / Testnet / Devnet)
├── peer_identity         : Ephemeral or Static Peer ID
├── best_block_id         : BLAKE3 Hash of Local Canonical Tip
├── best_height           : Local Chain Height
├── genesis_hash          : Genesis Block Hash (Must Match Exactly)
└── capability_flags      : Service Flags (Full Node, Archive, Relay)
```

- **Incompatible Networks:** If `network_identifier` or `genesis_hash` does not match, the connection is terminated immediately to prevent cross-network contamination.
- `Handshake Message Format: TBD`.

---

## 8. Message Categories & Wire Types

The protocol defines four distinct message domains:

```text
Scytale Wire Message Suite
├── 1. Peer Control
│   ├── Handshake / HandshakeAck
│   ├── Ping / Pong (Liveness & Latency Probing)
│   └── Disconnect (Reason Codes)
│
├── 2. Initial Chain Synchronization
│   ├── GetChainLocator (Sparse Block Hash Locator)
│   ├── ChainHeadersResponse (Sequential Block Headers)
│   ├── GetBlockData (Full Block Request)
│   └── BlockDataResponse (Raw Canonical Block Payload)
│
├── 3. Transaction Propagation
│   ├── TxAnnouncement (Inv / TxID Announcement)
│   ├── GetTxData (Request Specific TxIDs)
│   └── TxDataResponse (Canonical Serialized Transaction)
│
└── 4. Block Propagation
    ├── BlockAnnouncement (Header / BlockID Announcement)
    ├── GetBlockData (Request Specific BlockID)
    └── BlockDataResponse (Canonical Serialized Block)
```

- `Wire Message Names: TBD`, `Wire Encoding: TBD` (Compact binary framing).

---

## 9. Transaction Propagation (Relay)

Transactions propagate across the network using a two-phase announcement-and-request flow to conserve bandwidth:

```text
Node A (Origin / Relay)                            Node B (Peer)
      │                                                  │
      ├─────── TxAnnouncement (TxID: 0x8a3f...) ────────>│
      │                                                  │
      │                                      Already in Mempool or Spent?
      │                                      ├── YES: Ignore
      │                                      └── NO : Request Payload
      │                                                  │
      │<────── GetTxData (TxID: 0x8a3f...) ──────────────┤
      │                                                  │
      ├─────── TxDataResponse (Raw Tx Payload) ─────────>│
      │                                                  │
      │                                      [ Rust Engine Verification ]
      │                                      ├── INVALID: Discard & Penalize
      │                                      └── VALID  : Admit to Mempool
      │                                                   & Relay to Peers
```

- Cross-References: [`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md) and [`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md).

---

## 10. Block Propagation

Newly discovered blocks are announced immediately to minimize stale mining rates:

```text
Mined Block Discovered
           ↓
Broadcast BlockAnnouncement to Peers
           ↓
Peers Request Full Block Payload (if not already received)
           ↓
Peers Stream Raw Block Payload
           ↓
Rust Consensus Engine Executes Full Validation
           ├── VALID   ──> Commit to redb, Advance Tip, Relay to Neighbors
           └── INVALID ──> Drop Block, Penalize Peer
```

- Cross-References: [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md) and [`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md).

---

## 11. Initial Chain Synchronization (IBD)

A newly initialized or out-of-sync node executes the Initial Block Download workflow:

```text
Fresh Node Launch
       ↓
Connect to Peers & Handshake
       ↓
Evaluate Peer Tips & Cumulative Work
       ↓
Generate Chain Locator (Logarithmic History of Known Blocks)
       ↓
Request Sequential Block Headers from Heaviest Peer
       ↓
Verify Header Proof-of-Work & Difficulty Adjustments
       ↓
Batch Download Full Block Payloads
       ↓
Sequentially Apply & Validate UTXO State Transitions
       ↓
Local Tip Catches Up to Network Tip -> Transition to Active Relay
```

- **Unverified Metadata:** A peer's declared height is treated as **unverified advisory metadata** until its constituent blocks are mathematically validated.

---

## 12. Chain Locator & Ancestor Discovery

To discover where a remote peer's branch diverges without transmitting entire chain histories, the syncing node sends a **Chain Locator**:

$$\text{ChainLocator} = [ H_{\text{tip}}, H_{\text{tip}-1}, H_{\text{tip}-2}, \dots, H_{\text{tip}-8}, H_{\text{tip}-16}, H_{\text{tip}-32}, \dots, \text{Genesis} ]$$

- The remote peer scans the locator from newest to oldest to identify the latest shared common ancestor block.
- `Chain Locator Structure: TBD`.

---

## 13. Resource Limits & Flood Protection

To guarantee resilience against denial-of-service (DoS) attacks:

| Resource Boundary | Specification Status | Protection Target |
| :--- | :--- | :--- |
| **`Maximum Message Frame Size`** | `TBD` | Prevents memory allocation attacks on transport sockets. |
| **`Peer Request Rate Limits`** | `TBD` | Prevents I/O starvation from spamming inventory queries. |
| **`Inbound / Outbound Peer Limits`**| `TBD` | Manages node socket and memory resource budgets. |
| **`Relay Rate Policy`** | `TBD` | Suppresses transaction flooding across P2P links. |

---

## 14. Invalid Data & Peer Misbehavior Scoring

Scytale distinguishes between transport framing errors and ledger validation failures:

```text
Incoming Data
      │
      ├── Malformed Wire Framing / Protocol Violation
      │         ↓
      │    Immediate Disconnect / Connection Reset
      │
      └── Syntactically Valid Frame Carrying Invalid Ledger Object
                ↓
           Sent to Rust Engine for Semantic Verification
                ↓
           Consensus Failure (Invalid PoW, Double Spend, Bad Sig)
                ↓
           Increment Peer Misbehavior Score
                ↓
           Score Exceeds Threshold?
             ├── NO  ──> Log Warning & Suppress Object
             └── YES ──> Ban Peer IP for Configured Duration
```

- `Peer Misbehavior Scoring: TBD`, `Ban / Disconnect Policy: TBD`.

---

## 15. The Rust ↔ Go Inter-Process Communication (IPC) Boundary

The runtime boundary between the Go P2P subsystem and the Rust Protocol Engine is designed with explicit isolation:

```text
┌───────────────────────────┐                 ┌───────────────────────────┐
│     Go P2P Subsystem      │                 │   Rust Protocol Engine    │
│                           │                 │                           │
│ - Peer Message Ingress    │ ── (Inbound) ──>│ - Consensus Validation    │
│ - Wire Frame Deserializer │                 │ - State Transition Engine │
│ - Peer Message Egress     │<── (Outbound) ──│ - Miner / Mempool Events  │
└───────────────────────────┘                 └───────────────────────────┘
```

### Architecture Invariants:
- **Loose Coupling:** The Go daemon and Rust engine communicate over an explicit high-performance message boundary.
- **Specification Status:** `Rust ↔ Go Transport Boundary: TBD`, `IPC / RPC Mechanism: TBD`, `Message ABI: TBD` (Domain socket, IPC channel, or shared memory buffer).

---

## 16. Network Degradation & Offline Node Resilience

If all P2P connections drop (network partition / local offline mode):
- The node remains completely stable, preserving its local canonical `redb` state.
- Local RPC services (Passbook balance queries, historical lookups) continue functioning over active local state.
- Upon reconnection, the node resumes discovery, performs sync reconciliation, and re-attaches to the global mesh.

---

## 17. Open Questions & Pending Specifications

The following implementation domains remain designated as **TBD**:

| Parameter / Policy | Status | Scope |
| :--- | :--- | :--- |
| **P2P Library / Framework** | `TBD` | Choice of Go networking library (Libp2p vs. bespoke socket multiplexer). |
| **Rust ↔ Go IPC Mechanism** | `TBD` | Mechanism for inter-process messaging between Go and Rust. |
| **Wire Protocol Encoding** | `TBD` | Binary serialization format for P2P transport frames. |
| **Peer Identity Format** | `TBD` | Cryptographic public key schema for network peer IDs. |
| **Network Identifier Format** | `TBD` | Magic byte sequence separating Mainnet, Testnet, and Devnet. |
| **Peer Misbehavior Scoring** | `TBD` | Formal point scoring system and ban duration thresholds. |
| **P2P Privacy Model** | `TBD` | Analysis of IP broadcast privacy and transaction relay obfuscation. |

---

## 18. Cross-Specification References

- **[`docs/ARCHITECTURE.md`](ARCHITECTURE.md)**: System crate hierarchy and modular partitioning.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block structures and validation invariants.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction encoding and TxID derivation.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: UTXO state transitions and double-spend rules.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold verification.
- **[`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md)**: Cumulative work evaluation during initial synchronization.
- **[`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md)**: Transaction admission pipeline.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: Canonical state persistence in `redb`.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Network genesis root matching.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Fixed supply limits and economic invariants.
