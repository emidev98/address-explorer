//! Exchange ledger service: CSV import, account/transaction workflows,
//! reconciliation, and manual on-chain linking.

use std::collections::{BTreeMap, HashSet};

use crate::application::exchange_fiat::{ExchangeFiatService, FiatValue};
use crate::data::repositories::{
    addresses as address_repository, catalogs as catalog_repository,
    exchanges as exchange_repository, settings as settings_repository,
};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::exchange::{
    csv::{self, ParsedExchangeTx},
    matching::{self, OnChainLinkCandidate},
    ExchangeAccount, ExchangeAccountList, ExchangeAccountTableQuery, ExchangeDetail,
    ExchangeDuplicateChoice, ExchangeDuplicateInfo, ExchangeKpis, ExchangeLinkOutcome,
    ExchangeTransaction, ExchangeTransactionDraft, ExchangeTransactionSaveOutcome,
    ExchangeTransactionSaveRequest, ExchangeTransactionTableQuery, ExchangeTxType,
    ImportDataMapping, ImportDataPreview, ImportDataPreviewRow, ImportDataSummary, MatchState,
    OnChainLink, OnChainLinkCandidateView, OnChainLinkSource,
};
use crate::domain::table::{SortDirection, TablePage};

const MAX_CSV_BYTES: usize = 25 * 1024 * 1024;
const MAX_CSV_ROWS: usize = 200_000;
const MAX_PAGE_SIZE: u32 = 100;

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

    pub fn list_accounts(
        &self,
        query: &ExchangeAccountTableQuery,
    ) -> Result<ExchangeAccountList, ExchangeServiceError> {
        let sort = parse_account_sort(query.table.sort_by.as_deref(), query.table.sort_dir)?;
        let page_size = query.table.page_size.clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from(query.table.page) * u64::from(page_size);
        let (page, totals) =
            exchange_repository::list_accounts(&self.database, page_size, offset, sort)?;
        Ok(ExchangeAccountList { page, totals })
    }

    pub fn get_account(&self, id: i64) -> Result<ExchangeAccount, ExchangeServiceError> {
        exchange_repository::get_account(&self.database, id)?
            .ok_or(ExchangeServiceError::AccountNotFound)
    }

    pub async fn detail(&self, id: i64) -> Result<ExchangeDetail, ExchangeServiceError> {
        let account = self.get_account(id)?;
        let target = self.fiat.target_currency()?;
        let mut transactions = exchange_repository::list_account_transactions(&self.database, id)?;
        let warning = self.fiat.convert_transactions(&mut transactions).await;
        let kpis = kpis(&transactions, &target);
        Ok(ExchangeDetail {
            account,
            kpis,
            conversion_warning: warning,
        })
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

    pub fn delete_account(&self, id: i64) -> Result<bool, ExchangeServiceError> {
        Ok(exchange_repository::delete_account(&self.database, id)?)
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

        let (mut existing_fingerprints, mut linked_txids) =
            exchange_repository::existing_identities(&self.database, account.id)?;
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring =
            settings_repository::load_exchange_settings(&self.database)?.on_chain_link_scoring;
        let mut writes = Vec::new();
        let mut duplicates = 0;
        let mut auto_linked = 0;
        let now = now();
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
            if let Some(top) =
                matching::candidates(&temporary, &on_chain, "", &scoring, &linked_txids)
                    .into_iter()
                    .next()
            {
                if top.likeliness == 1.0 && !top.is_already_linked {
                    let source = if top.source == OnChainLinkSource::Hash {
                        OnChainLinkSource::Hash
                    } else {
                        OnChainLinkSource::AmountDate
                    };
                    let state = if source == OnChainLinkSource::Hash {
                        MatchState::HashMatch
                    } else {
                        MatchState::AmountDateMatch
                    };
                    let link = link_from_candidate(&top, source, now);
                    write.match_state = state;
                    write.matched_tx_id = Some(top.txid.clone());
                    write.on_chain_link = Some(link);
                    linked_txids.insert(top.txid.clone());
                    auto_linked += 1;
                }
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
            draft.fee,
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

    pub fn delete_transaction(
        &self,
        exchange_id: i64,
        transaction_id: i64,
    ) -> Result<bool, ExchangeServiceError> {
        Ok(exchange_repository::delete_transaction(
            &self.database,
            exchange_id,
            transaction_id,
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
        let transactions =
            exchange_repository::list_account_transactions(&self.database, exchange_id)?;
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring =
            settings_repository::load_exchange_settings(&self.database)?.on_chain_link_scoring;
        let linked = HashSet::new();
        let now = now();
        let mut updates = Vec::new();
        for transaction in transactions {
            if transaction.match_state == MatchState::ManualMatch {
                continue;
            }
            let result =
                matching::match_transaction(&transaction, &on_chain, &scoring, &linked, now);
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
        let on_chain_ids =
            exchange_repository::existing_identities(&self.database, transaction.exchange_id)?;
        let mut linked = on_chain_ids.1;
        if let Some(txid) = transaction
            .on_chain_link
            .as_ref()
            .map(|link| link.txid.clone())
            .or_else(|| transaction.matched_tx_id.clone())
        {
            linked.remove(&txid);
        }
        let on_chain = exchange_repository::load_on_chain_transactions(&self.database)?;
        let scoring =
            settings_repository::load_exchange_settings(&self.database)?.on_chain_link_scoring;
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
            linked_at: now(),
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
        parsed.fee,
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
    raw.insert("createdAt".into(), civil_time::format_datetime(now()));
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
    let amount = if csv::is_bitcoin_currency(&transaction.currency) {
        crate::components::format::format_btc(transaction.amount_sats)
    } else {
        format!("{} {}", transaction.amount_sats, transaction.currency)
    };
    ExchangeDuplicateInfo {
        transaction_id: transaction.id,
        summary: format!(
            "{} {} · {} · {}",
            transaction.tx_type.as_str(),
            amount,
            civil_time::format_datetime(transaction.timestamp),
            transaction
                .tx_hash
                .as_deref()
                .map(|hash| format!("hash {hash}"))
                .unwrap_or_else(|| {
                    transaction
                        .description
                        .clone()
                        .unwrap_or_else(|| "no hash".to_string())
                })
        ),
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

fn link_from_candidate(
    candidate: &OnChainLinkCandidate,
    source: OnChainLinkSource,
    linked_at: i64,
) -> OnChainLink {
    OnChainLink {
        txid: candidate.txid.clone(),
        local_addresses: candidate.local_addresses.clone(),
        source,
        linked_at,
    }
}

fn kpis(transactions: &[ExchangeTransaction], target: &str) -> ExchangeKpis {
    let mut kpis = ExchangeKpis {
        fiat_currency: target.to_string(),
        ..ExchangeKpis::default()
    };
    for transaction in transactions {
        let amount = transaction.amount_sats.abs();
        let fiat = transaction.converted_fiat_amount.unwrap_or(0.0).abs();
        match transaction.tx_type {
            ExchangeTxType::Buy => {
                kpis.bought_sats += amount;
                kpis.fiat_spent += fiat;
            }
            ExchangeTxType::Sell => {
                kpis.sold_sats += amount;
                kpis.fiat_received += fiat;
            }
            ExchangeTxType::Withdrawal => kpis.withdrawn_sats += amount,
            ExchangeTxType::Deposit => kpis.deposited_sats += amount,
            _ => {}
        }
        if transaction.match_state == MatchState::Unmatched {
            kpis.unmatched_count += 1;
        }
        if transaction
            .fee_currency
            .as_deref()
            .is_some_and(csv::is_bitcoin_currency)
        {
            kpis.btc_fees_sats += transaction.fee.unwrap_or(0.0).abs() as i64;
        }
    }
    kpis
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
