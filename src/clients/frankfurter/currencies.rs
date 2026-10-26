use crate::clients::frankfurter::error::FrankfurterError;
use crate::clients::frankfurter::http::HttpClient;
use crate::clients::frankfurter::types::{Currency, CurrencyDetail};

/// Optional scope for the currency catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencyScope {
    /// Include legacy currencies that no longer have publication coverage.
    All,
}

impl CurrencyScope {
    pub fn as_param(&self) -> &'static str {
        match self {
            Self::All => "all",
        }
    }
}

#[derive(Clone)]
pub struct Currencies {
    http: HttpClient,
}

impl Currencies {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns available currencies with their names and publication date
    /// ranges. Without providers, the catalogue covers eligible daily sources
    /// plus pegged currencies; [CurrencyScope::All] also includes legacy
    /// currencies.
    ///
    /// These dates do not guarantee a queryable base/quote pair; use
    /// `/coverage` (see [CoverageEndpoints]) with matching query filters.
    pub async fn get_currencies(
        &self,
        scope: Option<CurrencyScope>,
        providers: &[&str],
    ) -> Result<Vec<Currency>, FrankfurterError> {
        let mut params: Vec<(&'static str, String)> = Vec::new();
        if let Some(scope) = scope {
            params.push(("scope", scope.as_param().to_string()));
        }
        if !providers.is_empty() {
            params.push(("providers", providers.join(",")));
        }
        self.http.get_query("/currencies", &params).await
    }

    /// Returns details for a single currency, including provider information
    /// or peg metadata.
    pub async fn get_currency(&self, code: &str) -> Result<CurrencyDetail, FrankfurterError> {
        self.http.get(&format!("/currency/{}", code)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_maps_to_the_api_value() {
        assert_eq!(CurrencyScope::All.as_param(), "all");
    }
}
