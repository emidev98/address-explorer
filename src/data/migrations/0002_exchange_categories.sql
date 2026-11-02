-- Categories of the exchange provider catalog. The rows are named by
-- `exchange_categories_data.sql`; categories stay user-editable afterwards.
CREATE TABLE IF NOT EXISTS exchange_categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE
);
