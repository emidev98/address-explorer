use crate::clients::frankfurter::error::FrankfurterError;
use crate::clients::frankfurter::http::HttpClient;
use crate::clients::frankfurter::types::Coverage;

/// Endpoints for `/coverage`.
#[derive(Clone)]
pub struct CoverageEndpoints {
    http: HttpClient,
}

impl CoverageEndpoints {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns the first and last eligible observation dates at which
    /// `/rates` with the same base, quotes and providers returns at least one
    /// record. Defaults to the EUR-based daily blended feed.
    ///
    /// Null bounds mean no queryable pair. Interior gaps are possible, and
    /// bounds are request dates, not necessarily returned record dates.
    pub async fn get_coverage(
        &self,
        base: Option<&str>,
        quotes: &[&str],
        providers: &[&str],
    ) -> Result<Coverage, FrankfurterError> {
        let mut params: Vec<(&'static str, String)> = Vec::new();
        if let Some(base) = base {
            params.push(("base", base.to_string()));
        }
        if !quotes.is_empty() {
            params.push(("quotes", quotes.join(",")));
        }
        if !providers.is_empty() {
            params.push(("providers", providers.join(",")));
        }
        self.http.get_query("/coverage", &params).await
    }
}
