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
    mempool_tx_count INTEGER NOT NULL
);
