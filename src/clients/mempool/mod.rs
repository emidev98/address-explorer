#![allow(dead_code, unused_imports)]
pub mod bitcoin;
pub mod config;
pub mod error;
pub mod http;
pub mod liquid;
pub mod ws;

pub use bitcoin::Bitcoin;
pub use config::MempoolConfig;
pub use error::MempoolError;
pub use http::HttpClient;
pub use liquid::Liquid;

use bitcoin::Bitcoin as BitcoinApi;
use liquid::Liquid as LiquidApi;

#[derive(Clone)]
pub struct Mempool {
    pub bitcoin: BitcoinApi,
    pub liquid: LiquidApi,
}

impl Mempool {
    pub fn new(config: MempoolConfig) -> Self {
        let bitcoin_http = HttpClient::new(config.bitcoin_base_url(), &config);
        let liquid_http = HttpClient::new(config.liquid_base_url(), &config);
        Self {
            bitcoin: BitcoinApi::new(bitcoin_http),
            liquid: LiquidApi::new(liquid_http),
        }
    }
}
