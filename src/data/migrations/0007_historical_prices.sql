CREATE TABLE IF NOT EXISTS historical_prices (
    close_time INTEGER NOT NULL,
    close REAL NOT NULL,
    symbol TEXT NOT NULL,
    PRIMARY KEY (symbol, close_time)
) WITHOUT ROWID;
