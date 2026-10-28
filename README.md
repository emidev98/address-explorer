# Explorer

A simple Dioxus application for exploring files and directories.

## Features

- Browse files and directories
- View file contents
- Simple and minimal UI
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
