-- Exchange ledger accounts imported from CSV exports.
--
-- Exchange transactions are kept separate from on-chain `transactions`: they
-- describe the exchange's own ledger, while `transactions` indexes publicly
-- visible Bitcoin transactions. Reconciliation links the two.
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
