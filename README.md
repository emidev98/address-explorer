# Address Explorer

A Dioxus application for exploring Bitcoin addresses, historical prices, and exchange ledgers.

## Features

- Track Bitcoin addresses, balances, and transaction history
- Explore historical BTC prices and fiat conversions
- Import exchange CSV ledgers, reconcile them with cached on-chain data, and review provider balances
- Display fiat prices and balances in the default currency chosen in Settings
- Switch between Material 3 color palettes and appearance modes in Settings
- Choose automatic, top, or side primary navigation; phones use a slide-in drawer
- Compress the side panel into an icon rail, with Settings and the toggle pinned to the bottom

## Installation

1. **Prerequisites**
   - Rust (latest stable)
   - Cargo
   - Dioxus CLI (if running locally)

2. **Clone the repository**
   ```bash
   git clone https://github.com/emidev98/address-explorer.git
   cd address-explorer
   ```

3. **Run the application**
   ```bash
   dx serve
   ```

Or to build for production:
```bash
cargo build --release
```

Then run the compiled binary.

## Development

For development guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Styles

The interface uses a Material 3 theme implemented with Tailwind CSS v4. For
styling conventions, theme structure, and rebuild instructions, see
[CONTRIBUTING.md](CONTRIBUTING.md#styling-tailwind-css-v4-and-material-3).

## License

MIT
