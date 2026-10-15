pub mod addresses;
pub mod blocks;
pub mod difficulty;
pub mod fees;
pub mod lightning;
pub mod mempool;
pub mod transactions;
pub mod types;

pub use addresses::Addresses;
pub use blocks::Blocks;
pub use difficulty::Difficulty;
pub use fees::Fees;
pub use lightning::Lightning;
pub use mempool::MempoolEndpoints as Mempool;
pub use transactions::Transactions;

use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Bitcoin {
    pub addresses: Addresses,
    pub blocks: Blocks,
    pub difficulty: Difficulty,
    pub fees: Fees,
    pub lightning: Lightning,
    pub mempool: Mempool,
    pub transactions: Transactions,
}

impl Bitcoin {
    pub fn new(http: HttpClient) -> Self {
        Self {
            addresses: Addresses::new(http.clone()),
            blocks: Blocks::new(http.clone()),
            difficulty: Difficulty::new(http.clone()),
            fees: Fees::new(http.clone()),
            lightning: Lightning::new(http.clone()),
            mempool: Mempool::new(http.clone()),
            transactions: Transactions::new(http),
        }
    }
}
