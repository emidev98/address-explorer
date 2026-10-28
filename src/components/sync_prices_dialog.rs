use dioxus::prelude::*;

use super::DatePicker;
use crate::api::prices::{conversion_currencies_api, sync_prices_api, sync_prices_to_today_api};
use crate::domain::civil_time::{format_date, parse_date};
use crate::domain::price::{PriceSyncOutcome, SyncToTodaySummary, FIAT_CURRENCIES};

/// Seconds in a day; the inclusive end of the sync range's last day.
const DAY_SECONDS: i64 = 86_400;

/// The two operations offered by the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncMode {
    /// Convert the stored USD history of a range into another currency.
    Historical,
    /// Fetch BTC/USD daily closes from the latest stored point to today.
    ToToday,
}

impl SyncMode {
    fn label(self) -> &'static str {
        match self {
            Self::Historical => "Sync historical prices",
            Self::ToToday => "Sync to today",
        }
    }
}

/// Modal dialog of the "Sync Prices" action.
///
/// "Sync historical prices" is the existing conversion workflow: convert the
/// stored USD history of a date range into a chosen currency and persist it.
/// The range is prefilled with the date filter of the Historical Prices page;
/// the Frankfurter series is fetched once for the whole range and applied only
/// to close times that already exist in USD, so the result matches the
/// granularity of the table.
///
/// "Sync to today" fetches BTC/USD daily closes from Coin Metrics from the
/// latest stored close through the current day. Coin Metrics timestamps each
/// daily row at the start of its interval, so the rows are stored at the
/// matching daily close time.
///
/// Both modes leave existing rows untouched, so the sync can be re-run to
/// fill gaps. The dialog stays open after a successful sync so another mode,
/// currency, or range can follow.
#[component]
pub fn SyncPricesDialog(
    /// Prefill of the From date (`YYYY-MM-DD`, the page's current filter).
    initial_from: String,
    /// Prefill of the To date.
    initial_to: String,
    on_close: EventHandler<()>,
    on_synced: EventHandler<PriceSyncOutcome>,
) -> Element {
    let mut mode = use_signal(|| SyncMode::Historical);
    let mut selected = use_signal(String::new);
    let mut from_input = use_signal(|| initial_from.clone());
    let mut to_input = use_signal(|| initial_to.clone());
    let mut loading = use_signal(|| false);
    let mut status = use_signal(|| Option::<String>::None);

    let currencies = use_resource(move || async move {
        if mode() != SyncMode::Historical {
            return Ok(Vec::new());
        }
        conversion_currencies_api()
            .await
            .map_err(|error| error.to_string())
    });

    let close = move |_| on_close.call(());
    let sync = move |_| {
        if *loading.read() {
            return;
        }
        match mode() {
            SyncMode::Historical => {
                let code = selected();
                if code.is_empty() {
                    status.set(Some("Choose a currency first.".to_string()));
                    return;
                }
                let (Some(from_day), Some(to_day)) =
                    (parse_date(&from_input()), parse_date(&to_input()))
                else {
                    status.set(Some("Enter both dates as YYYY-MM-DD.".to_string()));
                    return;
                };
                // The To date includes its whole day.
                let to_time = to_day + DAY_SECONDS - 1;
                if from_day > to_time {
                    status.set(Some(
                        "The start date must not be after the end date.".to_string(),
                    ));
                    return;
                }

                loading.set(true);
                status.set(None);
                spawn(async move {
                    match sync_prices_api(code, from_day, to_time).await {
                        Ok(summary) => {
                            loading.set(false);
                            status.set(Some(format!(
                                "Sync complete: {} USD rows in range, {} stored, {} skipped (no FX rate for their day). Existing rows were left untouched.",
                                summary.source_rows, summary.stored, summary.skipped
                            )));
                            on_synced.call(PriceSyncOutcome::Historical(summary));
                        }
                        Err(error) => {
                            loading.set(false);
                            status.set(Some(format!("Could not sync the prices: {error}")));
                        }
                    }
                });
            }
            SyncMode::ToToday => {
                loading.set(true);
                status.set(None);
                spawn(async move {
                    match sync_prices_to_today_api().await {
                        Ok(summary) => {
                            loading.set(false);
                            status.set(Some(today_status(&summary)));
                            on_synced.call(PriceSyncOutcome::ToToday(summary));
                        }
                        Err(error) => {
                            loading.set(false);
                            status.set(Some(format!("Could not sync the prices: {error}")));
                        }
                    }
                });
            }
        }
    };

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

                div {
                    class: "md3-chip-group",
                    role: "group",
                    aria_label: "Sync mode",
                    for option in [SyncMode::Historical, SyncMode::ToToday] {
                        button {
                            r#type: "button",
                            class: if mode() == option { "md3-chip md3-chip-selected" } else { "md3-chip" },
                            aria_pressed: (mode() == option).to_string(),
                            disabled: *loading.read(),
                            onclick: move |_| {
                                if mode() != option {
                                    mode.set(option);
                                    status.set(None);
                                }
                            },
                            "{option.label()}"
                        }
                    }
                }

                if mode() == SyncMode::Historical {
                    p { class: "text-body-medium text-on-surface-variant",
                        "Convert the stored USD history of the range into another currency. Only close times that already exist in USD are populated, so the result matches the granularity of the table; the FX rates come from Frankfurter."
                    }

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
                            onchange: move |event| selected.set(event.value()),
                            option { value: "", "Select a currency" }
                            match currencies() {
                                Some(Ok(list)) => rsx! {
                                    for currency in list.iter().filter(|entry| entry.code != "USD") {
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
                                    // the built-in list so syncing stays
                                    // available.
                                    for code in FIAT_CURRENCIES.iter().filter(|code| **code != "USD") {
                                        option { value: "{code}", selected: selected() == *code, "{code}" }
                                    }
                                },
                                None => rsx! {
                                    option { value: "", disabled: true, "Loading currencies..." }
                                },
                            }
                        }
                    }

                    div { class: "md3-date-range",
                        div {
                            label {
                                class: "text-body-small text-on-surface-variant",
                                r#for: "sync-prices-from",
                                "From"
                            }
                            DatePicker {
                                id: "sync-prices-from".to_string(),
                                value: from_input(),
                                aria_label: "Sync from date".to_string(),
                                disabled: *loading.read(),
                                popup_above: true,
                                on_change: move |value: String| from_input.set(value),
                            }
                        }
                        div {
                            label {
                                class: "text-body-small text-on-surface-variant",
                                r#for: "sync-prices-to",
                                "To"
                            }
                            DatePicker {
                                id: "sync-prices-to".to_string(),
                                value: to_input(),
                                aria_label: "Sync to date".to_string(),
                                disabled: *loading.read(),
                                popup_above: true,
                                on_change: move |value: String| to_input.set(value),
                            }
                        }
                    }
                } else {
                    p { class: "text-body-medium text-on-surface-variant",
                        "Fetch BTC/USD daily prices from Coin Metrics, starting at the latest stored close time and continuing through the current day. Daily closes are stored, and existing rows are left untouched."
                    }
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
                        } else if mode() == SyncMode::ToToday {
                            "Sync to today"
                        } else {
                            "Sync prices"
                        }
                    }
                }
            }
        }
    }
}

/// Status line of a successful "sync to today" run.
fn today_status(summary: &SyncToTodaySummary) -> String {
    if summary.fetched == 0 {
        return "Sync complete: the stored daily BTC/USD history is already up to date."
            .to_string();
    }
    format!(
        "Sync complete: {} daily BTC/USD prices fetched from {} through {}, {} stored. Existing rows were left untouched.",
        summary.fetched,
        format_date(summary.from_time),
        format_date(summary.to_time),
        summary.stored
    )
}
