use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page};
use crate::clients::coinmetrics::types::{
    BlockchainAccount, BlockchainBalanceUpdate, BlockchainBlock, BlockchainFullBlock,
    BlockchainFullSingleTransaction, BlockchainFullTransaction, BlockchainRebasingChange,
    BlockchainSubAccount, BlockchainTransaction, ListResponse, TxTrackerTransaction,
};
use serde::Serialize;

/// Time and height interval shared by the blockchain-v2 endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChainTimeRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// `chain_sequence_number` interval shared by account, balance-update and
/// rebasing endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChainSequenceRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_chain_sequence_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_chain_sequence_number: Option<i64>,
}

/// Query for `/blockchain-v2/{asset}/blocks`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BlocksQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub block_hashes: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub heights: Vec<String>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    /// `main` (default) or `all` (includes stale blocks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for `/blockchain-v2/{asset}/accounts` and
/// `/blockchain-v2/{asset}/sub-accounts`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub accounts: Vec<String>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(flatten)]
    pub sequence: ChainSequenceRange,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for `/blockchain-v2/{asset}/transactions`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TransactionsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub txids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub block_hashes: Vec<String>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for `/blockchain-v2/{asset}/balance-updates`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BalanceUpdatesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub accounts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_account: Option<u32>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub sub_accounts: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub txids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub block_hashes: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub denominations: Vec<String>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(flatten)]
    pub sequence: ChainSequenceRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_sub_accounts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for
/// `/blockchain-v2/{asset}/accounts/{account}/balance-updates`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountBalanceUpdatesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub txids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub block_hashes: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub denominations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_counterparties: Option<bool>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(flatten)]
    pub sequence: ChainSequenceRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_sub_accounts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for `/blockchain-v2/{asset}/rebasing-changes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RebasingChangesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub txids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub block_hashes: Vec<String>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(flatten)]
    pub sequence: ChainSequenceRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Options shared by the full block and full transaction endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FullEntityQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_sub_accounts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
}

/// Query for `/blockchain/{asset}/transaction-tracker`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TxTrackerQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub txids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub replacements_for_txids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacements_only: Option<bool>,
    /// BTC only: track transactions touching these addresses.
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub addresses: Vec<String>,
    /// BTC only: requires `addresses`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unconfirmed_only: Option<bool>,
    #[serde(flatten)]
    pub time: ChainTimeRange,
    #[serde(flatten)]
    pub page: Page,
}

/// Blockchain v2 list/full-entity endpoints and the transaction tracker.
#[derive(Clone)]
pub struct Blockchain {
    http: HttpClient,
}

impl Blockchain {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_blocks(
        &self,
        asset: &str,
        query: BlocksQuery,
    ) -> Result<ListResponse<BlockchainBlock>, CoinMetricsError> {
        self.http
            .get_query(&format!("/blockchain-v2/{}/blocks", asset), &query)
            .await
    }

    pub async fn get_accounts(
        &self,
        asset: &str,
        query: AccountsQuery,
    ) -> Result<ListResponse<BlockchainAccount>, CoinMetricsError> {
        self.http
            .get_query(&format!("/blockchain-v2/{}/accounts", asset), &query)
            .await
    }

    pub async fn get_sub_accounts(
        &self,
        asset: &str,
        query: AccountsQuery,
    ) -> Result<ListResponse<BlockchainSubAccount>, CoinMetricsError> {
        self.http
            .get_query(&format!("/blockchain-v2/{}/sub-accounts", asset), &query)
            .await
    }

    pub async fn get_transactions(
        &self,
        asset: &str,
        query: TransactionsQuery,
    ) -> Result<ListResponse<BlockchainTransaction>, CoinMetricsError> {
        self.http
            .get_query(&format!("/blockchain-v2/{}/transactions", asset), &query)
            .await
    }

    pub async fn get_balance_updates(
        &self,
        asset: &str,
        query: BalanceUpdatesQuery,
    ) -> Result<ListResponse<BlockchainBalanceUpdate>, CoinMetricsError> {
        self.http
            .get_query(&format!("/blockchain-v2/{}/balance-updates", asset), &query)
            .await
    }

    pub async fn get_rebasing_changes(
        &self,
        asset: &str,
        query: RebasingChangesQuery,
    ) -> Result<ListResponse<BlockchainRebasingChange>, CoinMetricsError> {
        self.http
            .get_query(
                &format!("/blockchain-v2/{}/rebasing-changes", asset),
                &query,
            )
            .await
    }

    pub async fn get_account_balance_updates(
        &self,
        asset: &str,
        account: &str,
        query: AccountBalanceUpdatesQuery,
    ) -> Result<ListResponse<BlockchainBalanceUpdate>, CoinMetricsError> {
        self.http
            .get_query(
                &format!(
                    "/blockchain-v2/{}/accounts/{}/balance-updates",
                    asset, account
                ),
                &query,
            )
            .await
    }

    pub async fn get_block(
        &self,
        asset: &str,
        block_hash: &str,
        query: FullEntityQuery,
    ) -> Result<BlockchainFullBlock, CoinMetricsError> {
        self.http
            .get_query(
                &format!("/blockchain-v2/{}/blocks/{}", asset, block_hash),
                &query,
            )
            .await
    }

    pub async fn get_transaction(
        &self,
        asset: &str,
        txid: &str,
        query: FullEntityQuery,
    ) -> Result<BlockchainFullSingleTransaction, CoinMetricsError> {
        self.http
            .get_query(
                &format!("/blockchain-v2/{}/transactions/{}", asset, txid),
                &query,
            )
            .await
    }

    pub async fn get_block_transaction(
        &self,
        asset: &str,
        block_hash: &str,
        txid: &str,
        query: FullEntityQuery,
    ) -> Result<BlockchainFullTransaction, CoinMetricsError> {
        self.http
            .get_query(
                &format!(
                    "/blockchain-v2/{}/blocks/{}/transactions/{}",
                    asset, block_hash, txid
                ),
                &query,
            )
            .await
    }

    pub async fn get_tx_tracker(
        &self,
        asset: &str,
        query: TxTrackerQuery,
    ) -> Result<ListResponse<TxTrackerTransaction>, CoinMetricsError> {
        self.http
            .get_query(
                &format!("/blockchain/{}/transaction-tracker", asset),
                &query,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_blockchain_filters() {
        let query = BlocksQuery {
            heights: vec!["100".into(), "101".into()],
            time: ChainTimeRange {
                start_inclusive: Some(false),
                ..Default::default()
            },
            chain: Some("main".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({"heights": "100,101", "start_inclusive": false, "chain": "main"})
        );
    }

    #[test]
    fn serializes_balance_update_filters() {
        let query = BalanceUpdatesQuery {
            accounts: vec!["bc1qexample".into()],
            denominations: vec!["native".into()],
            sequence: ChainSequenceRange {
                start_chain_sequence_number: Some(10),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({
                "accounts": "bc1qexample",
                "denominations": "native",
                "start_chain_sequence_number": 10
            })
        );
    }
}
