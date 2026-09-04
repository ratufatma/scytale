import Database from 'better-sqlite3';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync, mkdirSync } from 'node:fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const dataDir = join(__dirname, 'data');

if (!existsSync(dataDir)) {
  mkdirSync(dataDir, { recursive: true });
}

const dbPath = process.env.EXPLORER_DB_PATH || join(dataDir, 'explorer.db');
const db = new Database(dbPath);

// Enable WAL mode for high concurrency
db.pragma('journal_mode = WAL');

// Initialize database schema
db.exec(`
  CREATE TABLE IF NOT EXISTS blocks (
    height INTEGER PRIMARY KEY,
    hash TEXT UNIQUE NOT NULL,
    prev_hash TEXT NOT NULL,
    miner TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tx_count INTEGER NOT NULL,
    created_at INTEGER NOT NULL
  );

  CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);
  CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);
`);

const stmtUpsert = db.prepare(`
  INSERT INTO blocks (height, hash, prev_hash, miner, timestamp, tx_count, created_at)
  VALUES (@height, @hash, @prev_hash, @miner, @timestamp, @tx_count, @created_at)
  ON CONFLICT(height) DO UPDATE SET
    hash = excluded.hash,
    prev_hash = excluded.prev_hash,
    miner = excluded.miner,
    timestamp = excluded.timestamp,
    tx_count = excluded.tx_count,
    created_at = excluded.created_at
`);

const stmtRecentBlocks = db.prepare(`
  SELECT height, hash, prev_hash, miner, timestamp, tx_count, created_at
  FROM blocks
  ORDER BY height DESC
  LIMIT ?
`);

const stmtBlockByHeight = db.prepare(`
  SELECT height, hash, prev_hash, miner, timestamp, tx_count, created_at
  FROM blocks
  WHERE height = ?
`);

const stmtBlockByHash = db.prepare(`
  SELECT height, hash, prev_hash, miner, timestamp, tx_count, created_at
  FROM blocks
  WHERE hash = ? OR hash = ?
`);

const stmtLatestBlock = db.prepare(`
  SELECT height, hash, prev_hash, miner, timestamp, tx_count, created_at
  FROM blocks
  ORDER BY height DESC
  LIMIT 1
`);

const stmtCountBlocks = db.prepare(`
  SELECT COUNT(*) as count FROM blocks
`);

/**
 * Inserts or updates a block in the database.
 * @param {Object} block
 * @param {number} block.height
 * @param {string} block.hash
 * @param {string} block.prev_hash
 * @param {string} block.miner
 * @param {number} block.timestamp
 * @param {number} block.tx_count
 */
export function upsertBlock(block) {
  const cleanHash = block.hash.replace(/^0x/i, '').toLowerCase();
  const cleanPrevHash = block.prev_hash.replace(/^0x/i, '').toLowerCase();

  return stmtUpsert.run({
    height: Number(block.height),
    hash: cleanHash,
    prev_hash: cleanPrevHash,
    miner: String(block.miner),
    timestamp: Number(block.timestamp),
    tx_count: Number(block.tx_count),
    created_at: Date.now()
  });
}

/**
 * Returns recent blocks sorted by height descending.
 * @param {number} limit
 */
export function getRecentBlocks(limit = 10) {
  const lim = Math.max(1, Math.min(100, Number(limit) || 10));
  return stmtRecentBlocks.all(lim);
}

/**
 * Finds a block by height or hash.
 * @param {string|number} identifier
 */
export function getBlockByIdentifier(identifier) {
  const idStr = String(identifier).trim();
  if (/^\d+$/.test(idStr)) {
    const row = stmtBlockByHeight.get(Number(idStr));
    if (row) return row;
  }

  const clean = idStr.replace(/^0x/i, '').toLowerCase();
  return stmtBlockByHash.get(clean, `0x${clean}`);
}

/**
 * Returns the highest canonical block in the database.
 */
export function getLatestBlock() {
  return stmtLatestBlock.get() || null;
}

/**
 * Returns the total count of blocks stored in the database.
 */
export function getBlockCount() {
  const row = stmtCountBlocks.get();
  return row ? row.count : 0;
}

export default db;
