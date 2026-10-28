-- Look up all symbols stored at one close time without scanning the table:
-- the prices table's "other available exchanges" column and the
-- per-close-time conversions endpoint both read by close_time, while the
-- WITHOUT ROWID primary key leads with the symbol.
CREATE INDEX IF NOT EXISTS idx_historical_prices_close_time
    ON historical_prices(close_time);
