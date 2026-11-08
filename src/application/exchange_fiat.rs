//! Historical fiat conversion for exchange ledger values.
//!
//! The current default currency is resolved from Settings. All conversions
//! go through a persistent rate cache; network calls happen outside any
//! SQLite guard and failures degrade to a warning rather than failing the
//! page.
use crate::clients::frankfurter::{Frankfurter, FrankfurterConfig, RatesQuery};
use crate::data::repositories::{fiat_rates as rate_repository, settings as settings_repository};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::exchange::{csv, ExchangeKpiTransaction, ExchangeTransaction};
use crate::domain::price::{normalize_default_symbol, USD_SYMBOL};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
const MAX_RANGE_DAYS: i64 = 366;
/// One fiat value that needs conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct FiatValue {
    pub timestamp: i64,
    pub amount: f64,
    pub currency: String,
}
/// Which derived field of one transaction a conversion result belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FiatField {
    Amount,
    Fee,
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
        Ok(normalize_default_symbol(&symbol))
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
                    warning = Some(format!("Could not read cached exchange rates: {error}"));
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
                        "Some historical exchange rates could not be loaded: {error}",
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
                        if let Err(error) = rate_repository::upsert_rate(
                            &self.database,
                            &base,
                            &target,
                            &rate.date,
                            converted,
                            civil_time::now_epoch_seconds(),
                        ) {
                            warning = Some(format!("Could not cache an exchange rate: {error}"));
                        }
                        if let Ok(Some(cached)) =
                            rate_repository::latest_rate(&self.database, &base, &target, &date)
                        {
                            rates.insert((base.clone(), date.clone()), cached.rate);
                        }
                    }
                    Ok(_) => {
                        warning = Some("An exchange rate was invalid and was skipped.".to_string());
                    }
                    Err(error) => {
                        warning = Some(format!(
                            "Some historical exchange rates could not be loaded: {error}",
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
            rate_repository::upsert_rates(&self.database, &cached, civil_time::now_epoch_seconds())
                .map_err(|error| error.to_string())?;
            cursor = chunk_end + 86_400;
        }
        Ok(())
    }
    /// Convert every fiat amount and fiat fee in place and return the warning,
    /// if any. BTC/XBT fees are left untouched: the table renders them as
    /// BTC, not as a fiat conversion.
    pub async fn convert_transactions(
        &self,
        transactions: &mut [ExchangeTransaction],
    ) -> Option<String> {
        let target = self.target_currency().ok()?;
        for transaction in transactions.iter_mut() {
            transaction.converted_fiat_amount = None;
            transaction.converted_fiat_currency = None;
            transaction.converted_fee_amount = None;
            transaction.converted_fee_currency = None;
        }
        let mut values = Vec::new();
        let mut assignments = Vec::new();
        for (index, transaction) in transactions.iter().enumerate() {
            if let (Some(amount), Some(currency)) = (
                transaction.fiat_amount,
                transaction.fiat_currency.as_deref(),
            ) {
                values.push(FiatValue {
                    timestamp: transaction.timestamp,
                    amount,
                    currency: currency.to_string(),
                });
                assignments.push((index, FiatField::Amount));
            }
            if let (Some(fee), Some(currency)) =
                (transaction.fee, transaction.fee_currency.as_deref())
            {
                if !csv::is_bitcoin_currency(currency) {
                    values.push(FiatValue {
                        timestamp: transaction.timestamp,
                        amount: fee,
                        currency: currency.to_string(),
                    });
                    assignments.push((index, FiatField::Fee));
                }
            }
        }
        if values.is_empty() {
            return None;
        }
        let (converted, warning) = self.convert_values(&target, &values).await;
        for ((index, field), value) in assignments.into_iter().zip(converted) {
            let Some(value) = value else {
                continue;
            };
            match field {
                FiatField::Amount => {
                    transactions[index].converted_fiat_amount = Some(value);
                    transactions[index].converted_fiat_currency = Some(target.clone());
                }
                FiatField::Fee => {
                    transactions[index].converted_fee_amount = Some(value);
                    transactions[index].converted_fee_currency = Some(target.clone());
                }
            }
        }
        warning
    }
    /// Convert the fiat amounts and fiat fees of the lean KPI projection into
    /// `target` and return the warning, if any. BTC/XBT fees stay satoshis.
    pub async fn convert_kpi_transactions(
        &self,
        target: &str,
        transactions: &mut [ExchangeKpiTransaction],
    ) -> Option<String> {
        for transaction in transactions.iter_mut() {
            transaction.converted_fiat_amount = None;
            transaction.converted_fee_amount = None;
        }
        let mut values = Vec::new();
        let mut assignments = Vec::new();
        for (index, transaction) in transactions.iter().enumerate() {
            if let (Some(amount), Some(currency)) = (
                transaction.fiat_amount,
                transaction.fiat_currency.as_deref(),
            ) {
                values.push(FiatValue {
                    timestamp: transaction.timestamp,
                    amount,
                    currency: currency.to_string(),
                });
                assignments.push((index, FiatField::Amount));
            }
            if let (Some(fee), Some(currency)) =
                (transaction.fee, transaction.fee_currency.as_deref())
            {
                if !csv::is_bitcoin_currency(currency) {
                    values.push(FiatValue {
                        timestamp: transaction.timestamp,
                        amount: fee,
                        currency: currency.to_string(),
                    });
                    assignments.push((index, FiatField::Fee));
                }
            }
        }
        if values.is_empty() {
            return None;
        }
        let (converted, warning) = self.convert_values(target, &values).await;
        for ((index, field), value) in assignments.into_iter().zip(converted) {
            let Some(value) = value else {
                continue;
            };
            match field {
                FiatField::Amount => {
                    transactions[index].converted_fiat_amount = Some(value);
                }
                FiatField::Fee => transactions[index].converted_fee_amount = Some(value),
            }
        }
        warning
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::repositories::fiat_rates as rate_repository;
    use crate::data::repositories::settings as settings_repository;
    use crate::domain::exchange::{ExchangeTxType, MatchState};
    use std::collections::BTreeMap;
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, RawWaker, RawWakerVTable, Waker};
        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn noop(_: *const ()) {}
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => output,
            std::task::Poll::Pending => panic!("test future must not pend"),
        }
    }
    fn transaction(
        timestamp: i64,
        fiat_amount: Option<f64>,
        fiat_currency: Option<&str>,
        fee: Option<f64>,
        fee_currency: Option<&str>,
    ) -> ExchangeTransaction {
        ExchangeTransaction {
            id: 1,
            exchange_id: 1,
            fingerprint: "fingerprint".into(),
            timestamp,
            tx_type: ExchangeTxType::Buy,
            amount_sats: 100_000_000,
            currency: "BTC".into(),
            fiat_amount,
            fiat_currency: fiat_currency.map(str::to_string),
            converted_fiat_amount: None,
            converted_fiat_currency: None,
            converted_fee_amount: None,
            converted_fee_currency: None,
            fee,
            fee_currency: fee_currency.map(str::to_string),
            tx_hash: None,
            description: None,
            raw_data: BTreeMap::new(),
            match_state: MatchState::Unmatched,
            matched_tx_id: None,
            on_chain_link: None,
            is_manually_edited: false,
            is_intentional_duplicate: false,
        }
    }
    #[test]
    fn converts_fiat_amounts_and_fiat_fees_to_the_default_currency() {
        let database = Database::open_in_memory().unwrap();
        settings_repository::save_default_fiat_currency(&database, "USD").unwrap();
        let timestamp = civil_time::epoch_seconds(2026, 3, 25, 12, 0, 0);
        let date = civil_time::format_date(timestamp);
        rate_repository::upsert_rate(&database, "EUR", "USD", &date, 1.1, 0).unwrap();
        let service = ExchangeFiatService::connect(database);
        let mut transaction =
            transaction(timestamp, Some(100.0), Some("EUR"), Some(2.0), Some("EUR"));
        let warning =
            block_on(service.convert_transactions(std::slice::from_mut(&mut transaction)));
        assert_eq!(warning, None);
        assert!((transaction.converted_fiat_amount.unwrap() - 110.0).abs() < 1e-9);
        assert_eq!(transaction.converted_fiat_currency.as_deref(), Some("USD"));
        assert!((transaction.converted_fee_amount.unwrap() - 2.2).abs() < 1e-9);
        assert_eq!(transaction.converted_fee_currency.as_deref(), Some("USD"));
    }
    #[test]
    fn converts_lean_kpi_rows_in_the_default_currency() {
        let database = Database::open_in_memory().unwrap();
        let timestamp = civil_time::epoch_seconds(2026, 3, 25, 12, 0, 0);
        let date = civil_time::format_date(timestamp);
        rate_repository::upsert_rate(&database, "EUR", "USD", &date, 1.1, 0).unwrap();
        let service = ExchangeFiatService::connect(database);
        let mut row = ExchangeKpiTransaction {
            timestamp,
            tx_type: ExchangeTxType::Buy,
            amount_sats: 100_000_000,
            fiat_amount: Some(100.0),
            fiat_currency: Some("EUR".into()),
            fee: Some(2.0),
            fee_currency: Some("EUR".into()),
            match_state: MatchState::Unmatched,
            converted_fiat_amount: None,
            converted_fee_amount: None,
        };
        let warning =
            block_on(service.convert_kpi_transactions("USD", std::slice::from_mut(&mut row)));
        assert_eq!(warning, None);
        assert!((row.converted_fiat_amount.unwrap() - 110.0).abs() < 1e-9);
        assert!((row.converted_fee_amount.unwrap() - 2.2).abs() < 1e-9);
    }
    #[test]
    fn leaves_btc_fees_in_btc() {
        let database = Database::open_in_memory().unwrap();
        let service = ExchangeFiatService::connect(database);
        let mut transaction = transaction(1_000, None, None, Some(5_000.0), Some("BTC"));
        let warning =
            block_on(service.convert_transactions(std::slice::from_mut(&mut transaction)));
        assert_eq!(warning, None);
        assert_eq!(transaction.converted_fee_amount, None);
        assert_eq!(transaction.converted_fee_currency, None);
    }
}
