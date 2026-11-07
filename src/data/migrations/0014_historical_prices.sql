-- Historical prices per symbol and close time.
--
-- USD is the reference series imported by `historical_prices_data`; other
-- symbols (the user's default fiat currency) are materialized at read time by
-- the prices service.
--
-- The WITHOUT ROWID primary key leads with the symbol. The close-time index
-- serves the "other available currencies" column and the per-close-time
-- conversion endpoint, which both read every symbol stored at one close time.
CREATE TABLE IF NOT EXISTS historical_prices (
    close_time INTEGER NOT NULL,
    close REAL NOT NULL,
    symbol TEXT NOT NULL,
    PRIMARY KEY (symbol, close_time)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_historical_prices_close_time
    ON historical_prices(close_time);
