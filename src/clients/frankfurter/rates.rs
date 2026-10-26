use crate::clients::frankfurter::error::FrankfurterError;
use crate::clients::frankfurter::http::HttpClient;
use crate::clients::frankfurter::types::Rate;

/// Downsample a date range by time period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// One record per ISO week.
    Week,
    /// One record per calendar month.
    Month,
}

impl Group {
    pub fn as_param(&self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

/// Optional fields to expand per rate record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expand {
    /// Adds a per-provider rate breakdown to each record.
    Providers,
}

impl Expand {
    pub fn as_param(&self) -> &'static str {
        match self {
            Self::Providers => "providers",
        }
    }
}

/// Optional filters for the `/rates` query. Fields left at their default are
/// omitted from the request.
///
/// `date` cannot be combined with `from`/`to`; the server rejects such
/// combinations with HTTP 422. Without any date parameter the latest rates
/// are returned.
#[derive(Debug, Clone, Default)]
pub struct RatesQuery {
    /// Single date (YYYY-MM-DD) to query.
    pub date: Option<String>,
    /// Start of a date range (YYYY-MM-DD).
    pub from: Option<String>,
    /// End of a date range (YYYY-MM-DD); defaults to today.
    pub to: Option<String>,
    /// Base currency code; defaults to EUR.
    pub base: Option<String>,
    /// Quote currency codes to include, e.g. `["USD", "GBP"]`.
    pub quotes: Vec<String>,
    /// Provider keys to blend, e.g. `["ECB", "TCMB"]`.
    ///
    /// Requests naming providers recompute the blend per date, so at daily
    /// granularity they return HTTP 422 for ranges longer than five years: a
    /// single provider is exempt and serves any range.
    pub providers: Vec<String>,
    /// Downsample a date range by week or month.
    pub group: Option<Group>,
    /// Optional fields to expand per rate record.
    pub expand: Option<Expand>,
}

impl RatesQuery {
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
        if !self.providers.is_empty() {
            params.push(("providers", self.providers.join(",")));
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
pub struct Rates {
    http: HttpClient,
}

impl Rates {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns exchange rates blended across providers, as an array of
    /// [Rate] records where each record is a single currency pair. Without
    /// date parameters in `query` the latest rates are returned.
    ///
    /// Ranges longer than five years return HTTP 422 when recompute-per-date
    /// features are requested (see [RatesQuery::providers] and
    /// [Expand::Providers]); aggregate with [Group], narrow with quotes, or
    /// split the range into shorter requests in that case.
    pub async fn get_rates(&self, query: RatesQuery) -> Result<Vec<Rate>, FrankfurterError> {
        self.http.get_query("/rates", &query.to_params()).await
    }

    /// Returns the blended exchange rate for a single currency pair. Without
    /// `date`, returns the latest rate. A same-currency pair returns the
    /// identity rate of 1.
    pub async fn get_rate(
        &self,
        base: &str,
        quote: &str,
        date: Option<&str>,
        providers: &[&str],
    ) -> Result<Rate, FrankfurterError> {
        let path = format!("/rate/{}/{}", base, quote);
        let mut params: Vec<(&'static str, String)> = Vec::new();
        if let Some(date) = date {
            params.push(("date", date.to_string()));
        }
        if !providers.is_empty() {
            params.push(("providers", providers.join(",")));
        }
        self.http.get_query(&path, &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_sends_no_params() {
        let query = RatesQuery::default();
        assert!(query.to_params().is_empty());
    }

    #[test]
    fn dates_map_to_their_query_names() {
        let query = RatesQuery {
            date: Some("2024-01-15".into()),
            from: Some("2024-01-01".into()),
            to: Some("2024-01-31".into()),
            ..Default::default()
        };
        assert_eq!(
            query.to_params(),
            vec![
                ("date", "2024-01-15".to_string()),
                ("from", "2024-01-01".to_string()),
                ("to", "2024-01-31".to_string()),
            ]
        );
    }

    #[test]
    fn filters_and_enums_map_to_api_values() {
        let query = RatesQuery {
            base: Some("USD".into()),
            quotes: vec!["GBP".into(), "JPY".into()],
            providers: vec!["ECB".into(), "TCMB".into()],
            group: Some(Group::Month),
            expand: Some(Expand::Providers),
            ..Default::default()
        };
        assert_eq!(
            query.to_params(),
            vec![
                ("base", "USD".to_string()),
                ("quotes", "GBP,JPY".to_string()),
                ("providers", "ECB,TCMB".to_string()),
                ("group", "month".to_string()),
                ("expand", "providers".to_string()),
            ]
        );
    }

    #[test]
    fn group_and_expand_use_lowercase_params() {
        assert_eq!(Group::Week.as_param(), "week");
        assert_eq!(Group::Month.as_param(), "month");
        assert_eq!(Expand::Providers.as_param(), "providers");
    }
}
