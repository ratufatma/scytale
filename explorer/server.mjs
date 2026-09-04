import express from 'express';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync, readFileSync } from 'node:fs';
import {
  upsertBlock,
  getRecentBlocks,
  getBlockByIdentifier,
  getLatestBlock,
  getBlockCount
} from './db.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const app = express();
const nodeBaseUrl = (process.env.NODE_URL || '').replace(/\/+$/, '');

// Middleware: CORS
app.use((req, res, next) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  if (req.method === 'OPTIONS') {
    return res.sendStatus(204);
  }
  next();
});

// Middleware: JSON body parser
app.use(express.json());

// Authentication Middleware for Ingest
function authenticateIndexer(req, res, next) {
  const authHeader = req.headers.authorization;
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return res.status(401).json({ error: 'Unauthorized: Missing or invalid Bearer token' });
  }

  const token = authHeader.slice(7).trim();
  const expectedKey = process.env.INDEXER_KEY || process.env.EXPLORER_API_KEY || 'secret123';

  if (token !== expectedKey) {
    return res.status(401).json({ error: 'Unauthorized: Invalid API key' });
  }

  next();
}

/**
 * Formats a database block record to match the domain Block JSON expected by the frontend.
 */
function formatBlock(row) {
  const hashStr = row.hash.startsWith('0x') ? row.hash : `0x${row.hash}`;
  const prevStr = row.prev_hash.startsWith('0x') ? row.prev_hash : `0x${row.prev_hash}`;

  return {
    height: row.height,
    hash: hashStr,
    previous_block_hash: prevStr,
    miner: row.miner,
    timestamp: row.timestamp,
    tx_count: row.tx_count,
    nonce: 0,
    total_quanta: 0,
    total_scy: '0.00000000',
    transactions: []
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Ingest Endpoint (POST /api/ingest & POST /rpc/api/ingest)
// ─────────────────────────────────────────────────────────────────────────────

function handleIngest(req, res) {
  const body = req.body;
  if (!body || typeof body !== 'object') {
    return res.status(400).json({ error: 'Bad Request: Missing JSON body' });
  }

  const { height, hash, prev_hash, miner, timestamp, tx_count } = body;

  if (
    height === undefined || height === null || typeof height !== 'number' || height < 0 ||
    !hash || typeof hash !== 'string' ||
    prev_hash === undefined || typeof prev_hash !== 'string' ||
    miner === undefined || typeof miner !== 'string' ||
    timestamp === undefined || typeof timestamp !== 'number' ||
    tx_count === undefined || typeof tx_count !== 'number'
  ) {
    return res.status(400).json({
      error: 'Bad Request: Missing or invalid required fields (height, hash, prev_hash, miner, timestamp, tx_count)'
    });
  }

  try {
    upsertBlock({
      height,
      hash,
      prev_hash,
      miner,
      timestamp,
      tx_count
    });

    return res.status(200).json({
      status: 'ok',
      height
    });
  } catch (err) {
    console.error('Error inserting block into database:', err);
    return res.status(500).json({ error: 'Internal Server Error: Failed to persist block' });
  }
}

app.post('/api/ingest', authenticateIndexer, handleIngest);
app.post('/rpc/api/ingest', authenticateIndexer, handleIngest);
app.post('/api/v1/ingest', authenticateIndexer, handleIngest);

// ─────────────────────────────────────────────────────────────────────────────
// Query Endpoints (Blocks, Status)
// ─────────────────────────────────────────────────────────────────────────────

function handleGetBlocks(req, res) {
  try {
    const limit = Number(req.query.limit) || 10;
    const rows = getRecentBlocks(limit);
    const formatted = rows.map(formatBlock);
    res.json(formatted);
  } catch (err) {
    console.error('Error retrieving blocks:', err);
    res.status(500).json({ error: 'Failed to retrieve blocks' });
  }
}

app.get('/api/blocks', handleGetBlocks);
app.get('/api/v1/blocks', handleGetBlocks);
app.get('/rpc/api/blocks', handleGetBlocks);
app.get('/rpc/api/v1/blocks', handleGetBlocks);

function handleGetBlockById(req, res) {
  try {
    const row = getBlockByIdentifier(req.params.id);
    if (!row) {
      return res.status(404).json({ error: 'Block not found' });
    }
    res.json(formatBlock(row));
  } catch (err) {
    console.error('Error finding block:', err);
    res.status(500).json({ error: 'Failed to look up block' });
  }
}

app.get('/api/blocks/:id', handleGetBlockById);
app.get('/api/v1/blocks/:id', handleGetBlockById);
app.get('/rpc/api/blocks/:id', handleGetBlockById);
app.get('/rpc/api/v1/blocks/:id', handleGetBlockById);

function handleGetStatus(req, res) {
  try {
    const latest = getLatestBlock();
    const count = getBlockCount();

    const tipHash = latest
      ? (latest.hash.startsWith('0x') ? latest.hash : `0x${latest.hash}`)
      : '0x0000000000000000000000000000000000000000000000000000000000000000';

    res.json({
      runtime_state: 'Operational',
      canonical_height: latest ? latest.height : 0,
      canonical_tip: tipHash,
      indexed_blocks_count: count,
      peer_count: 0,
      mempool_tx_count: 0,
      mining_active: false
    });
  } catch (err) {
    console.error('Error retrieving status:', err);
    res.status(500).json({ error: 'Failed to retrieve status' });
  }
}

// Forward read-only explorer queries to the node when one is configured.
async function proxyNodeRequest(req, res, next) {
  if (!nodeBaseUrl) return next();

  try {
    const upstream = await fetch(`${nodeBaseUrl}${req.originalUrl}`, {
      method: req.method,
      headers: {
        accept: req.headers.accept || 'application/json'
      }
    });

    res.status(upstream.status);
    const contentType = upstream.headers.get('content-type');
    if (contentType) res.setHeader('content-type', contentType);
    res.send(await upstream.text());
  } catch (err) {
    console.error(`Error proxying request to node (${req.originalUrl}):`, err);
    res.status(502).json({ error: 'Node unavailable' });
  }
}

if (nodeBaseUrl) {
  for (const path of [
    '/api/v1/status',
    '/api/v1/blocks',
    '/api/v1/blocks/:id',
    '/api/v1/tx/:id',
    '/api/v1/mempool',
    '/api/v1/passbook/:lock'
  ]) {
    app.get(path, proxyNodeRequest);
  }
}

app.get('/api/status', handleGetStatus);
app.get('/api/v1/status', handleGetStatus);
app.get('/rpc/api/status', handleGetStatus);
app.get('/rpc/api/v1/status', handleGetStatus);

// ─────────────────────────────────────────────────────────────────────────────
// Static Files & Web Interface
// ─────────────────────────────────────────────────────────────────────────────

app.get('/', (req, res) => {
  res.sendFile(join(__dirname, 'index.html'));
});

app.get('/index.html', (req, res) => {
  res.sendFile(join(__dirname, 'index.html'));
});

app.get('/favicon.svg', (req, res) => {
  res.type('image/svg+xml').sendFile(join(__dirname, 'favicon.svg'));
});

app.get('/gemini-svg.svg', (req, res) => {
  res.type('image/svg+xml').sendFile(join(__dirname, 'gemini-svg.svg'));
});

app.get('/logo.svg', (req, res) => {
  res.type('image/svg+xml').sendFile(join(__dirname, 'logo.svg'));
});

app.get('/favicon.ico', (req, res) => {
  res.type('image/svg+xml').sendFile(join(__dirname, 'favicon.svg'));
});

app.use(express.static(__dirname));

// ─────────────────────────────────────────────────────────────────────────────
// Server Start
// ─────────────────────────────────────────────────────────────────────────────

const PORT = Number(process.env.PORT) || 3000;
const HOST = process.env.HOST || '0.0.0.0';

let serverInstance = null;
if (process.env.NODE_ENV !== 'test') {
  serverInstance = app.listen(PORT, HOST, () => {
    console.log(`[Scytale Explorer] Server running on http://${HOST}:${PORT}`);
    console.log(`[Scytale Explorer] Ingest endpoint ready at POST http://${HOST}:${PORT}/api/ingest`);
  });
}

export { app, serverInstance };
export default app;
