-- Seed rows of the wallet provider catalog.
--
-- Names are well-known wallets from current wallet comparisons. IDs are
-- explicit so other tables keep stable references, and every row gets its
-- initial slug, category, custody model, platform, and brand color here. The
-- seed rows are fully user-owned after this migration: future migrations MUST
-- NOT re-seed providers by name.
--
-- `brand_color` is the provider's primary published brand color taken from its
-- official brand guidelines or press kit (researched 2026-11). It only seeds a
-- badge background; users may change it, and the UI derives a fallback color
-- when a row has none. Keep the values in sync with
-- `0020_provider_brand_colors.sql`, which backfills databases seeded before the
-- colors were added.
INSERT OR IGNORE INTO wallets_types
    (id, name, slug, category_id, custody_model, platform, brand_color)
VALUES
    (1, 'Ledger', 'ledger', 1, 'self_custody', 'hardware', '#000000'),
    (2, 'Trezor', 'trezor', 1, 'self_custody', 'hardware', '#60E198'),
    (3, 'MetaMask', 'metamask', 5, 'self_custody', 'software', '#F6851B'),
    (4, 'Trust Wallet', 'trust-wallet', 3, 'self_custody', 'software', '#0500FF'),
    (5, 'Exodus', 'exodus', 4, 'self_custody', 'software', '#6619FF'),
    (6, 'Phantom', 'phantom', 5, 'self_custody', 'software', '#AB9FF2'),
    (7, 'Coinbase Wallet', 'coinbase-wallet', 3, 'self_custody', 'software', '#0052FF'),
    (8, 'SafePal', 'safepal', 1, 'self_custody', 'hardware', '#4A21EF'),
    (9, 'Tangem', 'tangem', 1, 'self_custody', 'hardware', '#000000'),
    (10, 'Bitget Wallet', 'bitget-wallet', 3, 'self_custody', 'software', '#00F0FF'),
    (11, 'OKX Wallet', 'okx-wallet', 5, 'self_custody', 'software', '#000000'),
    (12, 'Base Wallet', 'base-wallet', 3, 'self_custody', 'software', '#0052FF'),
    (13, 'Sparrow Wallet', 'sparrow-wallet', 4, 'self_custody', 'software', '#FFA400'),
    (14, 'Electrum', 'electrum', 4, 'self_custody', 'software', '#1382EF'),
    (15, 'BlueWallet', 'bluewallet', 3, 'self_custody', 'software', '#3F78DC'),
    (16, 'Muun', 'muun', 3, 'self_custody', 'software', '#3B7DFF'),
    (17, 'Wasabi Wallet', 'wasabi-wallet', 4, 'self_custody', 'software', '#A2FF00'),
    (18, 'Blockchain.com Wallet', 'blockchain-com-wallet', 6, 'self_custody', 'software', '#0C6CF2');
