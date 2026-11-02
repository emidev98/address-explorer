//! App-wide default fiat currency.
//!
//! The preference is loaded once by the root component and provided through
//! Dioxus context. Screens read the same signal, so changing the selector in
//! Settings updates every open surface immediately instead of each screen
//! fetching the setting independently.

use dioxus::prelude::*;

/// Shared default fiat currency, provided once by the application root.
#[derive(Clone, Copy)]
pub struct FiatCurrency(pub Signal<String>);

impl FiatCurrency {
    /// Current ISO currency code, e.g. `USD`.
    pub fn code(self) -> String {
        self.0()
    }

    /// Replace the current ISO currency code.
    pub fn set(mut self, code: impl Into<String>) {
        self.0.set(code.into());
    }
}

/// Access the app-wide default fiat currency.
pub fn use_fiat_currency() -> FiatCurrency {
    use_context()
}
