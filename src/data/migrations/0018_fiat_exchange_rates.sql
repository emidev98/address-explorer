-- Historical fiat rates used to convert exchange fiat values into the user's
-- current default currency. Cached permanently so dynamic conversion does not
-- require a network request for every page load.
CREATE TABLE IF NOT EXISTS fiat_exchange_rates (
    base TEXT NOT NULL,
    quote TEXT NOT NULL,
    rate_date TEXT NOT NULL,
    rate REAL NOT NULL CHECK (rate > 0),
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (base, quote, rate_date)
);
