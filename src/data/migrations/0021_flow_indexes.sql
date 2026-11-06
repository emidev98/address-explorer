-- Read-path indexes for the Flow screen.
--
-- Flow applies its filters and date range before loading any transaction, so
-- the date bound can use an index instead of scanning every stored row. The
-- ledger timestamp index supports the same bound across all exchange accounts,
-- and the on-chain txid index resolves matched ledger rows efficiently.
CREATE INDEX IF NOT EXISTS idx_transactions_block_time
    ON transactions(block_time);

CREATE INDEX IF NOT EXISTS idx_exchange_transactions_timestamp
    ON exchange_transactions(timestamp);

CREATE INDEX IF NOT EXISTS idx_exchange_transactions_on_chain_txid
    ON exchange_transactions(on_chain_txid);
