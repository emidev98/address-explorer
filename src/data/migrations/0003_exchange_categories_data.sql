-- Initial exchange category vocabulary. These rows are a starting point only:
-- they are fully user-owned after the migration.
INSERT OR IGNORE INTO exchange_categories (id, name) VALUES
    (1, 'Centralized exchange'),
    (2, 'Decentralized exchange'),
    (3, 'Broker'),
    (4, 'P2P marketplace'),
    (5, 'OTC desk'),
    (6, 'Custodian'),
    (7, 'Payment processor'),
    (8, 'Bank');
