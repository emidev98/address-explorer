-- Categories of the wallet provider catalog. The rows are named by
-- `wallet_categories_data.sql`; categories stay user-editable afterwards.
CREATE TABLE IF NOT EXISTS wallet_categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE
);
