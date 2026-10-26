use serde::Deserialize;

/// A blended exchange-rate record as returned by `/rates` and
/// `/rate/{base}/{quote}`.
#[derive(Debug, Clone, Deserialize)]
pub struct Rate {
    /// Observation date (YYYY-MM-DD).
    pub date: String,
    /// Base currency or accounting unit code.
    pub base: String,
    /// Quote currency or accounting unit code.
    pub quote: String,
    /// Blended exchange rate (positive).
    pub rate: f64,
    /// Per-provider breakdown, present only when `expand=providers` was
    /// requested. Synthesized peg rows omit it because their rate comes from
    /// the peg, not from provider data.
    pub providers: Option<Vec<ProviderRate>>,
}

/// One provider's contribution to a blended [`Rate`].
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderRate {
    /// Provider key.
    pub key: String,
    /// Provider observation date used for this entry.
    pub date: String,
    /// Provider's rate, rebased to the row's base.
    pub rate: f64,
    /// Present and true when this entry did not contribute to the blended
    /// rate, either flagged as an outlier or overridden by a currency peg.
    pub excluded: Option<bool>,
}

/// Queryable history coverage as returned by `/coverage`.
#[derive(Debug, Clone, Deserialize)]
pub struct Coverage {
    /// Requested base currency, EUR by default.
    pub base: String,
    /// Requested quotes, or `None` for all.
    pub quotes: Option<Vec<String>>,
    /// Selected providers, or `None` for the default daily blend.
    pub providers: Option<Vec<String>>,
    /// First eligible observation date, or `None` when no pair is queryable.
    pub start_date: Option<String>,
    /// Last eligible observation date, or `None` when no pair is queryable.
    pub end_date: Option<String>,
}

/// A currency from the catalogue returned by `/currencies`.
#[derive(Debug, Clone, Deserialize)]
pub struct Currency {
    /// Currency or accounting unit code.
    pub iso_code: String,
    /// ISO 4217 numeric code.
    pub iso_numeric: Option<String>,
    /// Full currency name.
    pub name: String,
    pub symbol: Option<String>,
    /// Earliest publication coverage date; use `/coverage` for queryable pairs.
    pub start_date: Option<String>,
    /// Latest publication coverage date; use `/coverage` for queryable pairs.
    pub end_date: Option<String>,
}

/// Peg metadata, present only for pegged currencies in `/currency/{code}`.
#[derive(Debug, Clone, Deserialize)]
pub struct CurrencyPeg {
    pub base: String,
    pub rate: f64,
    pub authority: Option<String>,
    pub source: Option<String>,
}

/// Details for a single currency as returned by `/currency/{code}`.
#[derive(Debug, Clone, Deserialize)]
pub struct CurrencyDetail {
    /// Currency or accounting unit code.
    pub iso_code: String,
    /// ISO 4217 numeric code.
    pub iso_numeric: Option<String>,
    /// Full currency name.
    pub name: String,
    pub symbol: Option<String>,
    /// Provider keys that publish this currency.
    pub providers: Option<Vec<String>>,
    pub peg: Option<CurrencyPeg>,
}

/// A data provider as returned by `/providers` and `/providers/{provider}`.
#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub key: String,
    /// Full provider name.
    pub name: String,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: Option<String>,
    /// Official rate type as used by the source.
    pub rate_type: Option<String>,
    /// Base currency for published rates.
    pub pivot_currency: Option<String>,
    /// Link to the data source.
    pub data_url: Option<String>,
    /// Link to the terms of use.
    pub terms_url: Option<String>,
    /// Earliest publication coverage date; use `/coverage` for queryable pairs.
    pub start_date: Option<String>,
    /// Latest publication coverage date; use `/coverage` for queryable pairs.
    pub end_date: Option<String>,
    /// How often the provider publishes rates, in `None` for historical-only
    /// providers without a scheduled cadence (`daily` | `weekly` | `monthly` |
    /// `quarterly`).
    pub publish_cadence: Option<String>,
    /// The period each observation stands for, after SDMX FREQ. Coarser than
    /// daily never enters the blend: query such providers directly (`daily` |
    /// `monthly` | `quarterly`).
    pub frequency: Option<String>,
    /// Number of expected publishes missed since `end_date`, in units of
    /// `publish_cadence`.
    pub publishes_missed: Option<i64>,
    /// Recognised currency codes within their valid date ranges covered by
    /// this provider.
    pub currencies: Vec<String>,
    /// Unreviewed published codes absent from the currency registry. Available
    /// through single-provider rates but excluded from blends and the currency
    /// catalogue.
    pub unknown_currencies: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_a_single_rate_record() {
        let json = r#"{"date":"2026-03-25","base":"EUR","quote":"USD","rate":1.1568}"#;
        let rate: Rate = serde_json::from_str(json).unwrap();
        assert_eq!(rate.date, "2026-03-25");
        assert_eq!(rate.base, "EUR");
        assert_eq!(rate.quote, "USD");
        assert_eq!(rate.rate, 1.1568);
        assert!(rate.providers.is_none());
    }

    #[test]
    fn deserializes_the_expand_providers_breakdown() {
        let record = serde_json::from_str::<Rate>(
            r#"{
                "date": "2026-03-25", "base": "EUR", "quote": "USD", "rate": 1.1568,
                "providers": [
                    {"key": "ECB", "date": "2026-03-24", "rate": 0.916, "excluded": false},
                    {"key": "FED", "date": "2026-03-25", "rate": 1.164, "excluded": true}
                ]
            }"#,
        )
        .unwrap();
        let breakdown = record.providers.unwrap();
        assert_eq!(breakdown.len(), 2);
        assert_eq!(breakdown[0].key, "ECB");
        assert_eq!(breakdown[1].excluded, Some(true));
    }

    #[test]
    fn deserializes_provider_rate_without_excluded_flag() {
        let record: ProviderRate =
            serde_json::from_str(r#"{"key":"ECB","date":"2026-03-24","rate":0.916}"#).unwrap();
        assert_eq!(record.excluded, None);
    }

    #[test]
    fn deserializes_null_optional_fields_of_coverage() {
        let json = r#"{
            "base":"USD","quotes":null,"providers":null,"start_date":null,"end_date":null
        }"#;
        let coverage: Coverage = serde_json::from_str(json).unwrap();
        assert_eq!(coverage.base, "USD");
        assert_eq!(coverage.quotes, None);
        assert_eq!(coverage.start_date, None);
    }

    #[test]
    fn deserializes_a_captured_provider_detail() {
        let json = r#"{
            "key": "ECB",
            "name": "European Central Bank",
            "country_code": "EU",
            "rate_type": "reference rate",
            "pivot_currency": "EUR",
            "data_url": "https://www.ecb.europa.eu/",
            "terms_url": "https://www.ecb.europa.eu/disclaimer/",
            "start_date": "1999-01-04",
            "end_date": "2026-10-23",
            "publish_cadence": "daily",
            "frequency": "daily",
            "publishes_missed": 0,
            "currencies": ["EUR", "USD"],
            "unknown_currencies": []
        }"#;
        let provider: Provider = serde_json::from_str(json).unwrap();
        assert_eq!(provider.key, "ECB");
        assert_eq!(provider.frequency.as_deref(), Some("daily"));
        assert_eq!(provider.publishes_missed, Some(0));
        assert_eq!(provider.currencies.len(), 2);
    }

    #[test]
    fn deserializes_a_captured_currency_detail() {
        let json = r#"{
            "iso_code": "USD",
            "iso_numeric": "840",
            "name": "United States Dollar",
            "symbol": "$",
            "start_date": "1948-06-21",
            "end_date": "2026-10-26",
            "providers": ["ECB", "BOC", "FED"]
        }"#;
        let detail: CurrencyDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.iso_code, "USD");
        assert_eq!(detail.symbol.as_deref(), Some("$"));
        assert_eq!(detail.providers.map(|p| p.len()), Some(3));
        assert!(detail.peg.is_none());
    }

    #[test]
    fn deserializes_currency_peg_metadata() {
        let json = r#"{
            "iso_code": "HKD",
            "name": "Hong Kong Dollar",
            "peg": {
                "base": "USD",
                "rate": 7.75,
                "authority": "HKMA",
                "source": "https://www.hkma.gov.hk/"
            }
        }"#;
        let detail: CurrencyDetail = serde_json::from_str(json).unwrap();
        let peg = detail.peg.unwrap();
        assert_eq!(peg.base, "USD");
        assert_eq!(peg.rate, 7.75);
    }

    #[test]
    fn deserializes_a_catalogue_entry() {
        let json = r#"{"iso_code":"EUR","iso_numeric":"978","name":"Euro","symbol":"€"}"#;
        let currency: Currency = serde_json::from_str(json).unwrap();
        assert_eq!(currency.iso_code, "EUR");
        assert_eq!(currency.name, "Euro");
        assert!(currency.start_date.is_none());
    }
}
