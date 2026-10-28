-- Lookup tables for exchange and wallet providers.
--
-- Seed names are well-known platforms that appear in current exchange volume
-- rankings and wallet reviews (CoinGecko, CoinMarketCap, and major wallet
-- comparisons). IDs are explicit so future tables can reference stable rows.

CREATE TABLE IF NOT EXISTS exchanges_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

INSERT INTO exchanges_types (id, name) VALUES
    (1, 'Binance'),
    (2, 'Coinbase'),
    (3, 'Kraken'),
    (4, 'OKX'),
    (5, 'Bybit'),
    (6, 'MEXC'),
    (7, 'Bitget'),
    (8, 'Gate'),
    (9, 'KuCoin'),
    (10, 'Crypto.com'),
    (11, 'Revolut'),
    (12, 'Upbit'),
    (13, 'HTX'),
    (14, 'Gemini'),
    (15, 'Bitfinex'),
    (16, 'Robinhood');

CREATE TABLE IF NOT EXISTS wallets_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

INSERT INTO wallets_types (id, name) VALUES
    (1, 'Ledger'),
    (2, 'Trezor'),
    (3, 'MetaMask'),
    (4, 'Trust Wallet'),
    (5, 'Exodus'),
    (6, 'Phantom'),
    (7, 'Coinbase Wallet'),
    (8, 'SafePal'),
    (9, 'Tangem'),
    (10, 'Bitget Wallet'),
    (11, 'OKX Wallet'),
    (12, 'Base Wallet'),
    (13, 'Sparrow Wallet'),
    (14, 'Electrum'),
    (15, 'BlueWallet'),
    (16, 'Muun'),
    (17, 'Wasabi Wallet'),
    (18, 'Blockchain.com Wallet');
