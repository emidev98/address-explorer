//! Latest BTC price in the app-wide default currency.
use crate::api::prices::latest_price_api;
use crate::components::use_fiat_currency;
use crate::domain::price::Price;
use dioxus::prelude::*;
/// Load the most recent BTC price in the default fiat currency.
///
/// The stored close time is kept so callers can show how old the price is
/// when the sources cannot be reached and the database answers instead.
pub fn use_latest_price() -> Resource<Option<Price>> {
    let fiat = use_fiat_currency();
    use_resource(move || {
        let symbol = fiat.code();
        async move { latest_price_api(symbol).await.ok() }
    })
}
