use dioxus::prelude::*;

use super::date_picker::current_utc_date;
use super::{use_fiat_currency, DatePicker};
use crate::api::prices::{conversion_currencies_api, sync_prices_api};
use crate::domain::civil_time::format_date;
use crate::domain::date_selection::{DateBounds, DateSelection, DateSelectionMode, IsoDate};
use crate::domain::price::{
    FiatCurrency, PriceSyncSummary, SyncGranularity, FIAT_CURRENCIES, PRICE_HISTORY_START,
    USD_SYMBOL,
};

/// Granularity choices of the dialog, finest last.
const GRANULARITIES: [SyncGranularity; 4] = [
    SyncGranularity::Auto,
    SyncGranularity::Daily,
    SyncGranularity::Hourly,
    SyncGranularity::FifteenMinutes,
];

/// Modal dialog of the "Sync Prices" action.
///
/// One form drives one unified workflow:
///
/// - the currency defaults to the app-wide default currency and always offers
///   USD first;
/// - the granularity picks the resolution of the recent window (`Auto` asks
///   Coin Metrics for the finest frequency the free tier still serves, which
///   is a 15-minute sample of the 1-minute reference rate, or hourly when that
///   window is gone);
/// - the range defaults to the full source span, 2010-07-18 through today.
///
/// Every run fills the BTC/USD daily closes of the range; USD additionally
/// stores the finer recent observations, while another currency refreshes the
/// USD series first and then converts every stored USD close time of the range
/// with Frankfurter's daily rates. Existing rows are never overwritten, so the
/// sync can be re-run to fill what is still missing.
#[component]
pub fn SyncPricesDialog(
    on_close: EventHandler<()>,
    on_synced: EventHandler<PriceSyncSummary>,
) -> Element {
    let fiat = use_fiat_currency();
    let first_day = IsoDate::from_epoch(PRICE_HISTORY_START);
    let today = current_utc_date();
    let bounds = DateBounds::new(Some(first_day), Some(today));

    let mut selected = use_signal(|| fiat.code());
    let mut granularity = use_signal(SyncGranularity::default);
    let mut selection =
        use_signal(|| DateSelection::default_in_bounds(DateSelectionMode::Range, &bounds));
    let mut loading = use_signal(|| false);
    let mut status = use_signal(|| Option::<String>::None);

    let currencies = use_resource(move || async move {
        conversion_currencies_api()
            .await
            .map_err(|error| error.to_string())
    });

    let is_usd = selected() == USD_SYMBOL;
    let effective_granularity = if is_usd {
        granularity()
    } else {
        SyncGranularity::Auto
    };
    let description = sync_description(selected(), effective_granularity);

    let close = move |_| on_close.call(());
    let sync = move |_| {
        if *loading.read() {
            return;
        }
        let code = selected();
        let current = selection();
        let (Some(from_day), Some(to_day)) = (current.from, current.to) else {
            status.set(Some(
                "Choose both the From and To dates to sync.".to_string(),
            ));
            return;
        };
        let from_time = from_day.epoch_start();
        // The To date includes its whole day.
        let to_time = to_day.epoch_end();
        if from_time > to_time {
            status.set(Some(
                "The From date must not be after the To date.".to_string(),
            ));
            return;
        }
        let requested = if code == USD_SYMBOL {
            granularity()
        } else {
            SyncGranularity::Auto
        };

        loading.set(true);
        status.set(None);
        spawn(async move {
            match sync_prices_api(code, from_time, to_time, requested).await {
                Ok(summary) => {
                    loading.set(false);
                    status.set(Some(sync_status(&summary)));
                    on_synced.call(summary);
                }
                Err(error) => {
                    loading.set(false);
                    status.set(Some(format!("Could not sync the prices: {error}")));
                }
            }
        });
    };

    let displayed_selection = selection_or_default(selection(), bounds);

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: close,
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "sync-prices-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "sync-prices-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Sync Prices"
                }

                p { class: "text-body-medium text-on-surface-variant", "{description}" }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "sync-prices-currency",
                    "Currency"
                }
                div { class: "md3-text-field",
                    select {
                        id: "sync-prices-currency",
                        class: "md3-input",
                        disabled: *loading.read(),
                        onchange: move |event| {
                            let code = event.value();
                            if code != USD_SYMBOL {
                                granularity.set(SyncGranularity::Auto);
                            }
                            selected.set(code);
                            status.set(None);
                        },
                        match currencies() {
                            Some(Ok(list)) => rsx! {
                                for currency in currency_choices(&list, &selected()) {
                                    option {
                                    value: "{currency.code}",
                                    selected: selected() == currency.code,
                                        "{currency.code} — {currency.name}"
                                    }
                                }
                            },
                            Some(Err(_)) => rsx! {
                                // The live catalogue is unreachable (e.g. a
                                // transient Frankfurter outage): fall back to
                                // the built-in list so syncing stays available.
                                for code in FIAT_CURRENCIES.iter().copied() {
                                    option {
                                        value: "{code}",
                                        selected: selected() == code,
                                        "{code}"
                                    }
                                }
                            },
                            None => rsx! {
                                option {
                                    value: "{selected()}",
                                    selected: true,
                                    "{selected()}"
                                }
                                option { value: "", disabled: true, "Loading currencies..." }
                            },
                        }
                    }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "sync-prices-granularity",
                    "Granularity"
                }
                div { class: "md3-text-field",
                    select {
                        id: "sync-prices-granularity",
                        class: "md3-input",
                        disabled: *loading.read() || !is_usd,
                        onchange: move |event| {
                            if let Some(value) = SyncGranularity::from_key(&event.value()) {
                                granularity.set(value);
                                status.set(None);
                            }
                        },
                        for option in GRANULARITIES {
                            option {
                                value: "{option.key()}",
                                selected: effective_granularity == option,
                                "{option.label()}"
                            }
                        }
                    }
                }
                if !is_usd {
                    p { class: "text-body-small text-on-surface-variant",
                        "Auto follows the timestamps of the stored USD prices: Frankfurter publishes daily rates, so every close of a day is converted with that day's rate."
                    }
                }

                DatePicker {
                    id: "sync-prices-range".to_string(),
                    label: "Date range".to_string(),
                    value: displayed_selection,
                    min_date: Some(first_day),
                    max_date: Some(today),
                    disabled: *loading.read(),
                    popup_above: true,
                    compact: true,
                    on_change: move |value: DateSelection| {
                        selection.set(value);
                        status.set(None);
                    },
                }

                if let Some(message) = status() {
                    p {
                        class: "text-body-small text-on-surface-variant",
                        role: "status",
                        aria_live: "polite",
                        "{message}"
                    }
                }

                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        onclick: close,
                        disabled: *loading.read(),
                        "Close"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        onclick: sync,
                        disabled: *loading.read(),
                        if *loading.read() {
                            "Syncing..."
                        } else {
                            "Sync prices"
                        }
                    }
                }
            }
        }
    }
}

/// Use the configured selectable span when the dialog has no explicit range.
fn selection_or_default(selection: DateSelection, bounds: DateBounds) -> DateSelection {
    if selection.is_empty() {
        DateSelection::default_in_bounds(DateSelectionMode::Range, &bounds)
    } else {
        selection
    }
}

/// USD first, then every other catalogue currency sorted by code.
fn currency_choices(list: &[FiatCurrency], selected: &str) -> Vec<FiatCurrency> {
    let mut choices = Vec::with_capacity(list.len() + 1);
    match list.iter().find(|currency| currency.code == USD_SYMBOL) {
        Some(usd) => choices.push(usd.clone()),
        None => choices.push(FiatCurrency {
            code: USD_SYMBOL.to_string(),
            name: "United States Dollar".to_string(),
        }),
    }
    choices.extend(
        list.iter()
            .filter(|currency| currency.code != USD_SYMBOL)
            .cloned(),
    );
    // A stored preference outside the catalogue must still be selectable.
    if !choices.iter().any(|currency| currency.code == selected) {
        choices.push(FiatCurrency {
            code: selected.to_string(),
            name: selected.to_string(),
        });
    }
    choices
}

/// Text under the title: what the current selection will do.
fn sync_description(symbol: String, granularity: SyncGranularity) -> String {
    let resolution = match granularity {
        SyncGranularity::Auto => "the finest recent closes the free tier still serves",
        SyncGranularity::Daily => "daily closes",
        SyncGranularity::Hourly => "hourly closes",
        SyncGranularity::FifteenMinutes => "15-minute closes",
    };
    if symbol == USD_SYMBOL {
        format!(
            "Fetch the BTC/USD history of the selected range with {resolution}. Daily closes come from Coin Metrics and start in {}; finer closes exist only for the most recent days of the free tier.",
            format_date(PRICE_HISTORY_START)
        )
    } else {
        format!(
            "Refresh the BTC/USD history of the selected range, then convert every stored USD close time into {symbol}. Frankfurter publishes daily rates, so every close of a day uses that day's rate."
        )
    }
}

/// Status line of a successful run.
fn sync_status(summary: &PriceSyncSummary) -> String {
    let converted = summary
        .conversion
        .as_ref()
        .map_or(0, |conversion| conversion.stored);
    let mut message = if summary.daily_stored == 0 && summary.granular_stored == 0 && converted == 0
    {
        "Sync complete: the stored history is already up to date for the selected range."
            .to_string()
    } else {
        format!(
            "Sync complete: {} BTC/USD daily closes fetched from {} through {}, {} stored.",
            summary.daily_fetched,
            format_date(summary.from_time),
            format_date(summary.to_time),
            summary.daily_stored
        )
    };
    if let (Some(from_time), Some(to_time)) = (summary.granular_from_time, summary.granular_to_time)
    {
        message.push_str(&format!(
            " {} {} closes fetched for {} through {}, {} stored.",
            summary.granular_fetched,
            summary.granularity.resolution_label(),
            format_date(from_time),
            format_date(to_time),
            summary.granular_stored
        ));
    }
    if summary.unavailable_days > 0 {
        message.push_str(&format!(
            " {} day(s) have no available daily price.",
            summary.unavailable_days
        ));
    }
    if let Some(conversion) = &summary.conversion {
        message.push_str(&format!(
            " {}: {} rows converted, {} skipped (no FX rate for their day).",
            conversion.symbol, conversion.stored, conversion.skipped
        ));
    }
    for warning in &summary.warnings {
        message.push(' ');
        message.push_str(warning);
    }
    message
}
