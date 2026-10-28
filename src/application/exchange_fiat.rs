//! Historical fiat conversion for exchange ledger values.
//!
//! The current default currency is resolved from Settings. All conversions
//! go through a persistent rate cache; network calls happen outside any
//! SQLite guard and failures degrade to a warning rather than failing the
//! page.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::clients::frankfurter::{Frankfurter, FrankfurterConfig, RatesQuery};
use crate::data::repositories::{fiat_rates as rate_repository, settings as settings_repository};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::exchange::ExchangeTransaction;
use crate::domain::price::USD_SYMBOL;

const MAX_RANGE_DAYS: i64 = 366;

/// One fiat value that needs conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct FiatValue {
    pub timestamp: i64,
    pub amount: f64,
    pub currency: String,
}

/// Conversion service for exchange rows and CSV previews.
#[derive(Clone)]
pub struct ExchangeFiatService {
    database: Database,
    frankfurter: Frankfurter,
}

impl ExchangeFiatService {
    pub fn connect(database: Database) -> Self {
        Self {
            database,
            frankfurter: Frankfurter::new(FrankfurterConfig {
                timeout: Some(Duration::from_secs(20)),
                ..FrankfurterConfig::default()
            }),
        }
    }

    /// The current default currency, falling back to USD.
    pub fn target_currency(&self) -> Result<String, DatabaseError> {
        let symbol = settings_repository::load_default_fiat_currency(&self.database)?;
        Ok(if symbol.trim().is_empty() {
            USD_SYMBOL.to_string()
        } else {
            symbol.to_ascii_uppercase()
        })
    }

    /// Convert values into `target`, returning one entry per input value.
    pub async fn convert_values(
        &self,
        target: &str,
        values: &[FiatValue],
    ) -> (Vec<Option<f64>>, Option<String>) {
        let target = if target.trim().is_empty() {
            USD_SYMBOL.to_string()
        } else {
            target.trim().to_ascii_uppercase()
        };
        if values.is_empty() {
            return (Vec::new(), None);
        }
        let mut warning: Option<String> = None;
        let mut rates: HashMap<(String, String), f64> = HashMap::new();
        let mut missing: HashSet<(String, String)> = HashSet::new();

        for value in values {
            let base = value.currency.trim().to_ascii_uppercase();
            if base.is_empty() {
                continue;
            }
            let date = civil_time::format_date(value.timestamp);
            if base == target {
                rates.insert((base, date), 1.0);
                continue;
            }
            match rate_repository::latest_rate(&self.database, &base, &target, &date) {
                Ok(Some(rate)) => {
                    rates.insert((base, date), rate.rate);
                }
                Ok(None) => {
                    missing.insert((base, date));
                }
                Err(error) => {
                    warning = Some(format!("Could not read cached fiat rates: {error}"));
                }
            }
        }

        if !missing.is_empty() {
            let mut bases: HashMap<String, Vec<String>> = HashMap::new();
            for (base, date) in &missing {
                bases.entry(base.clone()).or_default().push(date.clone());
            }
            for (base, dates) in bases {
                let mut dates = dates;
                dates.sort();
                dates.dedup();
                let range_start = dates.first().cloned().unwrap_or_default();
                let range_end = dates.last().cloned().unwrap_or_default();
                if let Err(error) = self
                    .fetch_rate_ranges(&target, &base, &range_start, &range_end)
                    .await
                {
                    warning = Some(format!(
                        "Some historical fiat rates could not be loaded: {error}"
                    ));
                }
                for date in &dates {
                    if let Ok(Some(rate)) =
                        rate_repository::latest_rate(&self.database, &base, &target, date)
                    {
                        rates.insert((base.clone(), date.clone()), rate.rate);
                    }
                }
            }

            // A range starting on a weekend can have no preceding cached
            // row. Ask the single-pair endpoint for those dates.
            let still_missing = missing
                .iter()
                .filter(|(base, date)| !rates.contains_key(&(base.clone(), date.clone())))
                .cloned()
                .collect::<Vec<_>>();
            for (base, date) in still_missing {
                match self
                    .frankfurter
                    .rates
                    .get_rate(&target, &base, Some(&date), &[])
                    .await
                {
                    Ok(rate) if rate.rate > 0.0 => {
                        let converted = 1.0 / rate.rate;
                        let _ = rate_repository::upsert_rate(
                            &self.database,
                            &base,
                            &target,
                            &rate.date,
                            converted,
                            now(),
                        );
                        if let Ok(Some(cached)) =
                            rate_repository::latest_rate(&self.database, &base, &target, &date)
                        {
                            rates.insert((base.clone(), date.clone()), cached.rate);
                        }
                    }
                    Ok(_) => {
                        warning = Some("A fiat rate was invalid and was skipped.".to_string());
                    }
                    Err(error) => {
                        warning = Some(format!(
                            "Some historical fiat rates could not be loaded: {error}"
                        ));
                    }
                }
            }
        }

        let converted = values
            .iter()
            .map(|value| {
                let base = value.currency.trim().to_ascii_uppercase();
                if base.is_empty() {
                    return None;
                }
                let date = civil_time::format_date(value.timestamp);
                rates
                    .get(&(base.clone(), date))
                    .copied()
                    .map(|rate| value.amount * rate)
            })
            .collect();
        (converted, warning)
    }

    async fn fetch_rate_ranges(
        &self,
        target: &str,
        source: &str,
        start: &str,
        end: &str,
    ) -> Result<(), String> {
        let start_day =
            civil_time::parse_date(start).ok_or_else(|| "invalid start date".to_string())?;
        let end_day = civil_time::parse_date(end).ok_or_else(|| "invalid end date".to_string())?;
        let mut cursor = start_day;
        while cursor <= end_day {
            let chunk_end = (cursor + MAX_RANGE_DAYS * 86_400).min(end_day);
            let response = self
                .frankfurter
                .rates
                .get_rates(RatesQuery {
                    base: Some(target.to_string()),
                    quotes: vec![source.to_string()],
                    from: Some(civil_time::format_date(cursor)),
                    to: Some(civil_time::format_date(chunk_end)),
                    ..RatesQuery::default()
                })
                .await
                .map_err(|error| error.to_string())?;
            let mut cached = Vec::new();
            for rate in response {
                if rate.rate > 0.0 {
                    cached.push((
                        rate.quote.to_ascii_uppercase(),
                        rate.base.to_ascii_uppercase(),
                        rate.date,
                        1.0 / rate.rate,
                    ));
                }
            }
            rate_repository::upsert_rates(&self.database, &cached, now())
                .map_err(|error| error.to_string())?;
            cursor = chunk_end + 86_400;
        }
        Ok(())
    }

    /// Convert every row in place and return the warning, if any.
    pub async fn convert_transactions(
        &self,
        transactions: &mut [ExchangeTransaction],
    ) -> Option<String> {
        let target = self.target_currency().ok()?;
        let values = transactions
            .iter()
            .filter_map(|transaction| {
                Some(FiatValue {
                    timestamp: transaction.timestamp,
                    amount: transaction.fiat_amount?,
                    currency: transaction.fiat_currency.clone().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            for transaction in transactions {
                transaction.converted_fiat_amount = None;
                transaction.converted_fiat_currency = None;
            }
            return None;
        }
        let (converted, warning) = self.convert_values(&target, &values).await;
        let mut converted_iter = converted.into_iter();
        for transaction in transactions {
            if transaction.fiat_amount.is_some() {
                if let Some(value) = converted_iter.next().flatten() {
                    transaction.converted_fiat_amount = Some(value);
                    transaction.converted_fiat_currency = Some(target.clone());
                } else {
                    transaction.converted_fiat_amount = None;
                    transaction.converted_fiat_currency = None;
                }
            }
        }
        warning
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
