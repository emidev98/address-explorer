-- Exchange provider catalog.
--
-- Provider names are unique case-insensitively and a slug is derived from the
-- name at creation, so CSV imports and external references stay stable. The
-- unique indexes are created before `exchanges_types_data.sql` runs, so the
-- seed rows must carry their slug explicitly.
CREATE TABLE IF NOT EXISTS exchanges_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL DEFAULT '',
    category_id INTEGER REFERENCES exchange_categories(id) ON DELETE SET NULL,
    custody_model TEXT
        CHECK (custody_model IS NULL OR custody_model IN ('self_custody', 'custodial')),
    brand_color TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_exchanges_types_slug
    ON exchanges_types(slug);
CREATE UNIQUE INDEX IF NOT EXISTS idx_exchanges_types_name_nocase
    ON exchanges_types(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_exchanges_types_category
    ON exchanges_types(category_id);
