//! Endpoint dispatcher executed inside the SharedWorker.
//!
//! The dispatcher mirrors the native `#[server]` controller bodies. It is a
//! transport adapter: argument decoding, one application-service call, and
//! JSON encoding. Business rules and SQL stay in the application/data layers.

use crate::application;
use crate::db::BrowserDatabase;
use crate::domain;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

fn arg<T: DeserializeOwned>(payload: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(payload.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|error| format!("invalid `{name}` argument: {error}"))
}

fn encode<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("could not encode the response: {error}"))
}

fn parse_kind(value: &str) -> Result<domain::catalog::CatalogKind, String> {
    domain::catalog::CatalogKind::parse(value)
        .ok_or_else(|| format!("unknown catalog kind: {value}"))
}

/// Whether a successful endpoint changes data and must wait for the browser
/// commit barrier before reporting success.
pub fn is_write(endpoint: &str) -> bool {
    matches!(
        endpoint,
        "addresses.save"
            | "addresses.update"
            | "addresses.delete"
            | "catalogs.create_type"
            | "catalogs.update_type"
            | "catalogs.delete_types"
            | "catalogs.create_category"
            | "catalogs.rename_category"
            | "catalogs.delete_category"
            | "catalogs.import"
            | "exchanges.create"
            | "exchanges.update"
            | "exchanges.delete"
            | "exchanges.csv_import"
            | "exchanges.transaction_save"
            | "exchanges.transactions_delete"
            | "exchanges.reconcile"
            | "exchanges.reconcile_many"
            | "exchanges.link"
            | "exchanges.clear_link"
            | "prices.sync"
            | "prices.ensure_currency_history"
            | "prices.update"
            | "prices.delete"
            | "settings.save_theme"
            | "settings.save_default_fiat_currency"
            | "settings.save_on_chain_link_scoring"
            | "settings.save_confirm_destructive_actions"
            | "settings.save_show_date_range_support"
            | "settings.save_navigation_position"
            | "settings.save_side_nav_collapsed"
            | "settings.save_security"
            | "transactions.sync_address"
            | "transactions.sync_addresses"
    )
}

/// Execute one endpoint against the browser database.
pub async fn dispatch(
    db: &mut BrowserDatabase,
    endpoint: &str,
    payload: Value,
) -> Result<Value, String> {
    let database = &db.database;
    match endpoint {
        // ---- addresses ----
        "addresses.overview" => {
            let query = arg(&payload, "query")?;
            encode(
                application::addresses::AddressService::connect(database.clone())
                    .addresses_overview(&query)
                    .map_err(|error| error.to_string())?,
            )
        }
        "addresses.validate" => {
            let address: String = arg(&payload, "address")?;
            encode(
                application::addresses::validate_address(&address)
                    .map_err(|error| error.to_string())?,
            )
        }
        "addresses.save" => {
            let address: String = arg(&payload, "address")?;
            let description: Option<String> = arg(&payload, "description")?;
            let exchange_id: Option<i64> = arg(&payload, "exchange_id")?;
            let wallet_id: Option<i64> = arg(&payload, "wallet_id")?;
            let result = application::addresses::AddressService::connect(database.clone())
                .import_address_and_sync(&address, description.as_deref(), exchange_id, wallet_id)
                .await;
            match result {
                Ok(imported) => encode(domain::address::AddressSaveOutcome::Imported(Box::new(
                    imported,
                ))),
                Err(application::addresses::AddressServiceError::AlreadyExists) => {
                    encode(domain::address::AddressSaveOutcome::Duplicate)
                }
                Err(error) => Err(error.to_string()),
            }
        }
        "addresses.update" => {
            let address: String = arg(&payload, "address")?;
            let description: Option<String> = arg(&payload, "description")?;
            let exchange_id: Option<i64> = arg(&payload, "exchange_id")?;
            let wallet_id: Option<i64> = arg(&payload, "wallet_id")?;
            encode(
                application::addresses::AddressService::connect(database.clone())
                    .update_address(&address, description.as_deref(), exchange_id, wallet_id)
                    .map_err(|error| error.to_string())?,
            )
        }
        "addresses.delete" => {
            let addresses: Vec<String> = arg(&payload, "addresses")?;
            encode(
                application::addresses::AddressService::connect(database.clone())
                    .delete_addresses(&addresses)
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- catalogs ----
        "catalogs.list_types" => {
            let kind: String = arg(&payload, "kind")?;
            let query: domain::table::TableQuery = arg(&payload, "query")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .list_types(parse_kind(&kind)?, &query)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.create_type" => {
            let kind: String = arg(&payload, "kind")?;
            let draft: domain::catalog::CatalogTypeDraft = arg(&payload, "draft")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .create_type(parse_kind(&kind)?, draft)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.update_type" => {
            let kind: String = arg(&payload, "kind")?;
            let id: i64 = arg(&payload, "id")?;
            let draft: domain::catalog::CatalogTypeDraft = arg(&payload, "draft")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .update_type(parse_kind(&kind)?, id, draft)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.types_delete_preview" => {
            let kind: String = arg(&payload, "kind")?;
            let ids: Vec<i64> = arg(&payload, "ids")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .delete_preview(parse_kind(&kind)?, &ids)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.delete_types" => {
            let kind: String = arg(&payload, "kind")?;
            let ids: Vec<i64> = arg(&payload, "ids")?;
            let cascade: bool = arg(&payload, "cascade")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .delete_types(parse_kind(&kind)?, &ids, cascade)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.list_categories" => {
            let kind: String = arg(&payload, "kind")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .list_categories(parse_kind(&kind)?)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.create_category" => {
            let kind: String = arg(&payload, "kind")?;
            let name: String = arg(&payload, "name")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .create_category(parse_kind(&kind)?, &name)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.rename_category" => {
            let kind: String = arg(&payload, "kind")?;
            let id: i64 = arg(&payload, "id")?;
            let name: String = arg(&payload, "name")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .rename_category(parse_kind(&kind)?, id, &name)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.category_delete_preview" => {
            let kind: String = arg(&payload, "kind")?;
            let id: i64 = arg(&payload, "id")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .delete_category_preview(parse_kind(&kind)?, id)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.delete_category" => {
            let kind: String = arg(&payload, "kind")?;
            let id: i64 = arg(&payload, "id")?;
            let cascade: bool = arg(&payload, "cascade")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .delete_category(parse_kind(&kind)?, id, cascade)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.import_preview" => {
            let kind: String = arg(&payload, "kind")?;
            let csv_text: String = arg(&payload, "csv_text")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .preview_import(parse_kind(&kind)?, &csv_text)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.import" => {
            let kind: String = arg(&payload, "kind")?;
            let csv_text: String = arg(&payload, "csv_text")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .import_csv(parse_kind(&kind)?, &csv_text)
                    .map_err(|error| error.to_string())?,
            )
        }
        "catalogs.options" => {
            let kind: String = arg(&payload, "kind")?;
            encode(
                application::catalogs::CatalogService::connect(database.clone())
                    .options(parse_kind(&kind)?)
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- exchanges ----
        "exchanges.list" => {
            let query = arg(&payload, "query")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .list_accounts(&query)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.get" => {
            let id: i64 = arg(&payload, "id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .detail(id)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.create" => {
            let name: String = arg(&payload, "name")?;
            let provider_id: Option<i64> = arg(&payload, "provider_id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .create_account(&name, provider_id)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.update" => {
            let id: i64 = arg(&payload, "id")?;
            let name: String = arg(&payload, "name")?;
            let provider_id: Option<i64> = arg(&payload, "provider_id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .update_account(id, &name, provider_id)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.delete" => {
            let ids: Vec<i64> = arg(&payload, "ids")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .delete_accounts(&ids)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.csv_preview" => {
            let csv_text: String = arg(&payload, "csv_text")?;
            let mapping: Option<domain::exchange::ImportDataMapping> = arg(&payload, "mapping")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .preview_import_data(&csv_text, mapping.as_ref())
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.csv_import" => {
            let exchange_id: Option<i64> = arg(&payload, "exchange_id")?;
            let name: Option<String> = arg(&payload, "name")?;
            let provider_id: Option<i64> = arg(&payload, "provider_id")?;
            let csv_text: String = arg(&payload, "csv_text")?;
            let mapping: Option<domain::exchange::ImportDataMapping> = arg(&payload, "mapping")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .import_data(
                        exchange_id,
                        name.as_deref(),
                        provider_id,
                        &csv_text,
                        mapping.as_ref(),
                    )
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.transactions_list" => {
            let query = arg(&payload, "query")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .list_transactions(&query)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.transaction_get" => {
            let account_id: i64 = arg(&payload, "account_id")?;
            let transaction_id: i64 = arg(&payload, "transaction_id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .get_transaction(account_id, transaction_id)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.transaction_save" => {
            let request: domain::exchange::ExchangeTransactionSaveRequest =
                arg(&payload, "request")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .save_transaction(&request)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.transactions_delete" => {
            let account_id: i64 = arg(&payload, "account_id")?;
            let ids: Vec<i64> = arg(&payload, "ids")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .delete_transactions(account_id, &ids)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.reconcile" => {
            let account_id: i64 = arg(&payload, "account_id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .reconcile(account_id)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.reconcile_many" => {
            let ids: Vec<i64> = arg(&payload, "ids")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .reconcile_many(&ids)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.link_candidates" => {
            let transaction_id: i64 = arg(&payload, "transaction_id")?;
            let search: String = arg(&payload, "search")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .candidates(transaction_id, &search)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.link" => {
            let transaction_id: i64 = arg(&payload, "transaction_id")?;
            let txid: String = arg(&payload, "txid")?;
            let allow_duplicate: bool = arg(&payload, "allow_duplicate")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .link(transaction_id, &txid, allow_duplicate)
                    .map_err(|error| error.to_string())?,
            )
        }
        "exchanges.clear_link" => {
            let transaction_id: i64 = arg(&payload, "transaction_id")?;
            encode(
                application::exchanges::ExchangeService::connect(database.clone())
                    .clear_link(transaction_id)
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- flow ----
        "flow.filter_options" => encode(
            application::flow::FlowService::connect(database.clone())
                .filter_options()
                .map_err(|error| error.to_string())?,
        ),
        "flow.overview" => {
            let query = arg(&payload, "query")?;
            encode(
                application::flow::FlowService::connect(database.clone())
                    .overview(&query)
                    .map_err(|error| error.to_string())?,
            )
        }
        "flow.links" => {
            let query = arg(&payload, "query")?;
            encode(
                application::flow::FlowService::connect(database.clone())
                    .link_table(&query)
                    .map_err(|error| error.to_string())?,
            )
        }
        "flow.links_evidence" => {
            let query = arg(&payload, "query")?;
            encode(
                application::flow::FlowService::connect(database.clone())
                    .evidence(&query)
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- session ----
        "session.status" => Ok(serde_json::Value::Null),

        // ---- health ----
        "health.get" => encode(domain::health::Health {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            api_version: domain::health::API_VERSION,
            tls_fingerprint: None,
        }),

        // ---- prices ----
        "prices.latest" => {
            let symbol: String = arg(&payload, "symbol")?;
            encode(
                application::prices::PriceService::connect(database.clone())
                    .latest_price(&symbol)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.list" => {
            let query: domain::price::PriceTableQuery = arg(&payload, "query")?;
            encode(
                application::prices::list_price_page(
                    database,
                    &query.symbol,
                    query.from_time,
                    query.to_time,
                    &query.table,
                )
                .map_err(|error| error.to_string())?,
            )
        }
        "prices.sync" => {
            let symbol: String = arg(&payload, "symbol")?;
            let from_time: i64 = arg(&payload, "from_time")?;
            let to_time: i64 = arg(&payload, "to_time")?;
            let granularity: domain::price::SyncGranularity = arg(&payload, "granularity")?;
            encode(
                application::prices::PriceService::connect(database.clone())
                    .sync_prices(&symbol, from_time, to_time, granularity)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.ensure_currency_history" => {
            let symbol: String = arg(&payload, "symbol")?;
            encode(
                application::prices::PriceService::connect(database.clone())
                    .ensure_currency_history(&symbol)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.conversions" => {
            let symbol: String = arg(&payload, "symbol")?;
            let close_time: i64 = arg(&payload, "close_time")?;
            encode(
                application::prices::list_price_conversions(database, &symbol, close_time)
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.update" => {
            let symbol: String = arg(&payload, "symbol")?;
            let close_time: i64 = arg(&payload, "close_time")?;
            let close: f64 = arg(&payload, "close")?;
            encode(
                application::prices::update_stored_price(database, &symbol, close_time, close)
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.delete" => {
            let close_times: Vec<i64> = arg(&payload, "close_times")?;
            encode(
                application::prices::delete_stored_prices(database, &close_times)
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.series" => {
            let symbol: String = arg(&payload, "symbol")?;
            let from_time: i64 = arg(&payload, "from_time")?;
            let to_time: i64 = arg(&payload, "to_time")?;
            encode(
                application::prices::price_series(database, &symbol, from_time, to_time)
                    .map_err(|error| error.to_string())?,
            )
        }
        "prices.currencies" => encode(
            application::prices::PriceService::connect(database.clone())
                .conversion_currencies()
                .await
                .map_err(|error| error.to_string())?,
        ),

        // ---- settings ----
        "settings.get_theme" => {
            encode(application::settings::load_theme(database).map_err(|error| error.to_string())?)
        }
        "settings.save_theme" => {
            let preference: domain::theme::ThemePreference = arg(&payload, "preference")?;
            encode(
                application::settings::save_theme(database, &preference)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.get_default_fiat_currency" => encode(
            application::settings::load_default_fiat_currency(database)
                .map_err(|error| error.to_string())?,
        ),
        "settings.save_default_fiat_currency" => {
            let symbol: String = arg(&payload, "symbol")?;
            encode(
                application::settings::save_default_fiat_currency(database, &symbol)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.get_on_chain_link_scoring" => encode(
            application::settings::load_on_chain_link_scoring(database)
                .map_err(|error| error.to_string())?,
        ),
        "settings.save_confirm_destructive_actions" => {
            let enabled: bool = arg(&payload, "enabled")?;
            encode(
                application::settings::save_confirm_destructive_actions(database, enabled)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.save_on_chain_link_scoring" => {
            let settings: domain::exchange::OnChainLinkScoringSettings = arg(&payload, "settings")?;
            encode(
                application::settings::save_on_chain_link_scoring(database, &settings)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.get_interface" => encode(
            application::settings::load_interface_settings(database)
                .map_err(|error| error.to_string())?,
        ),
        "settings.save_show_date_range_support" => {
            let enabled: bool = arg(&payload, "enabled")?;
            encode(
                application::settings::save_show_date_range_support(database, enabled)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.save_navigation_position" => {
            let position: domain::interface_settings::NavigationPosition =
                arg(&payload, "position")?;
            encode(
                application::settings::save_navigation_position(database, position)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.save_side_nav_collapsed" => {
            let collapsed: bool = arg(&payload, "collapsed")?;
            encode(
                application::settings::save_side_nav_collapsed(database, collapsed)
                    .map_err(|error| error.to_string())?,
            )
        }
        "settings.get_security" => encode(
            application::settings::load_security_settings(database)
                .map_err(|error| error.to_string())?,
        ),
        "settings.save_security" => {
            let settings: domain::security_settings::SecuritySettings =
                arg(&payload, "settings_value")?;
            encode(
                application::settings::save_security_settings(database, &settings)
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- transactions ----
        "transactions.sync_address" => {
            let address: String = arg(&payload, "address")?;
            encode(
                application::transactions::TransactionService::connect(database.clone())
                    .sync_address_transactions(&address)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }
        "transactions.sync_addresses" => {
            let addresses: Vec<String> = arg(&payload, "addresses")?;
            encode(
                application::transactions::TransactionService::connect(database.clone())
                    .sync_addresses_transactions(&addresses)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        }

        // ---- data management ----
        other => Err(format!("unknown worker endpoint: {other}")),
    }
}
