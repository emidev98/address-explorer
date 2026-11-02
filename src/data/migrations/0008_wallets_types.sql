-- Wallet provider catalog.
--
-- Provider names are unique case-insensitively and a slug is derived from the
-- name at creation, so CSV imports and external references stay stable. The
-- unique indexes are created before `wallets_types_data.sql` runs, so the seed
-- rows must carry their slug explicitly.
CREATE TABLE IF NOT EXISTS wallets_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL DEFAULT '',
    category_id INTEGER REFERENCES wallet_categories(id) ON DELETE SET NULL,
    custody_model TEXT
        CHECK (custody_model IS NULL OR custody_model IN ('self_custody', 'custodial')),
    platform TEXT
        CHECK (platform IS NULL OR platform IN ('hardware', 'software')),
    brand_color TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_wallets_types_slug
    ON wallets_types(slug);
CREATE UNIQUE INDEX IF NOT EXISTS idx_wallets_types_name_nocase
    ON wallets_types(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_wallets_types_category
    ON wallets_types(category_id);
