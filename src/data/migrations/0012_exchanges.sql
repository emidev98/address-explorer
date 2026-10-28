-- Exchange ledger accounts imported from CSV exports.
--
-- Exchange transactions are kept separate from on-chain transactions: they
-- describe the exchange's own ledger, while `transactions` indexes publicly
-- visible Bitcoin transactions. Reconciliation links the two.
--
-- `exchange_balance_sats` stores the aggregate ledger balance of a provider on
-- its alphabetically first address. It is derived state maintained by the
-- exchange and address repositories in the same transaction as every ledger
-- or address mutation that can change it.

CREATE TABLE IF NOT EXISTS exchange_accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    provider_id INTEGER REFERENCES exchanges_types(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    last_imported_at INTEGER,
    last_modified_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_exchange_accounts_provider
    ON exchange_accounts(provider_id);

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

-- Historical fiat rates used to convert exchange fiat values into the
-- user's current default currency. Cached permanently so dynamic conversion
-- does not require a network request for every page load.
CREATE TABLE IF NOT EXISTS fiat_exchange_rates (
    base TEXT NOT NULL,
    quote TEXT NOT NULL,
    rate_date TEXT NOT NULL,
    rate REAL NOT NULL CHECK (rate > 0),
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (base, quote, rate_date)
);

ALTER TABLE addresses
    ADD COLUMN exchange_balance_sats INTEGER NOT NULL DEFAULT 0;
