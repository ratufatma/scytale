# Node Lifecycle Specification

## Startup

1. Parse CLI configuration.
2. Open the embedded storage engine.
3. Load the canonical chain and UTXO state.
4. Initialize the mempool, indexer, and mining worker.
5. Start the IPC server and optional HTTP gateway.
6. Initialize the transport-neutral network boundary when a direct peer transport is available.

Consensus and storage initialization do not depend on an external broker.

## Runtime

The node coordinates storage, consensus, mempool, mining, IPC, HTTP, and network components. Incoming blocks and transactions must enter the same validation paths as locally created data.

## Shutdown

1. Stop accepting new control requests.
2. Stop mining and background workers.
3. Stop the HTTP gateway and network transport.
4. Flush and close storage.
5. Remove temporary IPC resources.

Shutdown must be bounded and must not leave detached tasks operating on released node state.
