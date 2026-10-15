pub mod addresses;
pub mod assets;
pub mod blocks;
pub mod fees;
pub mod mempool;
pub mod transactions;
pub mod types;

pub use addresses::Addresses;
pub use assets::Assets;
pub use blocks::Blocks;
pub use fees::Fees;
pub use mempool::MempoolEndpoints as Mempool;
pub use transactions::Transactions;

use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Liquid {
    pub addresses: Addresses,
    pub assets: Assets,
    pub blocks: Blocks,
    pub fees: Fees,
    pub mempool: Mempool,
    pub transactions: Transactions,
}

impl Liquid {
    pub fn new(http: HttpClient) -> Self {
        Self {
            addresses: Addresses::new(http.clone()),
            assets: Assets::new(http.clone()),
            blocks: Blocks::new(http.clone()),
            fees: Fees::new(http.clone()),
            mempool: Mempool::new(http.clone()),
            transactions: Transactions::new(http),
        }
    }
}
