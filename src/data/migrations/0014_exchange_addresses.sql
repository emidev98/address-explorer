-- Exchange-owned Bitcoin wallets for all sixteen providers imported by
-- this application (see `exchange_accounts` and `exchanges_types`).
--
-- Scope and verification (researched 2026-10-28):
--   * binance-por        Binance quarterly proof-of-reserves BTC subset, ingested
--                        by ChainQuery: https://chainquery.com/sources/binance-por
--   * binance-2022       Binance November 2022 transparency disclosure:
--                        https://www.binance.com/en/blog/community/2895840147147652626
--   * bitfinex-wallets   Bitfinex's own public wallet list:
--                        https://github.com/bitfinexcom/pub/blob/main/wallets.txt
--   * bybit-por          Bybit wallet ownership disclosure:
--                        https://www.bybit.com/en/help-center/article/Bybit-Wallet-Addresses-Ownership-Explained
--   * okx-por            OKX reserve-wallet disclosure and GraphSense OKX pack:
--                        https://www.okx.com/proof-of-reserves
--   * kucoin-por         KuCoin transparency wallet list (GraphSense pack):
--                        https://www.kucoin.com/blog/transparency-and-trust-a-detailed-list-of-kucoin-s-wallets
--   * crypto-com-2022    Crypto.com 'Transparency First' cold-wallet disclosure:
--                        https://crypto.com/en/company-news/transparency-first
--   * huobi-por          HTX/Huobi reserve-wallet disclosure (GraphSense pack):
--                        https://www.huobi.com/support/en-us/detail/24922606430831
--   * coincarp           CoinCarp exchange wallet list (hot/cold flags and balances):
--                        https://www.coincarp.com/exchange/<exchange>/wallets/
--   * chaininfo          GraphSense 'Cold Wallets Tagpack' from chain.info richlist:
--                        https://github.com/graphsense/graphsense-tagpacks
--   * graphsense         GraphSense public exchange tag packs:
--                        https://github.com/graphsense/graphsense-tagpacks
--   * blockchain-com     Blockchain.com labelled addresses and Gemini hot-wallet labels:
--                        https://www.blockchain.com/explorer
--   * chainquery         ChainQuery aggregated labels, including self-disclosed Upbit
--                        cold wallet and Robinhood cold-wallet label:
--                        https://chainquery.com/address/<address>
--   * publicaml          PublicAML exchange/cluster attribution:
--                        https://publicaml.org/address/bitcoin/<address>/
--   * whale-alert        Whale Alert exchange labels on large BTC transfers:
--                        https://whale-alert.io/
--   * bitinfocharts      BitInfoCharts wallet labels:
--                        https://bitinfocharts.com/
--   * blocksec           Blocksec/Phalcon compliance labels:
--                        https://blocksec.com/phalcon/compliance
--   * chainface          ChainFace labelled transfers:
--                        https://www.chainface.io/
--   * upbit-disclosure   Upbit's public confirmation that 'Mr. 100' is its cold wallet,
--                        mirrored by ChainQuery/OKLink/Arkham:
--                        https://chainquery.com/address/1Ay8vMC7R1UbyCCZRVULMV7iQpHSAbguJP
--   * binance-safu       Binance SAFU fund disclosure:
--                        https://academy.binance.com/en/glossary/secure-asset-fund-for-users
--
-- Customer-specific deposit addresses are intentionally excluded: they are
-- single-user addresses and are not exchange treasury/withdrawal wallets.
-- Every address below is mainnet and syntactically valid. Each row is backed
-- by a first-party disclosure or a multi-source label database; addresses
-- whose public attributions conflict (for example Gemini vs. OKX) are left
-- out until a first-party disclosure resolves them. The `source` column
-- records which evidence applies to each row.
--
-- Tagging: the exchange catalog id is resolved by name, so the rows do not
-- depend on hard-coded `exchanges_types` ids. `wallet_id` stays NULL, the
-- mempool statistics start at zero, and existing rows are preserved so a user
-- label or classification is never overwritten. The 0014 seed migration
-- refreshes the provider balance anchor for every exchange after running this
-- file; running the file manually only inserts the address rows.

WITH seed(exchange_name, address, description, source) AS (
    VALUES
    ('Binance', '12VuUfQHTGqWDvBzm8TBad1mZBm4hjGEzN', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '16ftSEQ4ctQFDtVZiUBusQUjRrGhM3JYwe', 'Binance exchange wallet', 'graphsense,walletexplorer'),
    ('Binance', '1BAuq7Vho2CEkVkUxbfU26LhwQjbCmWQkD', 'Binance SAFU fund', 'binance-safu,blockchain-com'),
    ('Binance', '1DEuKerATHWRfyB14s6QCt4LPkfCTx4zaW', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '1GW6D31XgptUqay1eJLuAyrLL6QwRqiUUb', 'Binance exchange wallet', 'coincarp'),
    ('Binance', '1KuPikhUYtHz3fmSQ2UvotpUuN672NuEcm', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '1NDyJtNTjmwk5xPNhjgAMu4HDHigtobu1s', 'Binance exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Binance', '1PJiGp2yDLvUgqeBsuZVCBADArNsk6XEiw', 'Binance cold wallet', 'arkham,binance-por,chainquery,publicaml'),
    ('Binance', '1PPWMe42cywnM61nVatkmiggwdRQYLiCyS', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '1Pzaqw98PeRfyHypfqyEgg5yycJRsENrE7', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp,publicaml'),
    ('Binance', '31jUs8yQNVc9p6YuUPctWCkdhH7u8UiUba', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '32BgTv3NSYbMsBTwDbNNN2GKZPCTJSkqDv', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '32KqbtrRVxC6GLUJgJhVQtFTaCdq4GrgBb', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '32MHMmESiEYaBeZrCbpxMu6UF7LKDLdBPX', 'Binance exchange wallet', 'graphsense,richest_addresses'),
    ('Binance', '32bhzEniykYRFADVaRM5PYswsjC23cxtes', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '34HpHYiyQwg69gFmCq2BGHjF1DZnZnBeBP', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp'),
    ('Binance', '34xp4vRoCGJym3xR7yCVPFHoCNxv4Twseo', 'Binance cold wallet', 'arkham,binance-por,blockchain-com,chaininfo,chainquery,coincarp,exchange-wallets-binance,graphsense,publicaml,richest_addresses'),
    ('Binance', '36oiSkAi1VVuUpfdv8E2V5fZ2EarHRJpis', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '36zSLdRv1jyewjaC12fqK5fptn7PqewunL', 'Binance cold wallet', 'coincarp'),
    ('Binance', '378GLcve92X2Q4UrCyoMFDL5k5QrzeG7JN', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '386eAUqL3ZNZPmHeABXLo658DTQuJeLzUR', 'Binance cold wallet', 'chaininfo,graphsense'),
    ('Binance', '38DN2uFMZPiHLHJigfv4kWC9JWJrNnhLcn', 'Binance cold wallet', 'coincarp'),
    ('Binance', '395vnFScKQ1ay695C6v7gf89UzoFpx3WuJ', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '39884E3j6KZj82FK4vcCrkUvWYL5MQaS3v', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp'),
    ('Binance', '3AQ8bAh88TQU7JV1H3ovXrwsuV6s3zYZuN', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3AeUiDpPPUrUBS377584sFCpx8KLfpX9Ry', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3AtnehKDkFPC1bKvdrEVPSRGCtxQH8F1R8', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp'),
    ('Binance', '3AweAnU1qYSUCJ5Hvy9DFEB7dVqUebZw5i', 'Binance cold wallet', 'coincarp'),
    ('Binance', '3BzYxZXkdpkrFTWzKkWGRoWXJMuN2Ko621', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3CySuFKbBS29M7rE5iJakZRNqb3msMeFoN', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp'),
    ('Binance', '3E97AjYaCq9QYnfFMtBCYiCEsN956Rvpj2', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3EVVc8e2rxwUuERtdJCduWig8DnpsUqyA6', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3F1ZmTDAv6q3xrnT3tpm55DkCDzxhstU2Z', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3F9CGMu7JSJnMHA8jFM2KgxuH6hhxtvENP', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3FHNBLobJnbCTFTVakh5TXmEneyf5PT61B', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,coincarp'),
    ('Binance', '3FJ9hQuRtzy7pCqKHjeBGRDcY3fJbhHYXD', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3HdGoUTbcztBnS7UzY4vSPYhwr424CiWAA', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3HkPQSi1xso1Zh1EeqLjkMpura63fr5HRn', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3HkgC2R5PhqyXy6RVFyemvxN1VuFbQiQ5V', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3JFJPpH8Chwo7CDbyYQ4XcfgcjEP1FGRMJ', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3JqPhvKkAPcFB3oLELBT7z2tQdjpnxuDi9', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3KRduELtTmxBtqYpZviPysXoJDxdtdxFLZ', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3Kyo49ThRZPMukuYqPgqPejYeUpHvkA1EM', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3LQUu4v9z6KNch71j7kbj8GPeAGUo1FW6a', 'Binance cold wallet', 'arkham,binance-por,chainquery,coincarp'),
    ('Binance', '3LYJfcfHPXYJreMsASk2jkn69LWEYKzexb', 'Binance BTCB reserve', 'binance-2022,bitinfocharts,blockchain-com,chainquery,coincarp,exchange-wallets-binance,graphsense'),
    ('Binance', '3LcgLHzTvjLKBixBvkKGiadtiw2GBSKKqH', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3M219KR5vEneNb47ewrPfWyb5jQ2DjxRP6', 'Binance cold wallet', 'arkham,binance-por,blockchain-com,chaininfo,chainquery,coincarp,exchange-wallets-binance,graphsense,publicaml'),
    ('Binance', '3M3EtJGx5Dy9nCATLDhyRCrKGc38QC9z2e', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3M6UcBNGZAW1HRjiFDMRcY5aXFrQ4F9E1y', 'Binance cold wallet', 'chaininfo,graphsense'),
    ('Binance', '3NPL82eaehTFh4r3StpHqVQBTnZJFaGsyy', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3NXCvmLGz9SxYi6TnjbBQfQMcwiZ1iQETa', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', '3NjHh71XgjikBoTNYdWgXiNeZcLaKNThgb', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3Nm3c4gqVmvCmeP1sYvDchbb1r4QMu6emn', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3PXBET2GrTwCamkeDzKCx8DeGDyrbuGKoc', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3Q6ZCgJ3e4shasccLHen1VNUpReZTsjmsR', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3QK5vQ9hucSg8ZC8Vizq83qEWeHFLAWMud', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', '3QjvmQSvufx54RDAoZdXzkj2WZG7xdvv3i', 'Binance exchange wallet', 'graphsense,richest_addresses'),
    ('Binance', 'bc1p7x4aaws8t8cmmccu39kun6cajglx6ntuta7lhyxjt2cwrw4k89zqn0wley', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1q32lyrhp9zpww22phqjwwmelta0c8a5q990ghs6', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1q5n5dy7jgqhmpnmx44d3780qefzaphjreds0z9y', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', 'bc1q5qvs2dzzydqt4ygfn0k0ertjnv8ctytgcdz7l0', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1q5rsc4uscnmemlwru8xsys26k3xgxewqfnf3k7j', 'Binance proof-of-reserves wallet', 'binance-por,chainquery,publicaml'),
    ('Binance', 'bc1q78ufzeu8w8fwvxuphrdlg446xhyptf28fkatu5', 'Binance proof-of-reserves wallet (Ceffu custody)', 'binance-por,chainquery,publicaml'),
    ('Binance', 'bc1q7t9fxfaakmtk8pj7tdxjvwsng6y9x76czuaf5h', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1qdtmav38lca8yu3rrcknnqx5242cckgxqws7m72', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1ql42rmpvvq488tkqxvg8wmaa7j3jsrkxgnm8cy6', 'Binance exchange wallet', 'coincarp,graphsense,walletexplorer'),
    ('Binance', 'bc1qlupspe4hp6sxef2fp7n0w9y4x447ysclk4xg30', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Binance', 'bc1qm34lsc65zpw79lxes69zkqmk6ee3ewf0j77s3h', 'Binance hot wallet (customer withdrawals)', 'arkham,binance-por,blockchain-com,chainquery,coincarp,exchange-wallets-binance,graphsense,publicaml'),
    ('Binance', 'bc1qq393eyjh8hdjvcchy0mxquhh9wx8h3kzlkchfs', 'Binance proof-of-reserves wallet (Ceffu custody)', 'binance-por,chainquery,publicaml'),
    ('Binance', 'bc1qquax5zzensg9mkejdgq3q3h76v34cug96zk0f6', 'Binance proof-of-reserves wallet', 'binance-por,chainquery'),
    ('Bitfinex', '1129A9dFqy4ABDsGQ8RGusbMWYBoZc4myc', 'Bitfinex exchange wallet', 'graphsense,walletexplorer'),
    ('Bitfinex', '16TTYQEwSM5krF5MkQx6d1ePaHk4DPeq7a', 'Bitfinex exchange wallet', 'graphsense,walletexplorer'),
    ('Bitfinex', '1HPM1sNGef6Hq3YYd2hYQkpnRhQkkaWGVK', 'Bitfinex exchange wallet', 'graphsense,walletexplorer'),
    ('Bitfinex', '1Kr6QSydW9bFQG1mXiPNNu6WpJGmUa9i1g', 'Bitfinex hot wallet', 'bitfinex-wallets,coincarp,exchange-wallets-bitfinexcom,graphsense'),
    ('Bitfinex', '38nvB8P9QXjyLNva8CQ2AiAR35Sq1qaf2a', 'Bitfinex cold wallet', 'chaininfo,graphsense'),
    ('Bitfinex', '3D2oetdNuZUqQHPJmcMDDHYoqkyNVsFk9r', 'Bitfinex exchange wallet', 'chaininfo,coincarp,graphsense,richest_addresses'),
    ('Bitfinex', '3JZq4atUahhuA9rLhXLMhhTo133J9rF97j', 'Bitfinex exchange wallet', 'bitfinex-wallets,coincarp,exchange-wallets-bitfinexcom,graphsense,richest_addresses'),
    ('Bitfinex', 'bc1qgdjqv0av3q56jvd82tkdjpy7gdp9ut8tlqmgrpmv24sq90ecnvqqjwvw97', 'Bitfinex exchange wallet', 'bitfinex-wallets,blockchain-com,coincarp,exchange-wallets-bitfinexcom,graphsense'),
    ('Bitget', '115md8GZ5S8CmvUj8a8cW58YcKvv2SekYf', 'Bitget wallet 1', 'coincarp'),
    ('Bitget', '1FWQiwK27EnGXb6BiBMRLJvunJQZZPMcGd', 'Bitget exchange wallet', 'coincarp'),
    ('Bitget', '1GDn5X4R5vjdSvFPrq1MJubXFkMHVcFJZv', 'Bitget wallet 3', 'coincarp'),
    ('Bitget', '3GbdoiTnQrJYatcr2nhq7MYASSCWEKmN6L', 'Bitget exchange wallet', 'coincarp'),
    ('Bitget', '3HcSp9sR23w6MxeRrLqqyDzLqThtSMaypQ', 'Bitget exchange wallet', 'coincarp'),
    ('Bitget', '3MdofQ2ouxom9MzC9kKazGUShoL5R3cVLG', 'Bitget exchange wallet', 'coincarp'),
    ('Bybit', '1GrwDkr33gT6LuumniYjKEGjTLhsL5kmqC', 'Bybit exchange wallet', 'coincarp,exchange-wallets-bybit,graphsense'),
    ('Bybit', 'bc1q2qqqt87kh33s0er58akh7v9cwjgd83z5smh9rp', 'Bybit exchange wallet', 'coincarp,exchange-wallets-bybit,graphsense'),
    ('Bybit', 'bc1q9w4g79ndel72lygvwtqzem67z6uqv4yncvqjz3yn8my9swnwflxsutg4cx', 'Bybit exchange wallet', 'coincarp,exchange-wallets-bybit,graphsense'),
    ('Bybit', 'bc1qjysjfd9t9aspttpjqzv68k0ydpe7pvyd5vlyn37868473lell5tqkz456m', 'Bybit exchange wallet', 'coincarp,exchange-wallets-bybit,graphsense'),
    ('Bybit', 'bc1qs5vdqkusz4v7qac8ynx0vt9jrekwuupx2fl5udp9jql3sr03z3gsr2mf0f', 'Bybit cold wallet', 'bybit-por'),
    ('Coinbase', '12BZD75sbMmdj5dy1d5cS2Lo4YWs1FDbDn', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12Gjyd3MMR7Dj2KwCxw71wwzZXVp2xy8nK', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12US3aeNibsxe7Lhascm4AmvGiTxwSdD4X', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12mJZGPCWf5C2ap2dUdJiuGr8eWAvi6E5M', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12n3s8MCqdZzPnTisYrXagbfw8pJg8y9BW', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12nxRsisUcJqCDeUpTYH2a7F5jcad3YhFG', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '12yePmcVhMzzHYwh4BpVF3Cm1bYL9mqyKc', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '134ooU6EQmCFFzEPN7yga3S1XikcBtynco', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '13jh1fCa6pCFTcCH1H6MXDWLa9zajtniMT', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '13nwifHUz5ZfHuQhk5ETJ4BhmqbuQdvTFp', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '13pTeUzSU4nwkM72hM6e8AcP2d8LudZBjc', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '14Ai5GcasUdr5hR2GMzeojkzB9cm4oufHt', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '14KzHoS5dXbhy2kBevNKLz2ZMtjaqHkKWZ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '14YN9Zsx4H8DXUoK2XfRvjuanfjK1RrkHz', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '14zukxqLuLXg5kqZYRB1Q3Audec3NtxJPW', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '157ZXCYfK6NqBiFLkoLDt3PN8KyE8xdTu5', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '15QFkJUWFZ6QRpTrzUitH2Egyf1JTngVgV', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '15URsTiy1ksoMMV7DuEi9hvSqHgqobAtKa', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '15ZQJagAa2iUCwpQXUUCZ4BfzFW5TAVyJj', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '15mxk4fKk9ggQ4fikNWKpasahsdXZNdVu2', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '16Azr3MAzMKMPmxZXfRsBbBPYHnp2CuJNJ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '16BBBjvAArL3zdb1Fh1isr2w2hX7K1K4Gm', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '16BiUbH8yxFmCJs4ArtyTgHKfXQFvkvPNr', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '16H2Fe9MgxZ1jXW3TaXbgWh3SJfwyxQUTp', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '16dyizLarM2N4UjGaEFPESNmJC5vEALtZX', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '178KR5zk2Z2rc6rWdczpHBAokBDkjoC8jC', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17GAbQ5DSqnEhyrMMLd9yaraAmVyvoZJ3Z', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17GyU83L63BtxtpDgv8LmhhtmvcrXbfVt1', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17J26KSDtNFPAUhCMwjcRNWhneMsXS75Ba', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17RmGC3i2dASQai3mP9iJLX3ZD9TBm2YQV', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17SAATrqavNbzmqBwqxzZc7rK6u9Rmi9hE', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '17YyZSNFt31pzGXfZtrzs7Y5Nd56rG2uU5', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '17rVQwoL5v18KfoQJNMMoMBm3rDjodxw1m', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '182p5CrqfDxvonznRyi5AUBwsCkDX8eYMe', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '18Ayw3caz2xGDhTfz1nJxLw1NURPtBNJnn', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '18oRfGoPpb6bkQdACnD6dVYWceeWURK5R6', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '18saWjMCchDTGWunefuDJSWNdz4p6KtfBL', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '19F4xZNBxtPAdMqFudr2LDH5efeeUYF21x', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '19Te6hzGFSbryomVYqzG2kpBmAJYykx5Yv', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '19om4Guv9QmVesS7AewMUyE5JUJ9H7xwNN', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1AFNceZVB2MshqU5AEoxg14R7LrUx6Un6C', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1AMDVupuTEeTSnKi6LoSFBqikQybufZsv1', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1BMi633XQ8jt45Tb5EvZ4xdZHb2GzxKSfb', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1BbSZNNhoBUCFGMfnscBDXf7PH1takoMga', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1BvdLm9RcFWncKpJYr7YjFttYJ9zLf7We', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1C1bbDApniTd7DtUodpr3ayXxVtcHvwWgx', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1C5TB2QzeDDJUE4EQD17NmSyEXTk34huRo', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1C7u4Zqu6ZZRsiKsFMYVDvNLfCwsGrbeTq', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Cb1G5qFK91fShyaPPZWVFwYFBtqRG7h8D', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1Cus9pZdkckCz23bDAsKMY6aLSQ7CmuHyw', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1D3AHKaQbtPLVwYB1L9ATAPXo14xVA2Hjc', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1DTmjcZo3MMH9MscHizTLjrUxnvwvSLdf7', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1DdxS8CheskxKAPPbG8Uq4Qts8kS4x1tt2', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Djs2VyBVr6MYNcGVaHAr8B3N1mViS5yoo', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Dr1pZDgpowHDGJrUNTithDL33TdriFMzE', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Dt4Q2ofKUtHwvjN45tAUfikNBfJrDERcZ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1E1SKgxgYJFXzaLxgat2FNSnwWeKz1U15N', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1EDRfeNkjkH2SAhSbEKzhKuabnEbVWbKEp', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1Enhkd9LkQV56a9M12P4VuMDkjyTeLJy5m', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1Eqj1APg1dkcPH7CmHJ6SzRwtJRLpxtBFA', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FDzP5vQGMhCzeb9nH4AcUDnUbcHS22BMZ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FMbcnYvvccZ6hR324cFRpn1QX9TCkqtAe', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FUBESNxB2JkyXPc4o9wwoGt158DC9A8dj', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1Fjom9b9wvp3HPi2ZS9hqXQwJpXcjumSYJ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Fn4FSiHFr9SgbQ3ccsybN9f3RF1omst4x', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Fp9KLM6cs35y8QvyqAiL714WMFSLBJDLC', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FrfmxAhYRNU9ebotDHP9FqLbe54QZVcfi', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FtHBKrYkDchMA1pwRTpYZ77TpfwSgh6iF', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1FzWLkAahHooV3kzTgyx6qsswXJ6sCXkSR', 'Coinbase exchange wallet', 'chainquery,coincarp'),
    ('Coinbase', '1GBWcQuY5aD6e2zrbdb38j7pizVr2wcJnC', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Gjkd1hwrJxM9h5Sj1W5bfEN6km1qkVCg4', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Gtpfqoo8stQw2E4vCbZZbJBBZvYzzCeRy', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1HG6NbWmJjjSE1Z6oPetWG3nnKrQxgBN3a', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1HGdi2AtuvBUcdTFe62sKd7ZAFzoG7iQLb', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Hyh53PULY6Jyq5LPAJ4CHjgzEbVaqy7KU', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1J2zKTQnSrLJMXZTnA3a5ky1W2L3PJhagC', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1JYbzYitYQ1ZWTx5KEx7WH2AejC5i5UUbF', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1JaUiAyoRXr4kqxm3ZQP5uFZmF57bkqXbu', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Jc7vYQHCnmdyhbpcv5o3pYtUbH6Sk139z', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1JeTiSWo8pYWcNaTH4c3CfheAwVDYRoy1Q', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1JpAvQxbxurhU8wfxszCtzFey4D6N8goJv', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1JpiTWauQdtysbynNp88dWeuyg2gBbKDcT', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1KLay1g7rZxAiotuDhGy5YizdXSpqkFxQ9', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1KeK2uTAe8hDVKTbpyDDzd7qfRZ8z3LJx', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1KoiNofDvcUrrVfxgqpRZixWXNXo6d2kfQ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Kphp4WNPLpScp5T5f63jnVS9ohK6a86g7', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1LRXrhZRLVTjGCPFphPsXZfeHesRigm59r', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Lan1sQzYGViDZskUFmnitJvfejn2jYdNS', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1LbUMzM7tmyVJCpzBJTi5ZVmRb2Vn8mwVH', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Le1X8LJb4VkFsfktiUY7V7wMRhniu5Xgn', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1LyTftu54VMYCv5pq3S4pMzPRMnsYKTESw', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1M9pAdfhGHtQkhGRijApWAkkrPCduvV6Zi', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1MLsZmbT3mgmm6zLkTR26yJHVfn8BSdqqd', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1MVo3EXLakJym29CFK6o1MyaCBMdvcmmrL', 'Coinbase cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Coinbase', '1MYv4C4hZ7hC5sbHrPkzvmNoozQgnHKeAU', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1MqAfNMgbZoKtRinT89q1faSZqTKTqCFhR', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1N4yKFsFPjjoJyo3BAKUWpcs3SUghfgJkH', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1NJdKoBkKQNg6aYL9HTKyQ5SgfjZQDz3y3', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1NNGdZMYsN9pgLSXZ5dD5KFUbatFynpmvY', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1NQWCZqRYvANz3iX1Wnczpm277VEYvcAee', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1PL2cmmMLmGGDtqaSZJY8DR1iKJaziEPJv', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1Q7cu7WkeDurYgffeEc9CEnA6zLohbh9iQ', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1T7m8nSs5Yh3YuLNoHPb5ehwN3LRVEchn', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1gvH7pGPrEBNjqmwYS8UDhjFQkyqkKCLE', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '1q9kwzJggw6AbQjzJeYQFYK8D5gQK8sSh', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '3FdFFJRqvBWKEjFqkZhxuGbN9HNUVAfLqF', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Coinbase', '3FiMKffg2kY2Pi2Kebn2HZqN7m6kEcNUMk', 'Coinbase cold wallet', 'chaininfo,graphsense'),
    ('Crypto.com', '14m3sd9HCCFJW4LymahJCKMabAxTK4DAqW', 'Crypto.com cold wallet', 'coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense,publicaml'),
    ('Crypto.com', '35JBhBqhRU7DZYzUkxYKTyFbuBGZPAtJDY', 'Crypto.com exchange wallet', 'publicaml'),
    ('Crypto.com', '3LhhDLBVWBZChNQv8Dn4nDKFnCyojG1FqN', 'Crypto.com cold wallet', 'coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense'),
    ('Crypto.com', '3QsGsAXQ4rqRNvh5pEW55hf3F9PEyb7rVq', 'Crypto.com cold wallet', 'coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense,publicaml'),
    ('Crypto.com', 'bc1q4c8n5t00jmj8temxdgcc3t32nkg2wjwz24lywv', 'Crypto.com cold wallet', 'coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense,publicaml'),
    ('Crypto.com', 'bc1qpy4jwethqenp4r7hqls660wy8287vw0my32lmy', 'Crypto.com cold wallet', 'coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense'),
    ('Crypto.com', 'bc1qr4dl5wa7kl8yu792dceg9z5knl2gkn220lk7a9', 'Crypto.com cold wallet', 'blockchain-com,coincarp,crypto-com-2022,exchange-wallets-cryptocom,financemagnates,graphsense,publicaml'),
    ('Gate', '14kmvhQrWrNEHbrSKBySj4qHGjemDtS3SF', 'Gate.io wallet 5', 'coincarp'),
    ('Gate', '162bzZT2hJfv5Gm3ZmWfWfHJjCtMD6rHhw', 'Gate.io wallet 1', 'coincarp'),
    ('Gate', '1EkkGXR7dTbZbrKFKoe6YEP4gj4GzMeKvw', 'Gate exchange wallet', 'coincarp'),
    ('Gate', '1G47mSr3oANXMafVrR8UC4pzV7FEAzo3r9', 'Gate.io wallet 7', 'coincarp'),
    ('Gate', '1HpED69tpKSaEaWpY3Udt1DtcVcuCUoh2Y', 'Gate.io wallet 6', 'coincarp'),
    ('Gate', '3HroDXv8hmzKRtaSfBffRgedKpru8fgy6M', 'Gate cold wallet', 'chaininfo,coincarp,graphsense'),
    ('Gemini', '1GPRueEBA6x3VkGse6F2Z9Sq3ZzvHvFUWL', 'Gemini hot wallet', 'blockchain-com,publicaml,whale-alert'),
    ('Gemini', '1J37CY8hcdUXQ1KfBhMCsUVafa8XjDsdCn', 'Gemini hot wallet', 'blocksec,publicaml'),
    ('Gemini', '35tFJwVKEexyDr3fZicy3YpfQvApndVJXG', 'Gemini exchange wallet', 'publicaml,whale-alert'),
    ('Gemini', '38AqGTwpzTkb7tiJCQASG2bXwaGLCkjpEN', 'Gemini exchange wallet', 'publicaml,whale-alert'),
    ('Gemini', 'bc1qjg9e23ntuy2k35cvsghecqzl2y0zenw97q3gjj', 'Gemini exchange wallet', 'publicaml,whale-alert'),
    ('Gemini', 'bc1qmtl499lclce4gvjcgn38gehmmkh2kekywkc273ghhmcfl2s93jzq3wt9w4', 'Gemini custody wallet', 'publicaml,whale-alert'),
    ('Gemini', 'bc1quq29mutxkgxmjfdr7ayj3zd9ad0ld5mrhh89l2', 'Gemini hot wallet', 'blockchain-com,publicaml,whale-alert'),
    ('Gemini', 'bc1qwcqplfhq97887w9l6ercvp7v0m0ce4my882y43', 'Gemini exchange wallet', 'publicaml,whale-alert'),
    ('HTX', '12qTdZHx6f77aQ74CPCZGSY47VaRwYjVD8', 'HTX exchange wallet', 'coincarp,exchange-wallets-huobi,graphsense'),
    ('HTX', '13oLr3JJm4qx7PrTGtorPcxZ5YXpYKnXmE', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '143gLvWYUojXaWZRrxquRKpVNTkhmr415B', 'HTX exchange wallet', 'coincarp,exchange-wallets-huobi,graphsense'),
    ('HTX', '14XKsv8tT6tt8P8mfDQZgNF8wtN5erNu5D', 'HTX exchange wallet', 'coincarp,exchange-wallets-huobi,graphsense'),
    ('HTX', '15BAjnnitb4UPrPG8iGpe3p21hwdP6wwU7', 'HTX exchange wallet', 'coincarp'),
    ('HTX', '16AhJPQwo5NgpcHbyQA2f7BqWVjePN6nAq', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1FggHs2692e8E26ECZMcxncWuY9VCX7HRo', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1H3nwrL8acJDhejt2mW7ianBrQtTw9LTvn', 'HTX exchange wallet', 'graphsense,walletexplorer'),
    ('HTX', '1J29zwZCRJafLCh3NHY7T11Sc2bLpDfKsq', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1Lt7etHUD5to8rSFW9USYARP6thzUfco7V', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1MW8QJahh2YXovXpbhDcXAN8MEEL2AUn4n', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1McYknG7PKnpKhdYuxUqAxFke43uKtpS2e', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1PFQM6D3CFrf7PFtZZAW9YHr55EPsfAsrG', 'HTX exchange wallet', 'graphsense,richest_addresses'),
    ('HTX', '1vdXJDiDSzeUAoM7eAcWZPhM9wEKmH8dG', 'HTX exchange wallet', 'graphsense,walletexplorer'),
    ('HTX', '35hK24tcLEWcgNA4JxpvbkNkoAcDGqQPsP', 'HTX exchange wallet', 'chaininfo,coincarp,graphsense,richest_addresses'),
    ('HTX', '3Cbq7aT1tY8kMxWLbitaG7yT6bPbKChq64', 'HTX exchange wallet', 'coincarp'),
    ('Kraken', '12fXB6sBJCGbmf7XhRvTXMDCu91329aven', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '12gr1kbYQU2M81QBUQ5uAdwA9jKT6oDwx5', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '12oMwT4TuDmPe28jJRAw7ox5Pp6t2CSe5H', 'Kraken exchange wallet', 'coincarp'),
    ('Kraken', '14eQD1QQb8QFVG8YFwGz7skyzsvBLWLwJS', 'Kraken exchange wallet', 'chaininfo,coincarp,graphsense,richest_addresses'),
    ('Kraken', '169zLUVW2hwqhfaxas9sFaMndRYq5CF6PM', 'Kraken exchange wallet', 'coincarp'),
    ('Kraken', '16iqmhUox1Vh8yjR71SyuYRfP8oRwXspW9', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '17CzhFvGwH6TT46JtzhnhMpTw4mHYpEGCR', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '17QVYyUMgWzZaTBHgXeA5hM5tnWHXtEmnQ', 'Kraken cold wallet', 'coincarp'),
    ('Kraken', '18EtP9azZCFZgjdNdTSurtNEWF8AEjj8Cs', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '18ZHDyqpcizhEf7GJCgL4XrToXAy38XvUk', 'Kraken exchange wallet', 'coincarp'),
    ('Kraken', '196pUYs6kGpwii2KjV138SXrJ2hwKLaMVr', 'Kraken exchange wallet', 'graphsense,walletexplorer'),
    ('Kraken', '19Wa69eDP1F6J2Segjsd8co7bVjdLFeL2n', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '1A7znRYE24Z6K8MCAKXLmEvuS5ixzvUrjH', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '1AnwDVbwsLBVwRfqN2x9Eo4YEJSPXo2cwG', 'Kraken exchange wallet', 'chaininfo,coincarp,graphsense,richest_addresses'),
    ('Kraken', '1D32tY21kqGhMnipxP8LLuX4g3o3QbcrK7', 'Kraken exchange wallet', 'coincarp'),
    ('Kraken', '1Ed6rraaUdKTSGXdkHUw2hVF75469dz77C', 'Kraken exchange wallet', 'coincarp'),
    ('Kraken', '1Kd6zLb9iAjcrgq8HzWnoWNVLYYWjp3swA', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '1PNJfc9MzeBGV1UAjegqhFJb3erejqKHCi', 'Kraken exchange wallet', 'coincarp,graphsense,richest_addresses'),
    ('Kraken', '3G3VG4X1WquWjqXT27JRwrRoyZgdyWf1dT', 'Kraken exchange wallet', 'graphsense,walletexplorer'),
    ('KuCoin', '1AozLV7krw87WKxjCzzygM29BrYFxbxPwh', 'KuCoin exchange wallet', 'coincarp'),
    ('KuCoin', '1HDu2mDMr5bcgoGBty9Usmb8oH1rTDJxQ9', 'KuCoin exchange wallet', 'coincarp'),
    ('KuCoin', '38fJPq4dYGPoJizEUGCL9yWkqg73cJmC2n', 'KuCoin exchange wallet', 'coincarp,exchange-wallets-kucoin,graphsense'),
    ('KuCoin', '3PttW2xqWU3BUeRkeUpK1snvpuyb8mKkP9', 'KuCoin exchange wallet', 'coincarp'),
    ('KuCoin', 'bc1q080rkmk3kj86pxvf5nkxecdrw6nrx3zzy9xl7q', 'KuCoin exchange wallet', 'coincarp,exchange-wallets-kucoin,graphsense'),
    ('KuCoin', 'bc1q8yja3gw33ngd8aunmfr4hj820adc9nlsv0syvz', 'KuCoin exchange wallet', 'coincarp,exchange-wallets-kucoin,graphsense'),
    ('KuCoin', 'bc1qgrxsrmrhsapvh9addyx6sh8j4rw0sn9xtur9uq', 'KuCoin exchange wallet', 'coincarp,exchange-wallets-kucoin,graphsense'),
    ('MEXC', '13uZyaPbt4rTwYQ8xWFySVUzWH3pk2P5c7', 'MEXC exchange wallet', 'coincarp'),
    ('MEXC', '1LPfGH6HZD4koJDMHPD6m8WL7TCiKgHkjD', 'MEXC cold wallet 2', 'coincarp'),
    ('MEXC', 'bc1qrlsks7xe755m05zfreyjdxn34rypc4lnghtcwc', 'MEXC exchange wallet', 'coincarp'),
    ('OKX', '13jTtHxBPFwZkaCdm6BwJMMJkqvTpBZccw', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '13rCGm4Z3PDeYwo5a7GTT4jFYnRFBZbKr1', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '14kHu26yWkVD8qAnBfcFXHXxgquNoSpKum', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '15Exz1BAVan4Eweagy1rcPJnfyc6KJ4GvL', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '162z6mSSHzfTqb2Sn3NUk5r1Y2oGoCMCoM', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '16rF2zwSJ9goQ9fZfYoti5LsUqqegb5RnA', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '178E8tYZ5WJ6PpADdpmmZd67Se7uPhJCLX', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '18QUDxjDZAqAJorr4jkSEWHUDGLBF9uRCc', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1AumBaQDRaCC3cKKQVRHeyvoSPWNdDzsKP', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1BsdDaJtgFZrLfzEXvh6cD4VhtHHSHhMea', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1CE8chGD6Nu8qjcDF2uR1wMKyoWb8Kyxwz', 'OKX reserve wallet', 'exchange-wallets-okx,graphsense'),
    ('OKX', '1CY7fykRLWXeSbKB885Kr4KjQxmDdvW923', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1DVTB9YKi4KNjyEbAHPp17T8R1Pp17nSmA', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1DcT5Wij5tfb3oVViF8mA8p4WrG98ahZPT', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1DnHx95d2t5URq2SYvVk6kxGryvTEbTnTs', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1FTgXfXZRxMQcKSNeuFvWYVPsNgurTJ7BZ', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1FY6RL8Ju9b6CGsHTK68yYEcnzUasufyCe', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1FfgXrQUjX5nQ4zsiLBWjvFwW61jQHCqn', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1JQULE6yHr9UaitLr4wahTwJN7DaMX7W1Z', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1Lj2mCPJYbbC2X6oYwV6sXnE8CZ4heK5UD', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1LnoZawVFFQihU8d8ntxLMpYheZUfyeVAK', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1M6E6vPaYsuCb34mDNS2aepu2aJyL6xBG4', 'OKX exchange wallet', 'blockchain-com,exchange-wallets-okx,graphsense'),
    ('OKX', '1MbNM3jwxMjRzeA9xyHbMyePN68MY4Jxb', 'OKX reserve wallet', 'exchange-wallets-okx,graphsense'),
    ('Revolut', 'bc1q6h274e67rdr7xpn2mr2ttnssjzx3rhlkug6m5q', 'Revolut exchange wallet 3', 'publicaml'),
    ('Revolut', 'bc1qhlwghgj8wpyjcufancj5z4r0454zvlls92u9ed', 'Revolut exchange wallet 2', 'publicaml'),
    ('Revolut', 'bc1qkylhvveqtqlcry43jekhgknnhae2jmdt225l3e', 'Revolut exchange wallet 4', 'publicaml'),
    ('Revolut', 'bc1qs4z2d3h5je080f74tax92dwg08sf3hylj9vfg3', 'Revolut cold wallet (Coinbase Prime Custody)', 'arkham,revolut-safeguarding'),
    ('Revolut', 'bc1qtdt6lzutl4nyyf8t2u2jqkp8ellz9p6670gy2n', 'Revolut exchange wallet 1', 'publicaml'),
    ('Robinhood', '1opFYNKYG21CfzMYgCZBDoeBpQ9vhVUC9', 'Robinhood exchange wallet', 'publicaml'),
    ('Robinhood', 'bc1q4vxcxw7mpg9dcryqu0kav8awrn7qk5e6wgs3hg', 'Robinhood hot wallet', 'chainface,publicaml'),
    ('Robinhood', 'bc1ql49ydapnjafl5t2cp9zqpjwe6pdgmxy98859v2', 'Robinhood cold wallet', 'chainquery,publicaml,whale-alert'),
    ('Robinhood', 'bc1qmxcagqze2n4hr5rwflyfu35q90y22raxdgcp4p', 'Robinhood hot wallet', 'bitinfocharts,publicaml,whale-alert'),
    ('Upbit', '1Ay8vMC7R1UbyCCZRVULMV7iQpHSAbguJP', 'Upbit cold wallet', 'arkham,chainquery,oklink,upbit-disclosure,whale-alert'),
    ('Upbit', '3BC5Audd4NHBeAp2tJXZdXAvRAtswniXNZ', 'Upbit exchange wallet', 'publicaml')
)
INSERT INTO addresses (
    address, description, exchange_id,
    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
    chain_spent_txo_sum, chain_tx_count,
    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
    mempool_spent_txo_sum, mempool_tx_count
)
SELECT
    seed.address,
    seed.description,
    exchange.id,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0
FROM seed
JOIN exchanges_types AS exchange ON exchange.name = seed.exchange_name
WHERE NOT EXISTS (
    SELECT 1 FROM addresses AS existing WHERE existing.address = seed.address
);
