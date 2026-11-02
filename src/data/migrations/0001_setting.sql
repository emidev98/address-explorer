-- Key/value application settings, including the default fiat currency and the
-- theme selection.
CREATE TABLE IF NOT EXISTS setting (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
