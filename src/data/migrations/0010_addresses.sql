-- Stored Bitcoin addresses with their mempool statistics, an optional owner,
-- the transaction-index state, and the exchange ledger balance anchor.
--
-- A stored address belongs to either an exchange or a wallet, never both, and
-- may also be unclassified. SQLite cannot add a cross-column CHECK with
-- ALTER TABLE, so the either-or invariant is enforced with the triggers below
-- and repeated by the address repository before writing.
--
-- `tx_index_rate_limited` records that the transaction indexing walk stopped
-- on a mempool rate limit so the UI can warn and offer to resume.
--
-- `exchange_balance_sats` stores the aggregate ledger balance of a provider on
-- its alphabetically first address. It is derived state maintained by the
-- exchange and address repositories in the same transaction as every ledger or
-- address mutation that can change it.
CREATE TABLE IF NOT EXISTS addresses (
    address TEXT PRIMARY KEY,
    chain_funded_txo_count INTEGER NOT NULL,
    chain_funded_txo_sum INTEGER NOT NULL,
    chain_spent_txo_count INTEGER NOT NULL,
    chain_spent_txo_sum INTEGER NOT NULL,
    chain_tx_count INTEGER NOT NULL,
    mempool_funded_txo_count INTEGER NOT NULL,
    mempool_funded_txo_sum INTEGER NOT NULL,
    mempool_spent_txo_count INTEGER NOT NULL,
    mempool_spent_txo_sum INTEGER NOT NULL,
    mempool_tx_count INTEGER NOT NULL,
    description TEXT,
    exchange_id INTEGER REFERENCES exchanges_types(id),
    wallet_id INTEGER REFERENCES wallets_types(id),
    tx_index_rate_limited INTEGER NOT NULL DEFAULT 0,
    exchange_balance_sats INTEGER NOT NULL DEFAULT 0
);

-- Provider usage counts and the address owner pickers scan these columns;
-- SQLite does not index foreign keys automatically.
CREATE INDEX IF NOT EXISTS idx_addresses_exchange
    ON addresses(exchange_id);
CREATE INDEX IF NOT EXISTS idx_addresses_wallet
    ON addresses(wallet_id);

CREATE TRIGGER IF NOT EXISTS addresses_owner_either_insert
BEFORE INSERT ON addresses
WHEN NEW.exchange_id IS NOT NULL AND NEW.wallet_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'addresses.exchange_id and addresses.wallet_id cannot both be set');
END;

CREATE TRIGGER IF NOT EXISTS addresses_owner_either_update
BEFORE UPDATE OF exchange_id, wallet_id ON addresses
WHEN NEW.exchange_id IS NOT NULL AND NEW.wallet_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'addresses.exchange_id and addresses.wallet_id cannot both be set');
END;
