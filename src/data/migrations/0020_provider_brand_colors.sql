-- Brand-color backfill for the seeded provider catalogs.
--
-- `0007_exchanges_types_data.sql` and `0009_wallets_types_data.sql` carry the
-- brand colors for databases created from scratch. A database created before
-- the colors were added keeps its existing provider rows, because
-- `INSERT OR IGNORE` never updates a row and the provider seed entries have
-- already run. This migration fills only the colors that are still unset, so a
-- user's own color is never overwritten.
--
-- The values mirror the seed files; update both when a provider's brand color
-- changes. Rows are matched by slug, which the catalogs keep stable.
UPDATE exchanges_types SET brand_color = '#F0B90B'
WHERE slug = 'binance' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#0052FF'
WHERE slug = 'coinbase' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#5741D9'
WHERE slug = 'kraken' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#000000'
WHERE slug = 'okx' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#F7A600'
WHERE slug = 'bybit' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#1972E2'
WHERE slug = 'mexc' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#00F0FF'
WHERE slug = 'bitget' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#005596'
WHERE slug = 'gate' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#01BC8D'
WHERE slug = 'kucoin' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#002D74'
WHERE slug = 'crypto-com' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#191C1F'
WHERE slug = 'revolut' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#0062DF'
WHERE slug = 'upbit' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#0091D4'
WHERE slug = 'htx' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#00DCFA'
WHERE slug = 'gemini' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#03CA9B'
WHERE slug = 'bitfinex' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE exchanges_types SET brand_color = '#00C805'
WHERE slug = 'robinhood' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#000000'
WHERE slug = 'ledger' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#60E198'
WHERE slug = 'trezor' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#F6851B'
WHERE slug = 'metamask' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#0500FF'
WHERE slug = 'trust-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#6619FF'
WHERE slug = 'exodus' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#AB9FF2'
WHERE slug = 'phantom' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#0052FF'
WHERE slug = 'coinbase-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#4A21EF'
WHERE slug = 'safepal' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#000000'
WHERE slug = 'tangem' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#00F0FF'
WHERE slug = 'bitget-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#000000'
WHERE slug = 'okx-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#0052FF'
WHERE slug = 'base-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#FFA400'
WHERE slug = 'sparrow-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#1382EF'
WHERE slug = 'electrum' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#3F78DC'
WHERE slug = 'bluewallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#3B7DFF'
WHERE slug = 'muun' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#A2FF00'
WHERE slug = 'wasabi-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');

UPDATE wallets_types SET brand_color = '#0C6CF2'
WHERE slug = 'blockchain-com-wallet' AND (brand_color IS NULL OR trim(brand_color) = '');
