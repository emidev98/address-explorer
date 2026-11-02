-- Inputs of an indexed transaction.
--
-- Reverse transaction lookups scan input prevouts of the transactions stored
-- for known addresses, so the prevout address column is indexed.
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
CREATE INDEX IF NOT EXISTS idx_transaction_inputs_prevout_scriptpubkey_address
    ON transaction_inputs(prevout_scriptpubkey_address);
