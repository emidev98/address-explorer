-- Outputs of an indexed transaction.
--
-- Reverse transaction lookups scan output addresses of the transactions
-- stored for known addresses, so the output address column is indexed.
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
CREATE INDEX IF NOT EXISTS idx_transaction_outputs_scriptpubkey_address
    ON transaction_outputs(scriptpubkey_address);
