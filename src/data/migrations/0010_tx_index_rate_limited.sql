-- Track addresses whose transaction indexing stopped on a mempool rate
-- limit, so the UI can warn and offer resuming the indexing walk.
ALTER TABLE addresses
    ADD COLUMN tx_index_rate_limited INTEGER NOT NULL DEFAULT 0;
