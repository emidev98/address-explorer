#![allow(dead_code, unused_imports)]
pub mod config;
pub mod coverage;
pub mod currencies;
pub mod error;
pub mod http;
pub mod providers;
pub mod rates;
pub mod types;

pub use config::FrankfurterConfig;
pub use currencies::{Currencies, CurrencyScope};
pub use error::FrankfurterError;
pub use providers::{ProviderRatesQuery, Providers};
pub use rates::{Expand, Group, Rates, RatesQuery};

use coverage::CoverageEndpoints;
use http::HttpClient;

#[derive(Clone)]
pub struct Frankfurter {
    pub currencies: Currencies,
    pub coverage: CoverageEndpoints,
    pub providers: Providers,
    pub rates: Rates,
}

impl Frankfurter {
    /// Creates a client for the Frankfurter exchange-rate API described in
    /// `config`; no API key is required by the public instance.
    pub fn new(config: FrankfurterConfig) -> Self {
        let http = HttpClient::new(config.base_url(), &config);
        Self {
            currencies: Currencies::new(http.clone()),
            coverage: CoverageEndpoints::new(http.clone()),
            providers: Providers::new(http.clone()),
            rates: Rates::new(http),
        }
    }
}
