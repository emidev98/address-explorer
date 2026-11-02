-- Publicly visible Bitcoin transactions indexed for a stored address.
-- Exchange ledger rows live in `exchange_transactions`; reconciliation links
-- the two.
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    address TEXT NOT NULL,
    txid TEXT NOT NULL,
    version INTEGER NOT NULL,
    locktime INTEGER NOT NULL,
    size INTEGER NOT NULL,
    weight INTEGER NOT NULL,
    fee INTEGER NOT NULL,
    confirmed INTEGER NOT NULL CHECK (confirmed IN (0, 1)),
    block_height INTEGER,
    block_hash TEXT,
    block_time INTEGER,
    synced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (address, txid),
    FOREIGN KEY (address) REFERENCES addresses(address) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_transactions_address ON transactions(address);
CREATE INDEX IF NOT EXISTS idx_transactions_txid ON transactions(txid);
