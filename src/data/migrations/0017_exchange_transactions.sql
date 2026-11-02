-- Ledger rows of an imported exchange account.
CREATE TABLE IF NOT EXISTS exchange_transactions (
    id INTEGER PRIMARY KEY,
    exchange_id INTEGER NOT NULL,
    fingerprint TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tx_type TEXT NOT NULL CHECK (
        tx_type IN ('buy', 'sell', 'reward', 'withdrawal', 'deposit', 'unknown')
    ),
    amount_sats INTEGER NOT NULL,
    currency TEXT NOT NULL CHECK (currency IN ('BTC', 'XBT')),
    fiat_amount REAL,
    fiat_currency TEXT,
    fee REAL,
    fee_currency TEXT,
    tx_hash TEXT,
    description TEXT,
    raw_data TEXT NOT NULL,
    match_state TEXT NOT NULL CHECK (
        match_state IN ('hash_match', 'amount_date_match', 'manual_match', 'unmatched')
    ),
    matched_tx_id TEXT,
    on_chain_txid TEXT,
    on_chain_source TEXT CHECK (
        on_chain_source IS NULL OR on_chain_source IN ('hash', 'amount_date', 'manual')
    ),
    on_chain_linked_at INTEGER,
    on_chain_local_addresses TEXT,
    is_manually_edited INTEGER NOT NULL DEFAULT 0 CHECK (is_manually_edited IN (0, 1)),
    is_intentional_duplicate INTEGER NOT NULL DEFAULT 0
        CHECK (is_intentional_duplicate IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (exchange_id) REFERENCES exchange_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_exchange_transactions_account_fingerprint
    ON exchange_transactions(exchange_id, fingerprint);
CREATE INDEX IF NOT EXISTS idx_exchange_transactions_account_timestamp
    ON exchange_transactions(exchange_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_exchange_transactions_matched_txid
    ON exchange_transactions(matched_tx_id);
