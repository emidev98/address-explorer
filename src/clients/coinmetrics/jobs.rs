use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::types::{CreatedJob, JobsResponse};
use serde::Serialize;

/// Request body for `PUT /blockchain-job/account-balances`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountBalancesJobRequest {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_slot: Option<i64>,
}

/// Request body for `PUT /blockchain-job/balance-updates`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BalanceUpdatesJobRequest {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_slot: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_slot: Option<i64>,
}

/// Request body for `PUT /blockchain-job/transactions`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TransactionsJobRequest {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub txids: Vec<String>,
}

/// Asynchronous blockchain export jobs and their status endpoint.
#[derive(Clone)]
pub struct Jobs {
    http: HttpClient,
}

impl Jobs {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Creates a job returning account balances at a point in time.
    pub async fn create_account_balances_job(
        &self,
        request: AccountBalancesJobRequest,
    ) -> Result<CreatedJob, CoinMetricsError> {
        self.http
            .put_json("/blockchain-job/account-balances", &request)
            .await
    }

    /// Creates a job returning balance updates for an interval.
    pub async fn create_balance_updates_job(
        &self,
        request: BalanceUpdatesJobRequest,
    ) -> Result<CreatedJob, CoinMetricsError> {
        self.http
            .put_json("/blockchain-job/balance-updates", &request)
            .await
    }

    /// Creates a job returning transactions for the given txids.
    pub async fn create_transactions_job(
        &self,
        request: TransactionsJobRequest,
    ) -> Result<CreatedJob, CoinMetricsError> {
        self.http
            .put_json("/blockchain-job/transactions", &request)
            .await
    }

    /// Returns status, result URLs and warnings for the given job ids.
    pub async fn get_jobs(
        &self,
        ids: &str,
        include_warnings: Option<bool>,
    ) -> Result<JobsResponse, CoinMetricsError> {
        let mut query: Vec<(&str, String)> = vec![("ids", ids.to_string())];
        if let Some(value) = include_warnings {
            query.push(("include_warnings", value.to_string()));
        }
        self.http.get_query("/jobs", &query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn job_requests_serialize_arrays_as_json() {
        let request = TransactionsJobRequest {
            assets: vec!["btc".into()],
            txids: vec!["abc".into(), "def".into()],
        };
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({"assets": ["btc"], "txids": ["abc", "def"]})
        );
    }

    #[test]
    fn account_balance_request_omits_unset_fields() {
        let request = AccountBalancesJobRequest {
            assets: vec!["eth".into()],
            accounts: vec!["0xabc".into()],
            at_height: Some(100),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({"assets": ["eth"], "accounts": ["0xabc"], "at_height": 100})
        );
    }

    #[test]
    fn job_ids_query_serializes_as_repeated_parameters() {
        let query: Vec<(&str, String)> = vec![
            ("ids", "job-1".to_string()),
            ("include_warnings", "true".to_string()),
        ];
        let request = reqwest::Client::new()
            .get("https://example.com/jobs")
            .query(&query)
            .build()
            .unwrap();
        assert_eq!(
            request.url().query(),
            Some("ids=job-1&include_warnings=true")
        );
    }
}
