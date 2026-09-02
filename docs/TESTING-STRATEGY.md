# Scytale Testing Strategy & Quality Assurance Framework

This document defines the formal **Testing Strategy and Verification Framework** for the Scytale blockchain engine. It establishes a multi-tiered testing hierarchy designed to deliver mathematical certainty across consensus invariants, data durability, and network reliability without burdening development with fragile or redundant test suites.

---

## 1. Core Testing Philosophy: Reality over Mocks

> **Foundational Invariant:** *Passing a mocked unit test does not prove the real system works. Verification must test authentic execution paths against real storage, real data structures, and real network boundaries.*

### Guiding Principles:
1. **Risk-Proportional Depth:** Test rigor is concentrated where failure is catastrophic: Consensus Invariants, Monetary Accounting, Value Provenance, and Atomic Storage.
2. **Deterministic Reproducibility:** Every test suite must execute deterministically with zero flaky timing dependencies or random nondeterminism.
3. **No Artificial Mocking for Core Invariants:** Consensus state transitions and storage operations must be validated using live instances of `redb` and real cryptographic primitives.
4. **Separation of Concerns:** Fast unit tests reside near source files; heavy integration, property, and end-to-end reality tests reside in dedicated `tests/` suites.

---

## 2. Multi-Tiered Testing Hierarchy

```text
                                  [ REALITY / E2E TESTS ]
                              Full node process lifecycles,
                                real P2P sync, real storage
                             ───────────────────────────────
                                [ CONSENSUS & PROPERTY TESTS ]
                              Supply cap proofs, double-spend
                               rejections, reorg invariants
                             ───────────────────────────────────
                                [ INTEGRATION & STORAGE TESTS ]
                               Subsystem boundaries, redb atomic
                                commit & crash recovery cycles
                             ───────────────────────────────────────
                                [ DETERMINISTIC UNIT TESTS ]
                               BLAKE3 digests, quanta arithmetic,
                                 canonical byte encoding checks
```

---

## 3. Test Tier Specifications

### 3.1 Tier 1: Deterministic Unit Tests
- **Focus:** Discrete, pure algorithmic functions.
- **Coverage Domains:**
  - Integer `quanta` arithmetic and overflow prevention.
  - BLAKE3 32-byte hash calculation and threshold comparison ($\text{Hash} \le \text{Target}$).
  - Canonical byte serialization and deserialization symmetry ($\text{Object} \rightarrow \text{Bytes} \rightarrow \text{Object}$).
  - Fee calculation and value conservation checks ($\sum \text{In} - \sum \text{Out}$).
- **Target Execution Time:** $< 10\text{ milliseconds}$ per test.

### 3.2 Tier 2: Subsystem Integration Tests
- **Focus:** Interface boundaries between adjacent crates.
- **Coverage Domains:**
  - `Transaction` $\rightarrow$ `UTXO Set`: Validation of cryptographic unlocking proofs against stored `OutPoints`.
  - `Block` $\rightarrow$ `Consensus Engine`: Application of 13 consensus validation rules to block headers and transaction vectors.
  - `Mempool` $\rightarrow$ `Transaction Ingress`: Deduplication, conflict detection, and fee-rate sorting.
  - `Storage` $\rightarrow$ `Chain State`: Atomic insertion and retrieval of blocks, headers, and chain tips in `redb`.

### 3.3 Tier 3: Consensus Invariant & Property Tests
- **Focus:** Mathematical verification of unbreakable protocol rules.
- **Mandatory Invariant Suites:**
  - **Double-Spend Rejection:** Submitting two valid transactions referencing the same input `OutPoint` guarantees that exactly one is accepted and the other rejected.
  - **Monetary Supply Ceiling:** Proving that no sequence of valid blocks can cause total circulating quanta to exceed $42,000,000\text{ SCY}$ ($4.2 \times 10^{15}\text{ quanta}$).
  - **Zero Arbitrary Minting:** Proving that non-coinbase transactions cannot generate unbacked value.
  - **Heaviest Chain Convergence:** Proving that when presented with two competing valid branches, the node deterministically selects the branch with the greatest cumulative Proof-of-Work.

### 3.4 Tier 4: Storage Durability & Crash Recovery Tests
- **Focus:** ACID guarantees and crash resilience of the `redb` storage engine.
- **Coverage Domains:**
  - **Atomic Block Commit:** Simulating I/O interruptions mid-block to prove zero partial state corruption.
  - **Crash & Restart Verification:** Verifying that a node restarted after simulated process kill accurately restores the active tip and spendable UTXO set.
  - **Reorganization Rollback:** Proving that rolling back a branch cleanly restores previously consumed UTXOs and revokes created outputs.

### 3.5 Tier 5: P2P Network & Synchronization Tests
- **Focus:** Multi-node communication, wire framing, and message routing.
- **Coverage Domains:**
  - Protocol handshake negotiation, version compatibility, and network ID isolation.
  - Two-phase transaction and block announcement/relay mechanisms.
  - Initial Block Download (IBD) synchronization over simulated multi-node topologies.
  - Peer misbehavior detection, penalty scoring, and IP ban enforcement.

### 3.6 Tier 6: Autonomous Mining Lifecycle Tests
- **Focus:** Block template generation, coinbase calculation, and stale work cancellation.
- **Coverage Domains:**
  - Construction of valid candidate templates with correct difficulty targets and coinbase sums.
  - Immediate cancellation of in-flight mining workers upon receiving a new valid block from the network.
  - Zero-balance user onboarding: proving that a fresh node with 0 SCY can mine a valid block and unlock a positive balance.
  - *Testing Optimization:* Mining tests utilize an ultra-low testing difficulty target to execute instantly without consuming significant CPU work.

---

## 4. Test Suite Organization & Directory Layout

To maintain codebase hygiene, test files follow strict structural separation:

```text
scytale/
├── crates/
│   ├── scytale-core/src/             # In-crate unit tests (tests module)
│   ├── scytale-storage/src/          # Table definition & codec unit tests
│   ├── scytale-consensus/src/        # Arithmetic & validation unit tests
│   └── scytale-mempool/src/          # Prioritization & conflict unit tests
│
└── tests/                            # Dedicated Integration & Reality Suites
    ├── consensus_invariants/         # Supply cap, double spend, and PoW tests
    ├── storage_durability/           # redb atomic commit & crash recovery tests
    ├── p2p_integration/              # Multi-node sync & wire framing tests
    ├── mining_lifecycle/             # Miner template & stale worker cancel tests
    └── end_to_end/                   # Full node launch, sync, and payment workflows
```

---

## 5. Development Workflow & Quality Assurance Gates

All development contributions must pass the following standardized four-stage quality gate:

```text
                    Developer Modifies Codebase
                                 │
                                 ▼
         [ Stage 1: Code Formatting & Static Analysis ]
         ├── cargo fmt --check
         └── cargo clippy --workspace --all-targets --all-features -- -D warnings
                                 │
                                 ▼ (Zero Warnings Permitted)
         [ Stage 2: Compilation & Workspace Type-Checking ]
         └── cargo check --workspace
                                 │
                                 ▼ (Zero Errors Permitted)
         [ Stage 3: Comprehensive Test Suite Execution ]
         └── cargo test --workspace
                                 │
                                 ▼ (100% Passing Tests)
         [ Stage 4: Git Commit & Integration Checkpoint ]
```

---

## 6. Open Questions & Testing Parameter Status

The following testing frameworks and parameter configurations remain designated as **TBD**:

| Area | Status | Scope |
| :--- | :--- | :--- |
| **`Code Coverage Target`** | `TBD` | Minimum percentage target for line/branch test coverage. |
| **`Property-Based Testing Library`** | `TBD` | Selection of Rust property testing harness (e.g., `proptest` vs. `quickcheck`). |
| **`Consensus Fuzzing Strategy`** | `TBD` | Structured binary payload fuzzing using `cargo-fuzz` / `libFuzzer`. |
| **`Multi-Node P2P Test Harness`** | `TBD` | Orchestration framework for multi-process local cluster testing. |
| **`Deterministic Mining Target`** | `TBD` | Standard difficulty constant for zero-latency test block generation. |
| **`Long-Running Soak Test Policy`**| `TBD` | Continuous multi-day network simulation rules. |

---

## 7. Cross-Specification References

- **[`docs/CONSENSUS-SPEC.md`](CONSENSUS-SPEC.md)**: Consensus invariants tested in Tier 3.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: Durability and atomic transaction tests in Tier 4.
- **[`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md)**: Networking and synchronization tests in Tier 5.
- **[`docs/MINING-LIFECYCLE-SPEC.md`](MINING-LIFECYCLE-SPEC.md)**: Autonomous mining tests in Tier 6.
- **[`docs/NODE-LIFECYCLE-SPEC.md`](NODE-LIFECYCLE-SPEC.md)**: Full runtime process tests in Tier 6.
