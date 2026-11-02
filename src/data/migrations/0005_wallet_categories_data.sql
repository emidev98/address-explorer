-- Initial wallet category vocabulary. These rows are a starting point only:
-- they are fully user-owned after the migration.
INSERT OR IGNORE INTO wallet_categories (id, name) VALUES
    (1, 'Hardware wallet'),
    (2, 'Software wallet'),
    (3, 'Mobile wallet'),
    (4, 'Desktop wallet'),
    (5, 'Browser extension'),
    (6, 'Web wallet'),
    (7, 'Paper wallet'),
    (8, 'Multisig'),
    (9, 'Custodial');
