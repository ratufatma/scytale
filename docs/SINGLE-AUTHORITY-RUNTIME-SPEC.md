# Scytale Single-Authority Runtime Specification

**Status:** Architectural Constitution
**Version:** 1.0
**Scope:** Runtime authority, state ownership, module boundaries, lifecycle, and fail-stop behavior

## 1. Purpose

Scytale may use multiple Rust crates, applications, libraries, and supporting tools for maintainability and separation of concerns. This specification defines a stricter distinction between **code modularity** and **runtime authority**.

The governing principle is:

> **Do not fight code modularity. Prevent independent authority.**

A component may be structurally separate in the repository while remaining a subordinate implementation detail of the single Scytale runtime. No component outside the sovereign node may independently establish, mutate, or redefine canonical blockchain state.

This document is normative for all future architecture and implementation work. When an implementation conflicts with this document, the implementation is considered non-conforming until corrected or this specification is formally superseded.

## 2. Core Constitutional Rule

### 2.1 Single Sovereign Runtime

`scytale-node` is the sole **Sovereign Runtime** of Scytale.

Only `scytale-node` may own the live runtime lifecycle that establishes the canonical blockchain state and exposes authoritative node operations.

The sovereign runtime owns and coordinates, directly or indirectly:

- canonical chain state;
- canonical UTXO state;
- authoritative persistent blockchain storage;
- consensus validation;
- chain selection and reorganization;
- transaction admission into the node mempool;
- block acceptance and commitment;
- mining lifecycle;
- P2P synchronization and network-facing consensus participation;
- node startup, recovery, running, and shutdown;
- runtime integrity enforcement.

No second process, service, or library may become an alternative authority for any of these responsibilities.

### 2.2 One Source of Truth

Scytale shall have one authoritative blockchain state.

The authoritative state is represented by the node's canonical storage and the consensus/UTXO state reconstructed and maintained by `scytale-node`.

Derived stores, indexes, caches, explorers, analytics databases, and other projections are **not authoritative** and must always be rebuildable from authoritative node state.

## 3. Authority Is More Important Than Package Boundaries

Repository modularity is permitted and encouraged when it improves maintainability, testing, or compilation boundaries.

The following distinction is mandatory:

```text
crate separation       !=       runtime independence
process separation     !=       authority separation
library API            !=       ownership of canonical state
```

A crate can contain substantial blockchain logic while still having no independent runtime authority.

Conversely, a small helper that owns persistent state, performs autonomous mutation, or runs an independent lifecycle can become an architectural authority even if its code is small.

Therefore audits must inspect **state ownership, mutation paths, lifecycle, and dependency direction**, not merely directory names.

## 4. Four-Layer Architecture

Scytale components are classified into four layers.

### L0 — CONSENSUS CORE

L0 contains deterministic blockchain logic and primitives.

Typical members include:

- `scytale-primitives`
- `scytale-core`
- `scytale-script`
- `scytale-vm`
- `scytale-consensus`
- UTXO/eUTXO logic implemented as part of the core

L0 requirements:

- library-oriented code only;
- deterministic behavior where protocol rules require it;
- no sovereign runtime lifecycle;
- no independent node process;
- no independent canonical database;
- no independent canonical-state mutation authority;
- no alternative chain of truth;
- no policy that bypasses the node authority boundary.

A core library may expose functions for validation or state transformation. It does not thereby gain permission to decide when those transformations become canonical.

### L1 — NODE RUNTIME

L1 is `scytale-node`.

It is the sole sovereign runtime and the only layer authorized to turn validated operations into canonical state transitions.

L1 requirements:

- owns the runtime lifecycle;
- owns authoritative storage handles;
- owns canonical chain state;
- coordinates consensus, UTXO, mempool, P2P, mining, and recovery;
- enforces startup integrity checks;
- enforces fail-stop behavior for critical invariant failures;
- controls all authoritative mutation paths.

### L2 — CLIENT

L2 contains user-facing and developer-facing clients such as:

- `scytale-cli`
- Scytale Studio
- `scytale-py`
- SDK/client libraries

L2 may request operations and read node state through approved interfaces such as IPC or HTTP APIs.

L2 requirements:

- must not own canonical blockchain state;
- must not independently open or mutate canonical node storage;
- must not independently perform canonical chain selection;
- must not independently commit blocks or UTXO transitions;
- must not silently replace node consensus decisions;
- must treat node responses as authoritative for node state.

A client may perform local preparation, such as constructing a transaction or handling keys, but canonical admission remains a node responsibility.

### L3 — OBSERVER / TOOLING

L3 contains derived views and operational tooling, including:

- block explorers;
- relational indexes;
- analytics;
- monitoring;
- reporting;
- external dashboards;
- other derived projections.

L3 may maintain its own derived databases, provided those databases are explicitly non-authoritative and rebuildable.

L3 requirements:

- may observe;
- may index;
- may cache;
- may project;
- may rebuild from node state;
- must not become a second canonical state machine;
- must not determine canonical truth;
- must not mutate the blockchain directly;
- must not be a prerequisite for consensus correctness.

## 5. Authoritative State Model

The following state hierarchy is normative:

```text
                    SCYTALE NODE
                  SINGLE AUTHORITY
                         |
          +--------------+--------------+
          |              |              |
       Consensus      Storage         Runtime
          |              |              |
          +--------------+--------------+
                         |
                  CANONICAL STATE
                         |
              +----------+----------+
              |          |          |
             IPC        HTTP      Events
              |          |          |
             CLI       Studio   Observers
```

The important property is that all external interfaces terminate at the node authority boundary.

## 6. Storage Ownership

### 6.1 Authoritative Storage

The blockchain database used to recover and persist canonical state belongs exclusively to `scytale-node`.

No client, observer, indexer, or external service may open that database as an independent writer.

### 6.2 Derived Storage

A derived database may exist when it provides useful queries or projections. Such a database must satisfy all of the following:

1. It is explicitly classified as derived/non-authoritative.
2. Corruption does not alter canonical blockchain truth.
3. It can be deleted and rebuilt from the node's authoritative state.
4. Its failure cannot create a false canonical tip.
5. Its failure cannot authorize an invalid transaction or block.
6. Its schema is not a second protocol definition.

## 7. Runtime Lifecycle

Only the node controls the authoritative lifecycle.

The canonical lifecycle is conceptually:

```text
Starting
   -> Initializing
   -> Integrity Verification
   -> Recovery
   -> Synchronization
   -> Ready
   -> Running
   -> Stopping
   -> Stopped
```

No subordinate component may independently declare the blockchain healthy, canonical, synchronized, or ready for authoritative operation.

A subordinate component may report its own operational state, but that state must remain subordinate to node lifecycle semantics.

## 8. Critical vs Derived Failure

Not every failure should stop the node. The architecture distinguishes **critical integrity failures** from **derived-service failures**.

### 8.1 Critical Failure — FAIL STOP

The node must refuse to enter or remain in authoritative operation when a critical invariant is violated.

Examples include:

- canonical storage cannot be opened or verified;
- canonical chain reconstruction fails;
- consensus rules cannot be initialized consistently;
- genesis/network identity is inconsistent;
- UTXO reconstruction fails;
- block or transaction validation invariants fail;
- cryptographic verification infrastructure fails;
- canonical state is internally inconsistent;
- an authoritative mutation cannot be safely committed;
- a critical runtime component required for consensus authority is unavailable.

Required behavior:

```text
critical integrity failure
        -> error
        -> fail closed
        -> do not continue authoritative operation
```

The node must prefer stopping over continuing with an uncertain canonical state.

### 8.2 Derived-Service Failure — ISOLATE

A derived observer may fail without stopping the node, provided its failure cannot affect canonical state.

Examples include:

- explorer unavailable;
- SQLite index unavailable or corrupt;
- analytics pipeline unavailable;
- external HTTP webhook unavailable;
- monitoring dashboard unavailable;
- client disconnected.

Required behavior:

```text
observer failure
        -> isolate
        -> preserve node authority
        -> continue authoritative operation
        -> rebuild observer later
```

This is not a weakness in fail-closed design. It is preservation of authority boundaries.

## 9. Indexer Rule

All Scytale indexers are classified as **L3 derived observers** unless a future protocol specification explicitly changes that classification.

In particular, `scytale-indexer` and relational SQLite storage must not be treated as consensus authority.

The indexer may:

- receive committed-block events;
- construct relational views;
- maintain address/UTXO query projections;
- rebuild from authoritative storage;
- fail, restart, or be replaced independently.

The indexer must not:

- decide whether a block is canonical;
- authorize a transaction;
- replace UTXO consensus state;
- become necessary for node consensus startup;
- become a writer to authoritative blockchain storage;
- cause a valid node to stop solely because its derived view is unavailable.

If an indexer falls behind, the correct recovery mechanism is rebuild/backfill from authoritative node state.

## 10. Mutation Authority

Every canonical mutation must pass through the node authority boundary.

Examples:

```text
P2P block
   -> Node
   -> Consensus validation
   -> UTXO transition
   -> Canonical commit

RPC transaction
   -> Node
   -> Transaction validation
   -> Mempool admission
   -> Mining/confirmation
   -> Canonical commit

Mining result
   -> Node
   -> Block validation
   -> Canonical commit
```

Forbidden patterns include:

```text
CLI -> redb
Explorer -> redb
Indexer -> canonical DB
Studio -> UTXO mutation
Python client -> consensus state
Independent service -> canonical chain selection
```

## 11. Dependency Direction

Dependencies must flow toward authority, never around it.

The intended direction is:

```text
L3 observers  -> node interfaces
L2 clients    -> node interfaces
L1 node       -> L0 core libraries
L0 core       -> deterministic primitives
```

The following patterns are prohibited unless explicitly justified by a later constitutional amendment:

- L0 owning L1 lifecycle;
- L2 importing authoritative storage for direct mutation;
- L3 becoming a canonical data source;
- an observer deciding whether the node may accept consensus state;
- a derived database defining protocol truth.

## 12. Process Boundary Rules

Multiple processes are permitted, but only one process may be authoritative.

A deployment may contain:

```text
scytale-node
scytale-cli
scytale-studio
explorer
indexer
monitoring
```

This does not violate the constitution provided that only `scytale-node` owns canonical authority.

Running multiple programs does not imply multiple blockchain authorities.

## 13. Public Interfaces

IPC, HTTP, and event streams are controlled gateways into the node.

They may expose:

- state queries;
- transaction submission;
- operational commands explicitly authorized by the node;
- derived read APIs.

They must not expose an alternate path that bypasses node validation or canonical mutation controls.

The presence of an API is not permission for clients to directly manipulate underlying storage.

## 14. Integrity Attestation

The node may perform startup self-checks for critical subsystems.

Such checks must answer the question:

> Can the authoritative Scytale runtime safely establish and preserve canonical truth?

They should cover relevant critical invariants such as:

- storage readability and consistency;
- genesis/network identity;
- consensus parameter validity;
- monetary rule consistency;
- cryptographic verification;
- script execution baseline;
- UTXO integrity;
- block validation baseline;
- canonical chain recovery;
- runtime state machine integrity.

Self-checks must not falsely claim that optional observers are consensus-critical.

## 15. Anti-Fragmentation Rules

The following are hard architectural prohibitions.

### Rule A — No Second Sovereign

No program other than `scytale-node` may claim to be a Scytale node authority.

### Rule B — No Second Canonical Database

No component may maintain an independent database that is treated as canonical blockchain truth.

### Rule C — No Direct Canonical Mutation

No L2 or L3 component may mutate canonical chain state except through the node's authoritative interfaces.

### Rule D — No Autonomous Consensus Lifecycle

Consensus libraries may expose validation functions, but must not independently decide runtime startup, synchronization, or canonical commitment.

### Rule E — No Hidden Authority

A component does not become architectural infrastructure merely because it has its own thread, process, database, queue, cache, or API. Authority is determined by what state it can define or mutate.

### Rule F — Derived State Must Be Disposable

Any L3 database must be safe to delete and rebuild without changing blockchain truth.

### Rule G — Critical Failure Must Fail Closed

If canonical correctness cannot be established or preserved, the node must stop authoritative operation rather than guess.

### Rule H — Observer Failure Must Not Corrupt Authority

A failed observer must never force the node to adopt observer state, fabricate state, or alter canonical truth.

## 16. Module Design Test

Every new component must answer these questions before acceptance:

1. Does it own persistent state?
2. Does that state claim to be canonical?
3. Can it mutate blockchain state without `scytale-node`?
4. Can it start an independent authoritative lifecycle?
5. Can the node remain correct if it is removed?
6. Can its state be rebuilt from authoritative node state?
7. Can its failure alter consensus decisions?
8. Does another component depend on it as an alternative source of truth?

Interpretation:

- If the answer to question 2, 3, or 4 is **yes**, the component is not safely subordinate and requires architectural review.
- If question 6 is **no** for an observer, the design is suspect and must be reviewed.
- If question 7 is **yes**, the component is consensus-adjacent and must be classified explicitly rather than being treated as ordinary tooling.

## 17. Required Audit Targets

Future repository audits must inspect at minimum:

- every `Cargo.toml` package and workspace member;
- every executable `main` entry point;
- every persistent database opener;
- every filesystem path containing runtime state;
- every function capable of canonical mutation;
- all IPC/HTTP mutation endpoints;
- all P2P block/transaction ingress paths;
- mining-to-node commit paths;
- indexer and explorer integrations;
- client dependencies on internal crates;
- lifecycle/startup/shutdown code;
- tests that instantiate components independently.

Tests that instantiate a core component independently are not automatically wrong; the audit must determine whether they demonstrate code-testability or expose an unintended production authority boundary.

## 18. Acceptance Criteria

The implementation conforms to this specification only when all of the following are true:

1. `scytale-node` is the only sovereign runtime.
2. Canonical blockchain storage has one owner: the node runtime.
3. Canonical chain selection and commitment are performed only by the node authority.
4. L0 libraries have no independent authoritative lifecycle.
5. L2 clients cannot mutate canonical storage directly.
6. L3 observers cannot become canonical truth.
7. Derived databases can be rebuilt from authoritative state.
8. Critical integrity failures cause fail-stop behavior.
9. Derived-service failures are isolated from consensus authority.
10. No hidden second authority exists through an API, worker, thread, sidecar, or standalone service.
11. The dependency graph reinforces the authority hierarchy.
12. Tests and CI enforce the architectural boundaries rather than relying solely on documentation.

## 19. Current Architectural Direction

The current repository already establishes `scytale-node` as the default workspace runtime and contains explicit node-level coordination of storage, consensus, UTXO, mempool, mining, P2P, and recovery.

However, the indexer architecture must be normalized to this constitution. The relational SQLite indexer and external explorer indexer are derived observers and must not occupy a contradictory status in which they are optional during commit but critical during startup.

The desired direction is:

```text
Authoritative:
    scytale-node
        -> canonical storage
        -> consensus
        -> UTXO
        -> runtime lifecycle

Derived:
    indexer
    explorer
    analytics
    monitoring

Clients:
    CLI
    Studio
    Python
```

## 20. Change Control

Any change that introduces a new process, persistent state store, runtime worker, service, API, or standalone executable must explicitly identify its layer and authority status.

A change is architecturally non-conforming when it introduces a new source of canonical truth or an independent mutation path without amending this specification.

The burden of proof belongs to the new design: a component must demonstrate why its independence does not create independent authority.

## 21. Final Principle

Scytale is modular in code but singular in authority.

```text
Many crates.
Many interfaces.
Many clients.
Many derived views.

One canonical state.
One consensus authority.
One sovereign runtime.
```

That is the governing architectural identity of Scytale.
