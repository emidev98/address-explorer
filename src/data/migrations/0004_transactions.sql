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

CREATE TABLE IF NOT EXISTS transaction_inputs (
    id INTEGER PRIMARY KEY,
    transaction_id INTEGER NOT NULL,
    input_index INTEGER NOT NULL,
    prev_txid TEXT NOT NULL,
    prev_vout INTEGER NOT NULL,
    prevout_scriptpubkey TEXT,
    prevout_scriptpubkey_asm TEXT,
    prevout_scriptpubkey_type TEXT,
    prevout_scriptpubkey_address TEXT,
    prevout_value INTEGER,
    scriptsig TEXT NOT NULL,
    scriptsig_asm TEXT NOT NULL,
    is_coinbase INTEGER NOT NULL CHECK (is_coinbase IN (0, 1)),
    sequence INTEGER NOT NULL,
    UNIQUE (transaction_id, input_index),
    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_transaction_inputs_transaction
    ON transaction_inputs(transaction_id);

CREATE TABLE IF NOT EXISTS transaction_outputs (
    id INTEGER PRIMARY KEY,
    transaction_id INTEGER NOT NULL,
    output_index INTEGER NOT NULL,
    scriptpubkey TEXT NOT NULL,
    scriptpubkey_asm TEXT NOT NULL,
    scriptpubkey_type TEXT NOT NULL,
    scriptpubkey_address TEXT,
    value INTEGER NOT NULL,
    UNIQUE (transaction_id, output_index),
    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_transaction_outputs_transaction
    ON transaction_outputs(transaction_id);
