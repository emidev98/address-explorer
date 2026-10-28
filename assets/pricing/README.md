# Historical Pricing Data

For the seek of simplicity and avoiding to pay an API for historical prices I decided to use the datasets provided by [Novandra Anugrah](https://www.kaggle.com/datasets/novandraanugrah/bitcoin-historical-datasets-2018-2024/data) and [SRK](https://www.kaggle.com/datasets/sudalairajkumar/cryptocurrencypricehistory?select=coin_Bitcoin.csv) in Kaggle. As well this is important due to allowing the system to work offline.

The Kaggle exports live in `assets/pricing/raw_data/` and are git-ignored because they can always be downloaded again. `pricing_pack` reads **every** `.csv` in that directory, so a new export only has to be dropped there; the file names are not hard-coded.

Since I also want to avoid creating an API, the data is compressed in the following format [YYYYMMDDHHmmss](https://day.js.org/docs/en/display/format) + `CENTS,` one after the other `201801010101011328427,201801020101011418176,201801030101011472878...` to decrease the size of the `btc_usd` dataset. Each record is:

- **timestamp**: 14 ASCII digits, the UTC close time (`20180101010101` = 2018-01-01 01:01:01);
- **cents**: the close price in USD cents as a decimal integer (`1328427` = $13,284.27);
- terminated by a comma, including the last record.

## Price precision

Before writing the packed file, every close price is **trimmed to two decimals**: the value is converted to integer USD cents and any further fractional digits in the Kaggle export are discarded, not rounded. For example, `144.5399932861328` becomes `14453` cents ($144.53), `1.999` becomes `199` cents ($1.99), and `0.009` becomes `0` cents. Consequently, the packed file never stores more than two decimal places, and the application seeds `historical_prices.close` with the same two-decimal value.

## Packed file

The packed file is `btc_usd.bin`. The packer combines all the source exports, sorts the records by close time, and keeps the first occurrence of each close time. Because the files are read from the highest-resolution export to the coarsest (15m, then 1h, 4h, and 1d), the finest export supplies the value when several exports share a close time. Only this file is committed and bundled with the app.

## Source files

`assets/pricing/raw_data/` currently contains:

- `btc_1d_data_2013_to_2018.csv` — SRK daily closes, 2013-04-29 to 2017-12-31;
- `btc_15m_data_2018_to_2025.csv` — 15-minute closes from 2018-01-01;
- `btc_1h_data_2018_to_2025.csv` — hourly closes from 2018-01-01;
- `btc_4h_data_2018_to_2025.csv` — 4-hour closes from 2018-01-01;
- `btc_1d_data_2018_to_2025.csv` — daily closes from 2018-01-01.

The `..._2025` suffix comes from the Kaggle dataset names; the exports themselves extend beyond 2025.

## Regenerating the packed file

1. Download the Kaggle exports into `assets/pricing/raw_data/` using the file names above.
2. Run `cargo run --bin pricing_pack` from the repository root.

The tool locates the close-time column (`Close time` or `Date`) and the `Close` column from each file's header, so the two export layouts need no per-file configuration. It skips rows whose timestamp or close does not parse (the exports have truncated/malformed rows), reports how many records each file contributed and how many duplicate close times were dropped, then writes the packed file.

This is also based on the assumption that it is easier to compute the data; instead of sending it over the network.
