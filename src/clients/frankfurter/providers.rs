use crate::clients::frankfurter::error::FrankfurterError;
use crate::clients::frankfurter::http::HttpClient;
use crate::clients::frankfurter::rates::{Expand, Group};
use crate::clients::frankfurter::types::{Provider, Rate};

/// Optional filters for [`Providers::get_provider_rates`]. Fields left at
/// their default are omitted from the request.
///
/// Identical to the parameters of `/rates` except `providers`: the path
/// already scopes rates to one provider. Without a date parameter the latest
/// provider rates are returned.
#[derive(Debug, Clone, Default)]
pub struct ProviderRatesQuery {
    /// Single date (YYYY-MM-DD) to query.
    pub date: Option<String>,
    /// Start of a date range (YYYY-MM-DD).
    pub from: Option<String>,
    /// End of a date range (YYYY-MM-DD); defaults to today.
    pub to: Option<String>,
    /// Base currency code.
    pub base: Option<String>,
    /// Quote currency codes to include, e.g. `["USD", "GBP"]`.
    pub quotes: Vec<String>,
    /// Downsample a date range by week or month.
    pub group: Option<Group>,
    /// Optional fields to expand per rate record.
    pub expand: Option<Expand>,
}

impl ProviderRatesQuery {
    pub(crate) fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(date) = &self.date {
            params.push(("date", date.clone()));
        }
        if let Some(from) = &self.from {
            params.push(("from", from.clone()));
        }
        if let Some(to) = &self.to {
            params.push(("to", to.clone()));
        }
        if let Some(base) = &self.base {
            params.push(("base", base.clone()));
        }
        if !self.quotes.is_empty() {
            params.push(("quotes", self.quotes.join(",")));
        }
        if let Some(group) = self.group {
            params.push(("group", group.as_param().to_string()));
        }
        if let Some(expand) = self.expand {
            params.push(("expand", expand.as_param().to_string()));
        }
        params
    }
}

#[derive(Clone)]
pub struct Providers {
    http: HttpClient,
}

impl Providers {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns available exchange rate data providers with their publication
    /// coverage.
    pub async fn get_providers(&self) -> Result<Vec<Provider>, FrankfurterError> {
        self.http.get("/providers").await
    }

    /// Returns one entry from `/providers`, by key (case-insensitive).
    pub async fn get_provider(&self, provider: &str) -> Result<Provider, FrankfurterError> {
        self.http.get(&format!("/providers/{}", provider)).await
    }

    /// Returns one provider's rates as published, unblended. Identical to
    /// `/rates?providers={provider}`: same parameters, same records, same
    /// limits. Native-base records carry the provider's published digits;
    /// other bases are derived.
    pub async fn get_provider_rates(
        &self,
        provider: &str,
        query: ProviderRatesQuery,
    ) -> Result<Vec<Rate>, FrankfurterError> {
        let path = format!("/providers/{}/rates", provider);
        self.http.get_query(&path, &query.to_params()).await
    }

    /// Returns one provider's rate for a single currency pair. Identical to
    /// `/rate/{base}/{quote}?providers={provider}`. The pair may be published
    /// directly or derived from the provider's own rates for both currencies.
    pub async fn get_provider_rate(
        &self,
        provider: &str,
        base: &str,
        quote: &str,
        date: Option<&str>,
    ) -> Result<Rate, FrankfurterError> {
        let path = format!("/providers/{}/rate/{}/{}", provider, base, quote);
        let mut params: Vec<(&'static str, String)> = Vec::new();
        if let Some(date) = date {
            params.push(("date", date.to_string()));
        }
        self.http.get_query(&path, &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::frankfurter::rates::Group;

    #[test]
    fn default_query_sends_no_params() {
        let query = ProviderRatesQuery::default();
        assert!(query.to_params().is_empty());
    }

    #[test]
    fn fields_map_to_their_query_names_in_order() {
        let query = ProviderRatesQuery {
            date: Some("2024-05-15".into()),
            from: Some("2024-01-01".into()),
            to: Some("2024-01-31".into()),
            base: Some("EUR".into()),
            quotes: vec!["USD".into()],
            group: Some(Group::Week),
            expand: Some(Expand::Providers),
        };
        assert_eq!(
            query.to_params(),
            vec![
                ("date", "2024-05-15".to_string()),
                ("from", "2024-01-01".to_string()),
                ("to", "2024-01-31".to_string()),
                ("base", "EUR".to_string()),
                ("quotes", "USD".to_string()),
                ("group", "week".to_string()),
                ("expand", "providers".to_string()),
            ]
        );
    }
}
