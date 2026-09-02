# Scytale Security & Threat Model Specification

This document defines the formal **Security Architecture and Threat Model** for Scytale. It establishes the trust boundaries, threat categorizations, risk severity ratings, conceptual mitigations, and defensive design invariants across all architectural layers of the blockchain engine.

---

## 1. Core Security Philosophy & Trust Boundaries

Scytale enforces a strict zero-trust posture across all operational boundaries:

> **Foundational Security Invariant:** *All external network inputs, peer-provided metadata, and user-facing presentation states are untrusted. Canonical state mutations are permitted only through complete, fail-closed consensus validation.*

```text
                  UNTRUSTED EXTERNAL DOMAIN (P2P / RPC)
                                    │
                                    ▼
               [ Trust Boundary 1: P2P Network Ingress ]
               ├── Malformed wire frame rejection
               ├── Connection & message rate limiting
               └── Transport protocol framing validation
                                    │
                                    ▼
             [ Trust Boundary 2: Consensus Validation Engine ]
             ├── Complete cryptographic signature verification
             ├── Unspent UTXO existence & solvency verification
             ├── Strict value conservation (In >= Out)
             ├── Proof-of-Work threshold validation (Hash <= Target)
             └── Maximum supply cap enforcement (<= 42M SCY)
                                    │
                                    ▼
                [ TRUSTED CANONICAL DOMAIN (redb Storage) ]
               ├── Canonical UTXO Set
               └── Canonical Chain State & Blocks
                                    │
                                    ▼
             [ Trust Boundary 3: Passbook Presentation Viewer ]
             ├── Read-only dynamic balance derivation
             └── Confirmation status rendering (No state mutation)
```

---

## 2. Core Security Principles

1. **Untrusted External Data:** All serialized bytes received over the network or via APIs are assumed to be malicious until decoded, structured, and validated.
2. **Validation Precedes Mutation:** State is never altered speculatively before validation is 100% complete.
3. **Single Authoritative Source:** The committed `redb` database is the sole authority on ledger truth; no subsystem may maintain a conflicting canonical state.
4. **Fail-Closed Execution:** Any anomaly, unexpected error, or verification failure results in immediate rejection and transaction rollback.
5. **Deterministic Integer Accounting:** All monetary math operates strictly in unsigned integer `quanta` (`u64`) to eliminate floating-point non-determinism and precision leakage.

---

## 3. Comprehensive Threat Analysis & Mitigations

```text
Threat Taxonomy & Defense Matrix
├── 1. Value & Monetary Threats
│   ├── Double-Spending
│   ├── Unauthorized Spending
│   └── Arbitrary Minting & Oversupply
│
├── 2. Consensus & Chain Manipulation
│   ├── Invalid Proof-of-Work Injection
│   ├── Difficulty & Timestamp Manipulation
│   └── Deep Chain Reorganization Attacks
│
├── 3. Network & Denial-of-Service Threats
│   ├── Transaction Flooding & Mempool Exhaustion
│   ├── Block Flooding & Bandwidth Starvation
│   ├── Eclipse & Peer Isolation Attacks
│   └── Sybil Connection Depletion
│
└── 4. Storage & Presentation Integrity
    ├── Partial Writes & Database Corruption
    ├── Value Provenance Fabrication
    └── Passbook Balance Misrepresentation
```

---

### 3.1 Monetary & Value Threats

#### A. Double-Spending Attacks
- **Threat:** An attacker attempts to spend the same UTXO in two conflicting transactions across different blocks or within the mempool.
- **Severity:** **CRITICAL**
- **Mitigation:**
  - The `UTXO_SET` enforces primary-key uniqueness on `OutPoint (TxID, OutputIndex)`.
  - Atomic block commit deletes consumed `OutPoints` synchronously; any competing transaction referencing an already-spent output fails validation immediately.

#### B. Unauthorized Spending Attacks
- **Threat:** An attacker attempts to spend a UTXO without possessing the private keys or cryptographic proofs required by the output's `locking_condition`.
- **Severity:** **CRITICAL**
- **Mitigation:**
  - Mandatory cryptographic authorization validation as specified in [`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md).
  - Unlocking proofs are verified statelessly against the serialized transaction hash prior to admission.

#### C. Arbitrary Minting & Supply Cap Violations
- **Threat:** An attacker attempts to craft transactions with $\sum \text{Outputs} > \sum \text{Inputs}$ or mine coinbase outputs exceeding the block subsidy plus fees.
- **Severity:** **CRITICAL**
- **Mitigation:**
  - Consensus Rule 8 enforces $\text{Coinbase} \le R(H) + \sum \text{Fees}$.
  - Consensus Rule 6 enforces strict integer value conservation on non-coinbase transactions.
  - Global supply reconciles strictly against the immutable $42,000,000\text{ SCY}$ ceiling.

---

### 3.2 Consensus & Chain Manipulation

#### D. Invalid Proof-of-Work Injection
- **Threat:** An attacker broadcasts blocks with forged headers or insufficient computational work to stall validating nodes.
- **Severity:** **HIGH**
- **Mitigation:**
  - Every node independently re-hashes the candidate header using **BLAKE3** and asserts $\text{Numeric}(\text{Hash}) \le \text{difficulty\_target}$.
  - Blocks with invalid PoW are discarded immediately without evaluating transaction payloads.

#### E. Difficulty & Timestamp Manipulation
- **Threat:** Miners manipulate block timestamps to artificially depress network difficulty or force extreme retarget swings.
- **Severity:** **HIGH**
- **Mitigation:**
  - Bounded retarget clamping factors (limiting max adjustment per epoch).
  - Monotonic timestamp validation rules requiring block timestamps to exceed the median of preceding blocks (Median-Time-Past).

#### F. Deep Chain Reorganization Attacks
- **Threat:** An attacker secretly mines a private branch to displace confirmed transactions and execute high-value double-spends.
- **Severity:** **CRITICAL**
- **Mitigation:**
  - Canonical chain selection is anchored exclusively in **cumulative Proof-of-Work** (heaviest chain rule).
  - Probabilistic settlement finality: high-value transactions require multiple confirmation depths before being treated as irreversible.

---

### 3.3 Network & Denial-of-Service (DoS) Threats

#### G. Transaction Spam & Mempool Exhaustion
- **Threat:** An attacker floods the network with millions of zero-fee or dust transactions to consume node memory (RAM) and disk space.
- **Severity:** **MEDIUM**
- **Mitigation:**
  - Local mempool size limits with fee-density eviction policies (shedding lowest-fee transactions under pressure).
  - Minimum relay fee rate thresholds enforced prior to network propagation.

#### H. Eclipse & Peer Isolation Attacks
- **Threat:** An attacker monopolizes all inbound and outbound P2P connections of a victim node to feed it an isolated, false branch.
- **Severity:** **HIGH**
- **Mitigation:**
  - Outbound peer diversity across independent IP subnets and autonomous systems.
  - `Peer Anti-Eclipse Policy: TBD`.

#### I. Sybil Connection Depletion
- **Threat:** An attacker spins up thousands of fake peer identities to exhaust node socket descriptors and connection pools.
- **Severity:** **MEDIUM**
- **Mitigation:**
  - Strict limits on concurrent inbound/outbound connections and per-IP connection limits.
  - Misbehavior penalty scoring leading to temporary or permanent IP bans for abusive behavior.

---

### 3.4 Storage & Presentation Integrity

#### J. Database Corruption & Partial Writes
- **Threat:** A sudden power loss or process kill during block execution leaves the UTXO set and chain state in an inconsistent, corrupt state.
- **Severity:** **CRITICAL**
- **Mitigation:**
  - Complete ACID transaction atomicity provided by `redb`.
  - All-or-nothing commits: state mutations rollback cleanly to the previous confirmed tip if interrupted mid-block.

#### K. Value Provenance & Presentation Manipulation
- **Threat:** A malicious user or tampered client attempts to spoof a positive balance in the Passbook without verified UTXO backing on the ledger.
- **Severity:** **MEDIUM**
- **Mitigation:**
  - Passbook balance derivation is strictly **read-only** and computed dynamically by aggregating confirmed, unspent outputs from the validated node ledger.
  - Passbook display carries zero consensus authority and is never trusted as a source of truth by other nodes.

---

## 4. Risk Severity Classification Matrix

| Severity Tier | Impact Description | Core Failure Scenarios |
| :--- | :--- | :--- |
| **`CRITICAL`** | Direct loss of funds, supply cap violation, unrecoverable state corruption, or permanent consensus splits. | - Double-spend acceptance.<br>- Unauthorized signature bypass.<br>- Supply exceeding 42M SCY.<br>- redb database corruption. |
| **`HIGH`** | Temporary network disruption, miner exploitation, or peer isolation. | - Invalid PoW relay propagation.<br>- Eclipse attacks.<br>- Deep reorg double-spending.<br>- Difficulty retarget gaming. |
| **`MEDIUM`** | Local resource exhaustion or client-side UI degradation. | - Mempool RAM flooding.<br>- Peer socket depletion.<br>- Passbook cache desynchronization. |
| **`LOW`** | Minor telemetry inaccuracies or transient connection retries. | - Duplicate wire announcements.<br>- Latency in peer discovery. |

---

## 5. Open Security Questions & Parameters

The following security specifications remain designated as **TBD**:

| Security Parameter | Status | Scope |
| :--- | :--- | :--- |
| **`Cryptographic Signature Suite`** | `TBD` | Concrete choice of digital signature algorithm (Ed25519 vs. Secp256k1). |
| **`Peer Anti-Eclipse Strategy`** | `TBD` | Algorithmic bucket rotation for peer IP addresses. |
| **`Mempool DoS Rate Limiting`** | `TBD` | Exact token bucket parameters for P2P transaction relay. |
| **`Storage Corruption Repair Tooling`**| `TBD` | CLI verification and offline database rebuild utilities. |
| **`Recommended Settlement Finality Depth`**| `TBD` | Statistical confirmation guidelines for commercial exchange deposits. |

---

## 6. Cross-Specification References

- **[`docs/CONSENSUS-SPEC.md`](CONSENSUS-SPEC.md)**: Universal consensus invariants.
- **[`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md)**: Cryptographic signature and locking condition rules.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle and double-spend prevention.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: 13 consensus validation checks.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold verification.
- **[`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md)**: Cumulative work selection and reorg handling.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: redb ACID atomicity and crash recovery.
- **[`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md)**: Mempool admission and spam mitigation.
- **[`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md)**: Peer misbehavior scoring and flood control.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: Decoupled presentation layer security.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Lineage verification and auditability.
