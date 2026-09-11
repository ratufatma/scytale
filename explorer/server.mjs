import express from 'express';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync, readFileSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import {
  upsertBlock,
  getRecentBlocks,
  getBlockByIdentifier,
  getLatestBlock,
  getBlockCount
} from './db.mjs';

const execFileAsync = promisify(execFile);

const __dirname = dirname(fileURLToPath(import.meta.url));
const app = express();
const nodeBaseUrl = (process.env.NODE_URL || '').replace(/\/+$/, '');
const indexerKey = process.env.INDEXER_KEY || process.env.EXPLORER_API_KEY || '';

if (process.env.NODE_ENV === 'production' && !indexerKey) {
  console.error('Missing mandatory INDEXER_KEY in environment');
  process.exit(1);
}

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
  if (!indexerKey || token !== indexerKey) {
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
    if (rows.length === 0) {
      return res.status(404).json({ error: 'No indexed canonical blocks found' });
    }
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

    if (!latest) {
      return res.status(404).json({ error: 'No indexed canonical block found' });
    }
    const tipHash = latest.hash.startsWith('0x') ? latest.hash : `0x${latest.hash}`;

    res.json({
      runtime_state: 'Operational',
      canonical_height: latest.height,
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
    '/api/v1/passbook',
    '/api/v1/passbook/statement',
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
// Faucet Endpoints (POST /api/v1/faucet & GET /api/v1/faucet/info)
// ─────────────────────────────────────────────────────────────────────────────

const faucetCooldowns = new Map(); // address -> timestamp
const ipCooldowns = new Map();      // ip -> timestamp
const COOLDOWN_MS = 24 * 60 * 60 * 1000;

const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
const BECH32_GENERATORS = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

function bech32Polymod(values) {
  let checksum = 1;
  for (const value of values) {
    const top = checksum >>> 25;
    checksum = ((checksum & 0x1ffffff) << 5) ^ value;
    for (let bit = 0; bit < 5; bit += 1) {
      if ((top >>> bit) & 1) checksum ^= BECH32_GENERATORS[bit];
    }
  }
  return checksum >>> 0;
}

function bech32HrpExpand(hrp) {
  return [
    ...[...hrp].map((character) => character.charCodeAt(0) >>> 5),
    0,
    ...[...hrp].map((character) => character.charCodeAt(0) & 31)
  ];
}

function isValidScytaleAddress(value) {
  if (typeof value !== 'string' || value.length < 14 || value.length > 90 || value !== value.toLowerCase()) {
    return false;
  }
  const separator = value.lastIndexOf('1');
  if (separator < 1 || separator + 7 > value.length || value.slice(0, separator) !== 'scy') {
    return false;
  }
  const data = [...value.slice(separator + 1)].map((character) => BECH32_CHARSET.indexOf(character));
  return data.every((item) => item >= 0)
    && bech32Polymod([...bech32HrpExpand('scy'), ...data]) === 1;
}

const FAUCET_WALLET = process.env.FAUCET_WALLET || '/var/lib/scytale/faucet_wallet.json';
function getFaucetAddress() {
  if (process.env.FAUCET_ADDRESS) return process.env.FAUCET_ADDRESS;
  try {
    if (existsSync(FAUCET_WALLET)) {
      const w = JSON.parse(readFileSync(FAUCET_WALLET, 'utf8'));
      if (w.address) return w.address;
    }
  } catch {}
  return 'scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q';
}
const CLI_PATH = process.env.SCYTALE_CLI_BIN || '/usr/local/bin/scytale-cli';
const SOCKET_PATH = process.env.SCYTALE_SOCKET || '/run/scytale/node.sock';

async function handleFaucetInfo(req, res) {
  let reserve_balance_scy = 0.0;
  let reserve_quanta = 0;
  const faucetAddr = getFaucetAddress();
  try {
    if (nodeBaseUrl) {
      const resp = await fetch(`${nodeBaseUrl}/api/v1/passbook?address=${faucetAddr}`, {
        signal: AbortSignal.timeout(3000)
      });
      if (resp.ok) {
        const pb = await resp.json();
        const conf = Number(pb.confirmed_native_balance_quanta) || 0;
        const pend = Number(pb.pending_native_balance_quanta) || 0;
        reserve_quanta = conf > 0 ? conf : pend;
        reserve_balance_scy = Number((reserve_quanta / 100000000).toFixed(8));
      }
    }
  } catch (err) {
    console.error('[Faucet Info Error]', err.message);
  }

  return res.json({
    faucet_address: faucetAddr,
    dispense_amount_scy: 10.0,
    cooldown_seconds: 1800,
    reserve_balance_scy,
    reserve_quanta
  });
}

async function handleFaucetClaim(req, res) {
  const body = req.body || {};
  const { address } = body;
  const clientIp = (req.headers['x-forwarded-for'] || '').split(',')[0].trim() || req.socket.remoteAddress || '127.0.0.1';

  if (!isValidScytaleAddress(address)) {
    return res.status(400).json({ error: 'Alamat Bech32 Scytale tidak valid (harus diawali scy1).' });
  }

  const now = Date.now();
  if (faucetCooldowns.has(address) && (now - faucetCooldowns.get(address) < COOLDOWN_MS)) {
    const remainingMin = Math.ceil((COOLDOWN_MS - (now - faucetCooldowns.get(address))) / 60000);
    return res.status(429).json({
      error: `Rate limit reached. Please wait ${remainingMin} more minute(s) before requesting again.`,
      remaining_minutes: remainingMin
    });
  }

  if (ipCooldowns.has(clientIp) && (now - ipCooldowns.get(clientIp) < COOLDOWN_MS)) {
    const remainingMin = Math.ceil((COOLDOWN_MS - (now - ipCooldowns.get(clientIp))) / 60000);
    return res.status(429).json({
      error: `Rate limit reached for your IP. Please wait ${remainingMin} more minute(s) before requesting again.`,
      remaining_minutes: remainingMin
    });
  }

  try {
    const { stdout, stderr } = await execFileAsync(CLI_PATH, [
      '--socket', SOCKET_PATH,
      'transfer-p2pkh',
      '--wallet-file', FAUCET_WALLET,
      '--to', address,
      '--amount', '1000000000',
      '--fee', '1000'
    ], { timeout: 30_000, windowsHide: true });
    const output = stdout + '\n' + (stderr || '');
    const match = output.match(/0x[a-fA-F0-9]{64}/);
    const txid = match ? match[0] : null;

    if (!txid) {
      throw new Error(`Tidak dapat mengekstrak TxID: ${output}`);
    }

    faucetCooldowns.set(address, now);
    ipCooldowns.set(clientIp, now);

    return res.status(200).json({
      success: true,
      txid,
      amount_scy: 10.0,
      recipient: address
    });
  } catch (err) {
    console.error('[Faucet Distribution Error]', err);
    return res.status(500).json({
      error: 'Gagal mendistribusikan koin faucet',
      details: err.message || String(err)
    });
  }
}

app.post('/api/faucet', handleFaucetClaim);
app.post('/api/v1/faucet', handleFaucetClaim);
app.post('/rpc/api/v1/faucet', handleFaucetClaim);

app.get('/api/faucet/info', handleFaucetInfo);
app.get('/api/v1/faucet/info', handleFaucetInfo);
app.get('/rpc/api/v1/faucet/info', handleFaucetInfo);

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
// Block Reconciler & Historical Catch-Up
// ─────────────────────────────────────────────────────────────────────────────

let isReconciling = false;

/**
 * Reconciles missing blocks from the node into the local SQLite database.
 * Compares local max height with upstream canonical_height, fetching missing blocks in ascending order.
 */
export async function reconcileBlocks() {
  if (!nodeBaseUrl || isReconciling) return;
  isReconciling = true;

  try {
    const statusRes = await fetch(`${nodeBaseUrl}/api/v1/status`, {
      signal: AbortSignal.timeout(5000)
    });
    if (!statusRes.ok) return;
    const status = await statusRes.json();
    const tipHeight = Number(status.canonical_height);
    if (isNaN(tipHeight)) return;

    const latestBlock = getLatestBlock();
    const localHeight = latestBlock ? Number(latestBlock.height) : -1;

    if (localHeight >= tipHeight) return;

    console.log(`[Scytale Explorer Reconciler] Catching up: local height ${localHeight} vs node tip ${tipHeight}`);

    let currentFrom = localHeight + 1;
    while (currentFrom <= tipHeight) {
      const batchLimit = Math.min(50, tipHeight - currentFrom + 1);
      const blocksRes = await fetch(
        `${nodeBaseUrl}/api/v1/blocks?from_height=${currentFrom}&limit=${batchLimit}&order=asc`,
        { signal: AbortSignal.timeout(10000) }
      );
      if (!blocksRes.ok) break;

      const summaries = await blocksRes.json();
      if (!Array.isArray(summaries) || summaries.length === 0) break;

      for (const summary of summaries) {
        let miner = 'Unknown';
        try {
          const detailRes = await fetch(`${nodeBaseUrl}/api/v1/blocks/${summary.height}`, {
            signal: AbortSignal.timeout(5000)
          });
          if (detailRes.ok) {
            const detail = await detailRes.json();
            const coinbaseTx = (detail.transactions || []).find(t => t.is_coinbase);
            if (coinbaseTx && coinbaseTx.outputs && coinbaseTx.outputs.length > 0) {
              miner = coinbaseTx.outputs[0].address || coinbaseTx.outputs[0].locking_script_hex || 'Unknown';
            }
          }
        } catch {
          // Keep default miner on individual detail timeout
        }

        upsertBlock({
          height: summary.height,
          hash: summary.hash,
          prev_hash: summary.previous_block_hash,
          miner,
          timestamp: summary.timestamp,
          tx_count: summary.tx_count
        });
      }

      currentFrom += summaries.length;
    }
  } catch (err) {
    console.error('[Scytale Explorer Reconciler] Error during reconciliation:', err.message);
  } finally {
    isReconciling = false;
  }
}

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
    if (nodeBaseUrl) {
      console.log(`[Scytale Explorer] Node upstream configured at ${nodeBaseUrl}. Starting block reconciler...`);
      reconcileBlocks();
      setInterval(reconcileBlocks, 15000);
    }
  });
}

export { app, serverInstance };
export default app;
