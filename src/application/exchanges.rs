//! Exchange ledger service: CSV import, account/transaction workflows,
//! reconciliation, and manual on-chain linking.
use crate::application::exchange_fiat::{ExchangeFiatService, FiatValue};
use crate::application::prices::PriceService;
use crate::data::repositories::{
    addresses as address_repository, catalogs as catalog_repository,
    exchanges as exchange_repository, settings as settings_repository,
};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::exchange::{
    csv::{self, ParsedExchangeTx},
    matching::{self, OnChainClaims, OnChainLinkCandidate, OnChainTransaction},
    ExchangeAccount, ExchangeAccountDetail, ExchangeAccountList, ExchangeAccountTableQuery,
    ExchangeDuplicateChoice, ExchangeDuplicateInfo, ExchangeKpiMetric, ExchangeKpiTransaction,
    ExchangeKpis, ExchangeLinkOutcome, ExchangeReconcileBatchSummary, ExchangeReconcileFailure,
    ExchangeTransaction, ExchangeTransactionDraft, ExchangeTransactionSaveOutcome,
    ExchangeTransactionSaveRequest, ExchangeTransactionTableQuery, ExchangeTxType,
    ImportDataMapping, ImportDataPreview, ImportDataPreviewRow, ImportDataSummary, MatchState,
    OnChainLink, OnChainLinkCandidateView, OnChainLinkScoringSettings, OnChainLinkSource,
};
use crate::domain::price::{fiat_to_sats, sats_to_fiat, USD_SYMBOL};
use crate::domain::table::{SortDirection, TablePage};
use std::collections::BTreeMap;
const MAX_CSV_BYTES: usize = 25 * 1024 * 1024;
const MAX_CSV_ROWS: usize = 200_000;
const MAX_PAGE_SIZE: u32 = 100;
/// Largest number of rows one bulk action may address.
const MAX_BULK_IDS: usize = 100;
#[derive(Debug, thiserror::Error)]
pub enum ExchangeServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("exchange account not found")]
    AccountNotFound,
    #[error("exchange transaction not found")]
    TransactionNotFound,
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
}
pub struct ExchangeService {
    database: Database,
    fiat: ExchangeFiatService,
}
impl ExchangeService {
    pub fn connect(database: Database) -> Self {
        Self {
            fiat: ExchangeFiatService::connect(database.clone()),
            database,
        }
    }
    pub async fn list_accounts(
        &self,
        query: &ExchangeAccountTableQuery,
    ) -> Result<ExchangeAccountList, ExchangeServiceError> {
        let sort = parse_account_sort(query.table.sort_by.as_deref(), query.table.sort_dir)?;
        let page_size = query.table.page_size.clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from(query.table.page) * u64::from(page_size);
        let page = exchange_repository::list_accounts(&self.database, page_size, offset, sort)?;
        let mut transactions = exchange_repository::list_kpi_transactions(&self.database, None)?;
        let (kpis, conversion_warning) = self.kpis_for(&mut transactions).await?;
        Ok(ExchangeAccountList {
            page,
            kpis,
            conversion_warning,
        })
    }
    pub fn get_account(&self, id: i64) -> Result<ExchangeAccount, ExchangeServiceError> {
        exchange_repository::get_account(&self.database, id)?
            .ok_or(ExchangeServiceError::AccountNotFound)
    }
    pub async fn detail(&self, id: i64) -> Result<ExchangeAccountDetail, ExchangeServiceError> {
        let account = self.get_account(id)?;
        let mut transactions =
            exchange_repository::list_kpi_transactions(&self.database, Some(id))?;
        let (kpis, conversion_warning) = self.kpis_for(&mut transactions).await?;
        Ok(ExchangeAccountDetail {
            account,
            kpis,
            conversion_warning,
        })
    }
    /// Value the lean ledger projection in the current default currency. Both
    /// the exchanges list and the account detail use this path so their KPI
    /// cards always describe the same population the same way.
    async fn kpis_for(
        &self,
        transactions: &mut [ExchangeKpiTransaction],
    ) -> Result<(ExchangeKpis, Option<String>), ExchangeServiceError> {
        let target = self.fiat.target_currency()?;
        let mut warnings = Vec::new();
        if let Some(warning) = self
            .fiat
            .convert_kpi_transactions(&target, transactions)
            .await
        {
            warnings.push(warning);
        }
        let prices = PriceService::connect(self.database.clone());
        let (btc_prices, price_warning) = btc_prices(&prices, transactions, &target).await;
        if let Some(warning) = price_warning {
            warnings.push(warning);
        }
        let kpis = kpis(transactions, &target, &btc_prices);
        let conversion_warning = (!warnings.is_empty()).then(|| warnings.join(" "));
        Ok((kpis, conversion_warning))
    }
    pub async fn list_transactions(
        &self,
        query: &ExchangeTransactionTableQuery,
    ) -> Result<TablePage<ExchangeTransaction>, ExchangeServiceError> {
        self.get_account(query.exchange_id)?;
        let sort = parse_transaction_sort(query.table.sort_by.as_deref(), query.table.sort_dir)?;
        let page_size = query.table.page_size.clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from(query.table.page) * u64::from(page_size);
        let mut page = exchange_repository::list_transactions(
            &self.database,
            &exchange_repository::TransactionPageQuery {
                exchange_id: query.exchange_id,
                limit: page_size,
                offset,
                sort,
            },
        )?;
        self.fiat.convert_transactions(&mut page.items).await;
        Ok(page)
    }
    pub fn create_account(
        &self,
        name: &str,
        provider_id: Option<i64>,
    ) -> Result<ExchangeAccount, ExchangeServiceError> {
        let name = validate_name(name)?;
        self.validate_provider(provider_id)?;
        Ok(exchange_repository::create_account(
            &self.database,
            &name,
            provider_id,
        )?)
    }
    pub fn update_account(
        &self,
        id: i64,
        name: &str,
        provider_id: Option<i64>,
    ) -> Result<ExchangeAccount, ExchangeServiceError> {
        let name = validate_name(name)?;
        self.validate_provider(provider_id)?;
        Ok(exchange_repository::update_account(
            &self.database,
            id,
            &name,
            provider_id,
        )?)
    }
    /// Delete several exchange accounts atomically; their ledger rows
    /// cascade and the affected provider balances are refreshed once.
    pub fn delete_accounts(&self, ids: &[i64]) -> Result<usize, ExchangeServiceError> {
        let ids = normalize_bulk_ids(ids, "exchange")?;
        Ok(exchange_repository::delete_accounts(&self.database, &ids)?)
    }
    pub async fn preview_import_data(
        &self,
        csv_text: &str,
        mapping: Option<&ImportDataMapping>,
    ) -> Result<ImportDataPreview, ExchangeServiceError> {
        let document = parse_document(csv_text)?;
        let detected = csv::detect_parser(&document.headers);
        let (parsed, detected_id, detected_name) = parse_for_request(&document, detected, mapping)?;
        let skipped_non_btc = parsed
            .iter()
            .filter(|row| !csv::is_bitcoin_currency(&row.currency))
            .count();
        let btc_rows = parsed
            .into_iter()
            .filter(|row| csv::is_bitcoin_currency(&row.currency))
            .collect::<Vec<_>>();
        let target = self.fiat.target_currency()?;
        let preview_slice = btc_rows.iter().take(5).collect::<Vec<_>>();
        let values = preview_slice
            .iter()
            .filter_map(|row| {
                Some(FiatValue {
                    timestamp: row.timestamp,
                    amount: row.fiat_amount?,
                    currency: row.fiat_currency.clone().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        let (converted, warning) = self.fiat.convert_values(&target, &values).await;
        let mut converted_iter = converted.into_iter();
        let preview = preview_slice
            .iter()
            .map(|row| {
                let converted = if row.fiat_amount.is_some() {
                    converted_iter.next().flatten()
                } else {
                    None
                };
                ImportDataPreviewRow {
                    timestamp: row.timestamp,
                    tx_type: row.tx_type,
                    amount_sats: row.amount.round() as i64,
                    currency: row.currency.clone(),
                    converted_fiat_amount: converted.or(row.fiat_amount),
                    converted_fiat_currency: if converted.is_some() {
                        Some(target.clone())
                    } else {
                        row.fiat_currency.clone()
                    },
                    tx_hash: row.tx_hash.clone(),
                }
            })
            .collect();
        Ok(ImportDataPreview {
            headers: document.headers,
            row_count: document.rows.len(),
            detected_parser_id: detected_id,
            detected_parser_name: detected_name,
            parsed_count: btc_rows.len(),
            skipped_non_btc,
            preview,
            converted_fiat_currency: target,
            conversion_warning: warning,
        })
    }
    pub async fn import_data(
        &self,
        exchange_id: Option<i64>,
        name: Option<&str>,
        provider_id: Option<i64>,
        csv_text: &str,
        mapping: Option<&ImportDataMapping>,
    ) -> Result<ImportDataSummary, ExchangeServiceError> {
        let document = parse_document(csv_text)?;
        let detected = csv::detect_parser(&document.headers);
        let (parsed, _, _) = parse_for_request(&document, detected, mapping)?;
        let skipped_non_btc = parsed
            .iter()
            .filter(|row| !csv::is_bitcoin_currency(&row.currency))
            .count();
        let btc_rows = parsed
            .into_iter()
            .filter(|row| csv::is_bitcoin_currency(&row.currency))
            .collect::<Vec<_>>();
        let created = exchange_id.is_none();
        let account = if let Some(id) = exchange_id {
            self.get_account(id)?
        } else {
            let name = name.ok_or_else(|| {
                ExchangeServiceError::Validation("Enter an exchange name.".to_string())
            })?;
            self.create_account(name, provider_id)?
        };
        if btc_rows.is_empty() {
            return Ok(ImportDataSummary {
                exchange_id: account.id,
                created,
                inserted: 0,
                duplicates: 0,
                auto_linked: 0,
                skipped_non_btc,
                conversion_warning: None,
            });
        }
        let exchange_repository::ExistingIdentities {
            fingerprints: mut existing_fingerprints,
            linked_txids,
            shareable_txids,
        } = exchange_repository::existing_identities(&self.database, account.id)?;
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring = settings_repository::load_on_chain_link_scoring(&self.database)?;
        let mut claims = OnChainClaims::from_linked_txids(&linked_txids);
        for txid in &shareable_txids {
            claims.claim(txid.clone(), true);
        }
        let mut writes = Vec::new();
        let mut duplicates = 0;
        let mut auto_linked = 0;
        let now = civil_time::now_epoch_seconds();
        for parsed in btc_rows {
            if existing_fingerprints.contains(&parsed.fingerprint) {
                duplicates += 1;
                continue;
            }
            existing_fingerprints.insert(parsed.fingerprint.clone());
            let mut write = write_from_parsed(&parsed);
            let temporary = ExchangeTransaction {
                id: 0,
                exchange_id: account.id,
                fingerprint: parsed.fingerprint.clone(),
                timestamp: parsed.timestamp,
                tx_type: parsed.tx_type,
                amount_sats: write.amount_sats,
                currency: parsed.currency.clone(),
                fiat_amount: parsed.fiat_amount,
                fiat_currency: parsed.fiat_currency.clone(),
                converted_fiat_amount: None,
                converted_fiat_currency: None,
                converted_fee_amount: None,
                converted_fee_currency: None,
                fee: parsed.fee,
                fee_currency: parsed.fee_currency.clone(),
                tx_hash: parsed.tx_hash.clone(),
                description: parsed.description.clone(),
                raw_data: parsed.raw_data.clone(),
                match_state: MatchState::Unmatched,
                matched_tx_id: None,
                on_chain_link: None,
                is_manually_edited: false,
                is_intentional_duplicate: false,
            };
            let result = matching::match_transaction(&temporary, &on_chain, &scoring, &claims, now);
            let state = result.match_state;
            if let Some(link) = result.on_chain_link {
                write.match_state = state;
                write.matched_tx_id = Some(link.txid.clone());
                claims.claim(link.txid.clone(), temporary.is_intentional_duplicate);
                write.on_chain_link = Some(link);
                auto_linked += 1;
            }
            writes.push(write);
        }
        exchange_repository::insert_imported_transactions(
            &self.database,
            account.id,
            &writes,
            now,
        )?;
        let inserted = writes.len();
        Ok(ImportDataSummary {
            exchange_id: account.id,
            created,
            inserted,
            duplicates,
            auto_linked,
            skipped_non_btc,
            conversion_warning: None,
        })
    }
    pub async fn save_transaction(
        &self,
        request: &ExchangeTransactionSaveRequest,
    ) -> Result<ExchangeTransactionSaveOutcome, ExchangeServiceError> {
        self.get_account(request.exchange_id)?;
        let draft = &request.draft;
        if draft.timestamp <= 0 {
            return Err(ExchangeServiceError::Validation(
                "Enter a valid transaction date and time.".to_string(),
            ));
        }
        let currency = draft.currency.trim().to_ascii_uppercase();
        if !csv::is_bitcoin_currency(&currency) {
            return Err(ExchangeServiceError::Validation(
                "Only BTC/XBT exchange transactions are supported.".to_string(),
            ));
        }
        let tx_hash = draft
            .tx_hash
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let fingerprint = exchange_repository::fingerprint_for(
            draft.timestamp,
            draft.amount_sats,
            &currency,
            draft.tx_type,
            tx_hash.as_deref(),
        );
        let existing = request
            .transaction_id
            .map(|id| {
                exchange_repository::get_transaction(&self.database, id)?
                    .ok_or(ExchangeServiceError::TransactionNotFound)
            })
            .transpose()?;
        let duplicate = exchange_repository::find_by_fingerprint(
            &self.database,
            request.exchange_id,
            &fingerprint,
            request.transaction_id,
        )?;
        if let (Some(duplicate), None) = (&duplicate, request.duplicate_choice) {
            return Ok(ExchangeTransactionSaveOutcome::Duplicate(duplicate_info(
                duplicate,
            )));
        }
        let replace_id = match request.duplicate_choice {
            Some(ExchangeDuplicateChoice::Replace) => duplicate.as_ref().map(|row| row.id),
            _ => None,
        };
        let mut write = ExchangeTransactionWriteBuilder::new(
            fingerprint,
            draft.timestamp,
            draft.tx_type,
            draft.amount_sats,
            currency,
            draft.fiat_amount,
            draft.fiat_currency.clone(),
            csv::normalize_fee(draft.fee, draft.fee_currency.as_deref()),
            draft.fee_currency.clone(),
            tx_hash,
            draft.description.clone(),
            existing
                .as_ref()
                .map(|row| row.raw_data.clone())
                .unwrap_or_else(|| manual_raw_data(draft)),
        )
        .build();
        write.is_manually_edited = true;
        write.is_intentional_duplicate = match request.duplicate_choice {
            Some(ExchangeDuplicateChoice::Replace) => false,
            Some(ExchangeDuplicateChoice::KeepBoth) => true,
            None => existing
                .as_ref()
                .is_some_and(|row| row.is_intentional_duplicate),
        };
        if let Some(existing) = &existing {
            let match_relevant_changed = existing.timestamp != write.timestamp
                || existing.amount_sats != write.amount_sats
                || existing.currency != write.currency
                || existing.tx_type != write.tx_type
                || existing.tx_hash != write.tx_hash;
            if existing.match_state == MatchState::ManualMatch {
                write.match_state = MatchState::ManualMatch;
                write.matched_tx_id = existing.matched_tx_id.clone();
                write.on_chain_link = existing.on_chain_link.clone();
            } else if !match_relevant_changed {
                write.match_state = existing.match_state;
                write.matched_tx_id = existing.matched_tx_id.clone();
                write.on_chain_link = existing.on_chain_link.clone();
            }
        }
        let saved = exchange_repository::save_transaction(
            &self.database,
            request.exchange_id,
            request.transaction_id,
            replace_id,
            &write,
        )?;
        let mut saved = saved;
        self.fiat
            .convert_transactions(std::slice::from_mut(&mut saved))
            .await;
        Ok(ExchangeTransactionSaveOutcome::Saved(Box::new(saved)))
    }
    /// Delete several ledger transactions of one account atomically.
    pub fn delete_transactions(
        &self,
        exchange_id: i64,
        ids: &[i64],
    ) -> Result<usize, ExchangeServiceError> {
        let ids = normalize_bulk_ids(ids, "transaction")?;
        Ok(exchange_repository::delete_transactions(
            &self.database,
            exchange_id,
            &ids,
        )?)
    }
    pub async fn get_transaction(
        &self,
        exchange_id: i64,
        transaction_id: i64,
    ) -> Result<ExchangeTransaction, ExchangeServiceError> {
        let mut transaction = exchange_repository::get_transaction(&self.database, transaction_id)?
            .ok_or(ExchangeServiceError::TransactionNotFound)?;
        if transaction.exchange_id != exchange_id {
            return Err(ExchangeServiceError::TransactionNotFound);
        }
        self.fiat
            .convert_transactions(std::slice::from_mut(&mut transaction))
            .await;
        Ok(transaction)
    }
    pub async fn reconcile(&self, exchange_id: i64) -> Result<(), ExchangeServiceError> {
        self.get_account(exchange_id)?;
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring = settings_repository::load_on_chain_link_scoring(&self.database)?;
        self.reconcile_account(exchange_id, &on_chain, &scoring)
    }
    /// Reconcile several exchange accounts in one request.
    ///
    /// The on-chain transaction cache and scoring settings are loaded once.
    /// Each account is matched and updated independently: a failure is
    /// reported for that account and the remaining accounts still run.
    pub fn reconcile_many(
        &self,
        ids: &[i64],
    ) -> Result<ExchangeReconcileBatchSummary, ExchangeServiceError> {
        let ids = normalize_bulk_ids(ids, "exchange")?;
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring = settings_repository::load_on_chain_link_scoring(&self.database)?;
        let mut summary = ExchangeReconcileBatchSummary::default();
        for exchange_id in ids {
            let name = match self.get_account(exchange_id) {
                Ok(account) => account.name,
                Err(ExchangeServiceError::AccountNotFound) => {
                    summary.failed.push(ExchangeReconcileFailure {
                        exchange_id,
                        name: None,
                        error: "exchange account not found".to_string(),
                    });
                    continue;
                }
                Err(error) => return Err(error),
            };
            match self.reconcile_account(exchange_id, &on_chain, &scoring) {
                Ok(()) => summary.reconciled += 1,
                Err(error) => summary.failed.push(ExchangeReconcileFailure {
                    exchange_id,
                    name: Some(name),
                    error: error.to_string(),
                }),
            }
        }
        Ok(summary)
    }
    fn reconcile_account(
        &self,
        exchange_id: i64,
        on_chain: &[OnChainTransaction],
        scoring: &OnChainLinkScoringSettings,
    ) -> Result<(), ExchangeServiceError> {
        let transactions =
            exchange_repository::list_account_transactions(&self.database, exchange_id)?;
        let mut claims = OnChainClaims::new();
        for transaction in &transactions {
            if transaction.match_state == MatchState::ManualMatch {
                if let Some(txid) = transaction_link_txid(transaction) {
                    claims.claim(txid, transaction.is_intentional_duplicate);
                }
            }
        }
        let now = civil_time::now_epoch_seconds();
        let mut updates = Vec::new();
        for transaction in transactions {
            if transaction.match_state == MatchState::ManualMatch {
                continue;
            }
            let mut result =
                matching::match_transaction(&transaction, on_chain, scoring, &claims, now);
            if let (Some(existing), Some(next)) =
                (&transaction.on_chain_link, result.on_chain_link.as_mut())
            {
                if existing.txid == next.txid && existing.source == next.source {
                    next.linked_at = existing.linked_at;
                }
            }
            if let Some(link) = &result.on_chain_link {
                claims.claim(link.txid.clone(), transaction.is_intentional_duplicate);
            }
            if result.match_state != transaction.match_state
                || result.matched_tx_id != transaction.matched_tx_id
                || result.on_chain_link != transaction.on_chain_link
            {
                updates.push((
                    transaction.id,
                    result.match_state,
                    result.matched_tx_id,
                    result.on_chain_link,
                ));
            }
        }
        exchange_repository::update_match_states(&self.database, exchange_id, &updates)?;
        Ok(())
    }
    pub fn candidates(
        &self,
        transaction_id: i64,
        search: &str,
    ) -> Result<Vec<OnChainLinkCandidateView>, ExchangeServiceError> {
        let transaction = exchange_repository::get_transaction(&self.database, transaction_id)?
            .ok_or(ExchangeServiceError::TransactionNotFound)?;
        let identities =
            exchange_repository::existing_identities(&self.database, transaction.exchange_id)?;
        let mut linked = identities.linked_txids;
        if let Some(txid) = transaction_link_txid(&transaction) {
            linked.remove(&txid);
        }
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring = settings_repository::load_on_chain_link_scoring(&self.database)?;
        Ok(
            matching::candidates(&transaction, &on_chain, search, &scoring, &linked)
                .into_iter()
                .map(|candidate| candidate_view(&self.database, candidate))
                .collect(),
        )
    }
    pub fn link(
        &self,
        transaction_id: i64,
        txid: &str,
        allow_duplicate: bool,
    ) -> Result<ExchangeLinkOutcome, ExchangeServiceError> {
        let transaction = exchange_repository::get_transaction(&self.database, transaction_id)?
            .ok_or(ExchangeServiceError::TransactionNotFound)?;
        if !matching::is_on_chain_linkable(transaction.tx_type) {
            return Err(ExchangeServiceError::Validation(
                "Only deposits and withdrawals can be linked.".to_string(),
            ));
        }
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let candidate = on_chain
            .iter()
            .find(|candidate| candidate.txid == txid)
            .ok_or_else(|| {
                ExchangeServiceError::Validation("On-chain transaction not found.".into())
            })?;
        let duplicate = exchange_repository::list_account_transactions(
            &self.database,
            transaction.exchange_id,
        )?
        .into_iter()
        .find(|row| {
            row.id != transaction.id
                && row
                    .on_chain_link
                    .as_ref()
                    .map(|link| link.txid.as_str())
                    .or(row.matched_tx_id.as_deref())
                    == Some(txid)
        });
        if let (Some(duplicate), false) = (&duplicate, allow_duplicate) {
            return Ok(ExchangeLinkOutcome::Duplicate(duplicate_info(duplicate)));
        }
        let link = OnChainLink {
            txid: txid.to_string(),
            local_addresses: candidate.local_addresses.clone(),
            source: OnChainLinkSource::Manual,
            linked_at: civil_time::now_epoch_seconds(),
        };
        let updated = exchange_repository::set_link(
            &self.database,
            transaction.exchange_id,
            transaction.id,
            MatchState::ManualMatch,
            Some(txid),
            Some(&link),
        )?;
        Ok(ExchangeLinkOutcome::Linked(Box::new(updated)))
    }
    pub fn clear_link(
        &self,
        transaction_id: i64,
    ) -> Result<ExchangeTransaction, ExchangeServiceError> {
        let transaction = exchange_repository::get_transaction(&self.database, transaction_id)?
            .ok_or(ExchangeServiceError::TransactionNotFound)?;
        Ok(exchange_repository::set_link(
            &self.database,
            transaction.exchange_id,
            transaction.id,
            MatchState::Unmatched,
            None,
            None,
        )?)
    }
    fn validate_provider(&self, provider_id: Option<i64>) -> Result<(), ExchangeServiceError> {
        if let Some(provider_id) = provider_id {
            if !catalog_repository::exchange_type_exists(&self.database, provider_id)? {
                return Err(ExchangeServiceError::Validation(
                    "Choose an existing exchange provider.".to_string(),
                ));
            }
        }
        Ok(())
    }
}
fn validate_name(name: &str) -> Result<String, ExchangeServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ExchangeServiceError::Validation(
            "Enter an exchange name.".to_string(),
        ));
    }
    if name.chars().count() > 200 {
        return Err(ExchangeServiceError::Validation(
            "The exchange name is limited to 200 characters.".to_string(),
        ));
    }
    Ok(name.to_string())
}
/// Deduplicate and validate a batch selection before it reaches the
/// repository. Sorting also gives the SQL parameters a deterministic order.
fn normalize_bulk_ids(ids: &[i64], noun: &str) -> Result<Vec<i64>, ExchangeServiceError> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Err(ExchangeServiceError::Validation(format!(
            "Select at least one {noun}."
        )));
    }
    if ids.len() > MAX_BULK_IDS {
        return Err(ExchangeServiceError::Validation(format!(
            "Select at most {MAX_BULK_IDS} rows at once."
        )));
    }
    Ok(ids)
}
fn parse_account_sort(
    key: Option<&str>,
    direction: SortDirection,
) -> Result<
    Option<(
        exchange_repository::ExchangeAccountSortColumn,
        SortDirection,
    )>,
    ExchangeServiceError,
> {
    match key {
        None => Ok(None),
        Some(key) => exchange_repository::ExchangeAccountSortColumn::from_key(key)
            .map(|column| Some((column, direction)))
            .ok_or_else(|| ExchangeServiceError::Validation(format!("unknown sort column: {key}"))),
    }
}
fn parse_transaction_sort(
    key: Option<&str>,
    direction: SortDirection,
) -> Result<
    Option<(
        exchange_repository::ExchangeTransactionSortColumn,
        SortDirection,
    )>,
    ExchangeServiceError,
> {
    match key {
        None => Ok(None),
        Some(key) => exchange_repository::ExchangeTransactionSortColumn::from_key(key)
            .map(|column| Some((column, direction)))
            .ok_or_else(|| ExchangeServiceError::Validation(format!("unknown sort column: {key}"))),
    }
}
fn parse_document(text: &str) -> Result<csv::CsvDocument, ExchangeServiceError> {
    if text.len() > MAX_CSV_BYTES {
        return Err(ExchangeServiceError::Validation(
            "The CSV file is too large.".to_string(),
        ));
    }
    let document = csv::parse_csv_text(text).map_err(ExchangeServiceError::Validation)?;
    if document.rows.len() > MAX_CSV_ROWS {
        return Err(ExchangeServiceError::Validation(
            "The CSV file contains too many rows.".to_string(),
        ));
    }
    Ok(document)
}
type ParsedCsv = (Vec<ParsedExchangeTx>, Option<String>, Option<String>);
fn parse_for_request(
    document: &csv::CsvDocument,
    detected: Option<csv::ParserKind>,
    mapping: Option<&ImportDataMapping>,
) -> Result<ParsedCsv, ExchangeServiceError> {
    if let Some(mapping) = mapping.filter(|mapping| mapping.is_complete()) {
        return Ok((csv::parse_manual_rows(&document.rows, mapping), None, None));
    }
    if let Some(kind) = detected {
        let info = kind.info();
        return Ok((
            csv::parse_rows(kind, &document.rows),
            Some(info.id.to_string()),
            Some(info.name.to_string()),
        ));
    }
    Ok((Vec::new(), None, None))
}
fn write_from_parsed(parsed: &ParsedExchangeTx) -> exchange_repository::ExchangeTransactionWrite {
    ExchangeTransactionWriteBuilder::new(
        parsed.fingerprint.clone(),
        parsed.timestamp,
        parsed.tx_type,
        parsed.amount.round() as i64,
        parsed.currency.clone(),
        parsed.fiat_amount,
        parsed.fiat_currency.clone(),
        csv::normalize_fee(parsed.fee, parsed.fee_currency.as_deref()),
        parsed.fee_currency.clone(),
        parsed.tx_hash.clone(),
        parsed.description.clone(),
        parsed.raw_data.clone(),
    )
    .build()
}
struct ExchangeTransactionWriteBuilder {
    write: exchange_repository::ExchangeTransactionWrite,
}
impl ExchangeTransactionWriteBuilder {
    #[allow(clippy::too_many_arguments)]
    fn new(
        fingerprint: String,
        timestamp: i64,
        tx_type: ExchangeTxType,
        amount_sats: i64,
        currency: String,
        fiat_amount: Option<f64>,
        fiat_currency: Option<String>,
        fee: Option<f64>,
        fee_currency: Option<String>,
        tx_hash: Option<String>,
        description: Option<String>,
        raw_data: BTreeMap<String, String>,
    ) -> Self {
        Self {
            write: exchange_repository::ExchangeTransactionWrite {
                fingerprint,
                timestamp,
                tx_type,
                amount_sats,
                currency,
                fiat_amount,
                fiat_currency,
                fee,
                fee_currency,
                tx_hash,
                description,
                raw_data,
                match_state: MatchState::Unmatched,
                matched_tx_id: None,
                on_chain_link: None,
                is_manually_edited: false,
                is_intentional_duplicate: false,
            },
        }
    }
    fn build(self) -> exchange_repository::ExchangeTransactionWrite {
        self.write
    }
}
fn manual_raw_data(draft: &ExchangeTransactionDraft) -> BTreeMap<String, String> {
    let mut raw = BTreeMap::new();
    raw.insert("source".into(), "manual-entry".into());
    raw.insert(
        "createdAt".into(),
        civil_time::format_datetime(civil_time::now_epoch_seconds()),
    );
    raw.insert("timestamp".into(), draft.timestamp.to_string());
    raw.insert("type".into(), draft.tx_type.as_str().into());
    raw.insert("amount".into(), draft.amount_sats.to_string());
    raw.insert("currency".into(), draft.currency.clone());
    if let Some(value) = draft.fiat_amount {
        raw.insert("fiatAmount".into(), value.to_string());
    }
    if let Some(value) = &draft.fiat_currency {
        raw.insert("fiatCurrency".into(), value.clone());
    }
    if let Some(value) = draft.fee {
        raw.insert("fee".into(), value.to_string());
    }
    if let Some(value) = &draft.fee_currency {
        raw.insert("feeCurrency".into(), value.clone());
    }
    if let Some(value) = &draft.tx_hash {
        raw.insert("txHash".into(), value.clone());
    }
    if let Some(value) = &draft.description {
        raw.insert("description".into(), value.clone());
    }
    raw
}
fn duplicate_info(transaction: &ExchangeTransaction) -> ExchangeDuplicateInfo {
    ExchangeDuplicateInfo {
        transaction_id: transaction.id,
        tx_type: transaction.tx_type,
        amount_sats: transaction.amount_sats,
        currency: transaction.currency.clone(),
        timestamp: transaction.timestamp,
        tx_hash: transaction.tx_hash.clone(),
        description: transaction.description.clone(),
    }
}
fn candidate_view(
    database: &Database,
    candidate: OnChainLinkCandidate,
) -> OnChainLinkCandidateView {
    let local_addresses = candidate
        .local_addresses
        .into_iter()
        .map(|address| {
            let label = address_repository::get_address(database, &address)
                .ok()
                .flatten()
                .and_then(|row| row.description);
            crate::domain::exchange::ExchangeCandidateAddress { address, label }
        })
        .collect();
    OnChainLinkCandidateView {
        txid: candidate.txid,
        local_addresses,
        source: candidate.source,
        likeliness: candidate.likeliness,
        amount_delta_sats: candidate.amount_delta_sats,
        timestamp_delta_seconds: candidate.timestamp_delta_seconds,
        known_movement_sats: candidate.known_movement_sats,
        fee_sats: candidate.fee_sats,
        block_time: candidate.block_time,
        is_already_linked: candidate.is_already_linked,
    }
}
fn transaction_link_txid(transaction: &ExchangeTransaction) -> Option<String> {
    transaction
        .on_chain_link
        .as_ref()
        .map(|link| link.txid.clone())
        .or_else(|| transaction.matched_tx_id.clone())
}
/// Prepare the default-currency price history once per KPI request.
/// `ensure_currency_history` is idempotent and returns immediately once the
/// stored USD series has been materialized in `target`.
async fn prepare_currency_history(prices: &PriceService, target: &str) -> Option<String> {
    if target == USD_SYMBOL {
        return None;
    }
    match prices.ensure_currency_history(target).await {
        Ok(summary) => summary.warning,
        Err(error) => Some(format!(
            "Could not prepare {target} BTC price history: {error}."
        )),
    }
}
/// Whether one ledger row needs a historical BTC price. Rows with an already
/// converted fiat consideration and no fee can reuse that value; everything
/// else is valued at the transaction timestamp.
fn needs_btc_price(transaction: &ExchangeKpiTransaction) -> bool {
    if transaction.converted_fiat_amount.is_none() {
        return true;
    }
    let fee = transaction.fee.unwrap_or(0.0).abs();
    if fee == 0.0 {
        return false;
    }
    transaction
        .fee_currency
        .as_deref()
        .is_some_and(csv::is_bitcoin_currency)
        || transaction.converted_fee_amount.is_some()
}
/// Resolve the BTC price in `target` for every transaction timestamp that
/// needs valuation. One price is resolved per UTC day and shared by all rows of
/// that day, keeping a large ledger from issuing one lookup per row.
/// `PriceService::get_price` reads the stored history first and persists any
/// missing day it resolves, so repeated requests hit the database.
async fn btc_prices(
    prices: &PriceService,
    transactions: &[ExchangeKpiTransaction],
    target: &str,
) -> (BTreeMap<i64, f64>, Option<String>) {
    let mut timestamps_by_day = BTreeMap::<i64, Vec<i64>>::new();
    for transaction in transactions
        .iter()
        .filter(|transaction| needs_btc_price(transaction))
    {
        let day = transaction.timestamp.div_euclid(86_400) * 86_400;
        timestamps_by_day
            .entry(day)
            .or_default()
            .push(transaction.timestamp);
    }
    if timestamps_by_day.is_empty() {
        return (BTreeMap::new(), None);
    }
    let mut warning = prepare_currency_history(prices, target).await;
    let mut values = BTreeMap::new();
    for timestamps in timestamps_by_day.values() {
        let Some(&representative) = timestamps.first() else {
            continue;
        };
        match prices.get_price(target, representative).await {
            Ok(price) => {
                for timestamp in timestamps {
                    values.insert(*timestamp, price.close);
                }
            }
            Err(error) => {
                if warning.is_none() {
                    warning = Some(format!(
                        "Some ledger values could not be valued in {target}: {error}.",
                    ));
                }
            }
        }
    }
    (values, warning)
}
fn add_kpi_metric(metric: &mut ExchangeKpiMetric, sats: i64, fiat: Option<f64>) {
    metric.sats += sats;
    metric.fiat += fiat.unwrap_or(0.0);
    metric.count += 1;
}
fn kpis(
    transactions: &[ExchangeKpiTransaction],
    target: &str,
    btc_prices: &BTreeMap<i64, f64>,
) -> ExchangeKpis {
    let mut kpis = ExchangeKpis {
        fiat_currency: target.to_string(),
        ..ExchangeKpis::default()
    };
    for transaction in transactions {
        let sats = transaction.amount_sats.abs();
        let fiat = transaction.converted_fiat_amount.map(f64::abs).or_else(|| {
            btc_prices
                .get(&transaction.timestamp)
                .map(|price| sats_to_fiat(sats, *price))
        });
        match transaction.tx_type {
            ExchangeTxType::Buy => add_kpi_metric(&mut kpis.bought, sats, fiat),
            ExchangeTxType::Sell => add_kpi_metric(&mut kpis.sold, sats, fiat),
            ExchangeTxType::Reward => add_kpi_metric(&mut kpis.rewarded, sats, fiat),
            ExchangeTxType::Withdrawal => add_kpi_metric(&mut kpis.withdrawn, sats, fiat),
            ExchangeTxType::Deposit => add_kpi_metric(&mut kpis.deposited, sats, fiat),
            ExchangeTxType::Unknown => {}
        }
        if matches!(
            transaction.tx_type,
            ExchangeTxType::Withdrawal | ExchangeTxType::Deposit
        ) {
            kpis.onchain_total_count += 1;
            if transaction.match_state != MatchState::Unmatched {
                add_kpi_metric(&mut kpis.onchain_matched, sats, fiat);
            }
        }
        let fee = transaction.fee.unwrap_or(0.0).abs();
        if fee == 0.0 {
            continue;
        }
        kpis.fees.count += 1;
        let price = btc_prices.get(&transaction.timestamp).copied();
        if transaction
            .fee_currency
            .as_deref()
            .is_some_and(csv::is_bitcoin_currency)
        {
            let fee_sats = fee.round() as i64;
            kpis.fees.sats += fee_sats;
            kpis.fees.fiat += price
                .map(|price| sats_to_fiat(fee_sats, price))
                .unwrap_or(0.0);
        } else if let Some(fee_fiat) = transaction.converted_fee_amount.map(f64::abs) {
            kpis.fees.fiat += fee_fiat;
            if let Some(price) = price {
                kpis.fees.sats += fiat_to_sats(fee_fiat, price).unwrap_or(0);
            }
        }
    }
    kpis
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::mempool::bitcoin::types::{Tx, TxStatus, TxVin, Vout};
    use crate::data::repositories::transactions as transaction_repository;
    use crate::domain::address::StoredAddress;
    const TEST_ADDRESS: &str = "bc1qtestimport";
    const TEST_BLOCK_TIME: i64 = 1_700_000_000;
    fn on_chain_tx(txid: &str, output_value: i64, block_time: i64) -> Tx {
        Tx {
            txid: txid.into(),
            version: 2,
            locktime: 0,
            vin: vec![TxVin {
                txid: "funding".into(),
                vout: 0,
                prevout: None,
                scriptsig: String::new(),
                scriptsig_asm: String::new(),
                is_coinbase: false,
                sequence: 0,
            }],
            vout: vec![Vout {
                scriptpubkey: String::new(),
                scriptpubkey_asm: String::new(),
                scriptpubkey_type: "v0_p2wpkh".into(),
                scriptpubkey_address: Some(TEST_ADDRESS.into()),
                value: output_value,
            }],
            size: 100,
            weight: 400,
            fee: 250,
            status: TxStatus {
                confirmed: true,
                block_height: Some(1),
                block_hash: Some("block".into()),
                block_time: Some(block_time),
            },
        }
    }
    fn seed_on_chain_transaction(database: &Database, tx: Tx) {
        address_repository::insert_address(
            database,
            &StoredAddress {
                address: TEST_ADDRESS.into(),
                ..StoredAddress::default()
            },
        )
        .unwrap();
        transaction_repository::sync_transactions(database, TEST_ADDRESS, &[tx], false).unwrap();
    }
    fn import_mapping() -> ImportDataMapping {
        ImportDataMapping {
            timestamp: "Date".into(),
            amount: "Amount".into(),
            currency: "Currency".into(),
            tx_type: "Type".into(),
            ..ImportDataMapping::default()
        }
    }
    fn withdrawal_draft(timestamp: i64) -> ExchangeTransactionDraft {
        ExchangeTransactionDraft {
            timestamp,
            tx_type: ExchangeTxType::Withdrawal,
            amount_sats: 100_000,
            currency: "BTC".into(),
            fiat_amount: None,
            fiat_currency: None,
            fee: None,
            fee_currency: None,
            tx_hash: None,
            description: None,
        }
    }
    fn save_withdrawal(
        service: &ExchangeService,
        exchange_id: i64,
        timestamp: i64,
    ) -> ExchangeTransaction {
        let request = ExchangeTransactionSaveRequest {
            exchange_id,
            transaction_id: None,
            draft: withdrawal_draft(timestamp),
            duplicate_choice: None,
        };
        match block_on(service.save_transaction(&request)).unwrap() {
            ExchangeTransactionSaveOutcome::Saved(row) => *row,
            ExchangeTransactionSaveOutcome::Duplicate(info) => {
                panic!("unexpected duplicate: {}", info.transaction_id);
            }
        }
    }
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
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("future is pending"),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn kpi_transaction(
        tx_type: ExchangeTxType,
        amount_sats: i64,
        timestamp: i64,
        converted_fiat_amount: Option<f64>,
        fee: Option<f64>,
        fee_currency: Option<&str>,
        converted_fee_amount: Option<f64>,
        match_state: MatchState,
    ) -> ExchangeKpiTransaction {
        ExchangeKpiTransaction {
            timestamp,
            tx_type,
            amount_sats,
            fiat_amount: None,
            fiat_currency: None,
            fee,
            fee_currency: fee_currency.map(str::to_string),
            match_state,
            converted_fiat_amount,
            converted_fee_amount,
        }
    }
    #[test]
    fn deleting_an_empty_batch_is_rejected() {
        let service = ExchangeService::connect(Database::open_in_memory().unwrap());
        assert!(service.delete_accounts(&[]).is_err());
        assert!(service.delete_transactions(1, &[]).is_err());
    }
    #[test]
    fn kpis_split_btc_movements_and_value_all_fees_in_the_default_currency() {
        let timestamp = 1_700_000_000;
        let transactions = vec![
            kpi_transaction(
                ExchangeTxType::Buy,
                100_000_000,
                timestamp,
                Some(50_000.0),
                Some(2.5),
                Some("EUR"),
                Some(2.75),
                MatchState::HashMatch,
            ),
            kpi_transaction(
                ExchangeTxType::Sell,
                50_000_000,
                timestamp + 1,
                Some(30_000.0),
                None,
                None,
                None,
                MatchState::HashMatch,
            ),
            kpi_transaction(
                ExchangeTxType::Reward,
                1_000_000,
                timestamp,
                None,
                None,
                None,
                None,
                MatchState::Unmatched,
            ),
            kpi_transaction(
                ExchangeTxType::Withdrawal,
                10_000_000,
                timestamp,
                None,
                Some(10_000.0),
                Some("BTC"),
                None,
                MatchState::Unmatched,
            ),
            kpi_transaction(
                ExchangeTxType::Deposit,
                20_000_000,
                timestamp,
                None,
                None,
                None,
                None,
                MatchState::Unmatched,
            ),
            kpi_transaction(
                ExchangeTxType::Deposit,
                5_000_000,
                timestamp,
                None,
                None,
                None,
                None,
                MatchState::HashMatch,
            ),
        ];
        let prices = BTreeMap::from([(timestamp, 60_000.0)]);
        let kpis = kpis(&transactions, "USD", &prices);
        assert_eq!(kpis.bought.sats, 100_000_000);
        assert_eq!(kpis.bought.fiat, 50_000.0);
        assert_eq!(kpis.bought.count, 1);
        assert_eq!(kpis.sold.sats, 50_000_000);
        assert_eq!(kpis.sold.fiat, 30_000.0);
        assert_eq!(kpis.sold.count, 1);
        assert_eq!(kpis.rewarded.sats, 1_000_000);
        assert_eq!(kpis.rewarded.fiat, 600.0);
        assert_eq!(kpis.rewarded.count, 1);
        assert_eq!(kpis.withdrawn.sats, 10_000_000);
        assert_eq!(kpis.withdrawn.fiat, 6_000.0);
        assert_eq!(kpis.withdrawn.count, 1);
        assert_eq!(kpis.deposited.sats, 25_000_000);
        assert_eq!(kpis.deposited.fiat, 15_000.0);
        assert_eq!(kpis.deposited.count, 2);
        assert!((kpis.fees.fiat - 8.75).abs() < 1e-9);
        assert_eq!(kpis.fees.sats, 14_583);
        assert_eq!(kpis.fees.count, 2);
        assert_eq!(kpis.onchain_matched.sats, 5_000_000);
        assert_eq!(kpis.onchain_matched.fiat, 3_000.0);
        assert_eq!(kpis.onchain_matched.count, 1);
        assert_eq!(kpis.onchain_total_count, 3);
        assert_eq!(kpis.fiat_currency, "USD");
    }
    #[test]
    fn parsed_btc_fees_are_normalized_to_satoshis_at_the_import_boundary() {
        let parsed = ParsedExchangeTx {
            fingerprint: "fingerprint".into(),
            timestamp: 1_700_000_000,
            tx_type: ExchangeTxType::Buy,
            amount: 100_000_000.0,
            currency: "BTC".into(),
            fiat_amount: None,
            fiat_currency: None,
            fee: Some(0.0001),
            fee_currency: Some("BTC".into()),
            tx_hash: None,
            description: None,
            raw_data: BTreeMap::new(),
        };
        let write = write_from_parsed(&parsed);
        assert_eq!(write.fee, Some(10_000.0));
    }
    #[test]
    fn manual_btc_fees_are_normalized_to_satoshis_on_save() {
        let database = Database::open_in_memory().unwrap();
        let service = ExchangeService::connect(database);
        let account = service.create_account("Test", None).unwrap();
        let request = ExchangeTransactionSaveRequest {
            exchange_id: account.id,
            transaction_id: None,
            draft: ExchangeTransactionDraft {
                timestamp: 1_700_000_000,
                tx_type: ExchangeTxType::Buy,
                amount_sats: 100_000_000,
                currency: "BTC".into(),
                fiat_amount: None,
                fiat_currency: None,
                fee: Some(0.0001),
                fee_currency: Some("BTC".into()),
                tx_hash: None,
                description: None,
            },
            duplicate_choice: None,
        };
        let outcome = block_on(service.save_transaction(&request)).unwrap();
        match outcome {
            ExchangeTransactionSaveOutcome::Saved(row) => {
                assert_eq!(row.fee, Some(10_000.0));
            }
            ExchangeTransactionSaveOutcome::Duplicate(info) => {
                panic!("unexpected duplicate: {}", info.transaction_id);
            }
        }
    }
    #[test]
    fn import_auto_links_a_unique_amount_date_transaction() {
        let database = Database::open_in_memory().unwrap();
        seed_on_chain_transaction(
            &database,
            on_chain_tx("import-tx", 100_250, TEST_BLOCK_TIME),
        );
        let service = ExchangeService::connect(database);
        let mapping = import_mapping();
        let csv = "Date,Amount,Currency,Type\n2023-11-14 22:13:20,0.001,BTC,Withdrawal\n";
        let summary =
            block_on(service.import_data(None, Some("Test"), None, csv, Some(&mapping))).unwrap();
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.auto_linked, 1);
        let rows =
            exchange_repository::list_account_transactions(&service.database, summary.exchange_id)
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].match_state, MatchState::AmountDateMatch);
        assert_eq!(rows[0].matched_tx_id.as_deref(), Some("import-tx"));
    }
    #[test]
    fn reconcile_links_at_most_one_row_to_a_transaction() {
        let database = Database::open_in_memory().unwrap();
        seed_on_chain_transaction(
            &database,
            on_chain_tx("shared-tx", 100_250, TEST_BLOCK_TIME),
        );
        let service = ExchangeService::connect(database);
        let account = service.create_account("Test", None).unwrap();
        save_withdrawal(&service, account.id, TEST_BLOCK_TIME);
        save_withdrawal(&service, account.id, TEST_BLOCK_TIME + 1);
        block_on(service.reconcile(account.id)).unwrap();
        let rows =
            exchange_repository::list_account_transactions(&service.database, account.id).unwrap();
        let matched = rows
            .iter()
            .filter(|row| row.match_state != MatchState::Unmatched)
            .count();
        assert_eq!(matched, 1);
        assert!(rows
            .iter()
            .any(|row| row.match_state == MatchState::AmountDateMatch));
    }
    #[test]
    fn reconcile_many_reconciles_each_selected_account_and_reports_missing_ones() {
        let database = Database::open_in_memory().unwrap();
        seed_on_chain_transaction(&database, on_chain_tx("batch-tx", 100_250, TEST_BLOCK_TIME));
        let service = ExchangeService::connect(database);
        let first = service.create_account("First", None).unwrap();
        let second = service.create_account("Second", None).unwrap();
        save_withdrawal(&service, first.id, TEST_BLOCK_TIME);
        save_withdrawal(&service, second.id, TEST_BLOCK_TIME);
        let summary = service
            .reconcile_many(&[first.id, second.id, first.id, 999_999])
            .unwrap();
        assert_eq!(summary.reconciled, 2);
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].exchange_id, 999_999);
        assert_eq!(summary.failed[0].name, None);
        for account in [&first, &second] {
            let rows =
                exchange_repository::list_account_transactions(&service.database, account.id)
                    .unwrap();
            assert!(rows
                .iter()
                .all(|row| row.match_state == MatchState::AmountDateMatch),);
        }
    }
    #[test]
    fn reconcile_many_rejects_an_empty_selection() {
        let service = ExchangeService::connect(Database::open_in_memory().unwrap());
        assert!(service.reconcile_many(&[]).is_err());
    }
    #[test]
    fn reconcile_preserves_an_unchanged_link_timestamp() {
        let database = Database::open_in_memory().unwrap();
        seed_on_chain_transaction(
            &database,
            on_chain_tx("stable-tx", 100_250, TEST_BLOCK_TIME),
        );
        let service = ExchangeService::connect(database);
        let account = service.create_account("Test", None).unwrap();
        let saved = save_withdrawal(&service, account.id, TEST_BLOCK_TIME);
        block_on(service.reconcile(account.id)).unwrap();
        let row = exchange_repository::get_transaction(&service.database, saved.id)
            .unwrap()
            .unwrap();
        let link = row.on_chain_link.clone().unwrap();
        let old_link = OnChainLink {
            linked_at: 1,
            ..link
        };
        exchange_repository::set_link(
            &service.database,
            account.id,
            row.id,
            row.match_state,
            row.matched_tx_id.as_deref(),
            Some(&old_link),
        )
        .unwrap();
        block_on(service.reconcile(account.id)).unwrap();
        let row = exchange_repository::get_transaction(&service.database, saved.id)
            .unwrap()
            .unwrap();
        assert_eq!(row.on_chain_link.unwrap().linked_at, 1);
    }
}
