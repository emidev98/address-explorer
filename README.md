# Explorer

A Dioxus application for exploring Bitcoin addresses, historical prices, and exchange ledgers.

## Features

- Track Bitcoin addresses, balances, and transaction history
- Explore historical BTC prices and fiat conversions
- Import exchange CSV ledgers, reconcile them with cached on-chain data, and review provider balances
- Switch between Material 3 color palettes and appearance modes in Settings

## Installation

1. **Prerequisites**
   - Rust (latest stable)
   - Cargo
   - Dioxus CLI (if running locally)

2. **Clone the repository**
   ```bash
   git clone https://github.com/your-username/exporer.git
   cd exporer
   ```

3. **Run the application**
   ```bash
   dioxus serve
   ```

Or to build for production:
```bash
cargo build --release
```

Then run the compiled binary.

## Sample exchange data

Mocked Revolut, Binance, and Crypto.com ledgers with more than 100 BTC
operations each live in [`assets/exchange_samples/`](assets/exchange_samples/).
Drop one of the CSV files into **Exchanges → Import Data** to try the
import, preview, and reconciliation flow. The same folder documents the public
exchange wallets that are seeded into the addresses collection with their
exchange scope.

## Development

For development guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Styles

The interface uses a Material 3 theme implemented with Tailwind CSS v4. For
styling conventions, theme structure, and rebuild instructions, see
[CONTRIBUTING.md](CONTRIBUTING.md#styling-tailwind-css-v4-and-material-3).

## Screenshots

![Application Screenshot](assets/screenshot.png)

## License

MIT
