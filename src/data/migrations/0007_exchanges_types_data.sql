-- Seed rows of the exchange provider catalog.
--
-- Names are well-known platforms that appear in current exchange volume
-- rankings. IDs are explicit so other tables keep stable references, and every
-- row gets its initial slug, category, custody model, and brand color here.
-- The seed rows are fully user-owned after this migration: future migrations
-- MUST NOT re-seed providers by name.
--
-- `brand_color` is the provider's primary published brand color taken from its
-- official brand guidelines or press kit (researched 2026-11). It only seeds a
-- badge background; users may change it, and the UI derives a fallback color
-- when a row has none. Keep the values in sync with
-- `0020_provider_brand_colors.sql`, which backfills databases seeded before the
-- colors were added.
INSERT OR IGNORE INTO exchanges_types
    (id, name, slug, category_id, custody_model, brand_color)
VALUES
    (1, 'Binance', 'binance', 1, 'custodial', '#F0B90B'),
    (2, 'Coinbase', 'coinbase', 1, 'custodial', '#0052FF'),
    (3, 'Kraken', 'kraken', 1, 'custodial', '#5741D9'),
    (4, 'OKX', 'okx', 1, 'custodial', '#000000'),
    (5, 'Bybit', 'bybit', 1, 'custodial', '#F7A600'),
    (6, 'MEXC', 'mexc', 1, 'custodial', '#1972E2'),
    (7, 'Bitget', 'bitget', 1, 'custodial', '#00F0FF'),
    (8, 'Gate', 'gate', 1, 'custodial', '#005596'),
    (9, 'KuCoin', 'kucoin', 1, 'custodial', '#01BC8D'),
    (10, 'Crypto.com', 'crypto-com', 1, 'custodial', '#002D74'),
    (11, 'Revolut', 'revolut', 3, 'custodial', '#191C1F'),
    (12, 'Upbit', 'upbit', 1, 'custodial', '#0062DF'),
    (13, 'HTX', 'htx', 1, 'custodial', '#0091D4'),
    (14, 'Gemini', 'gemini', 1, 'custodial', '#00DCFA'),
    (15, 'Bitfinex', 'bitfinex', 1, 'custodial', '#03CA9B'),
    (16, 'Robinhood', 'robinhood', 3, 'custodial', '#00C805');
