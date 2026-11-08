//! Latest BTC price in the app-wide default currency.
use crate::api::prices::latest_price_api;
use crate::components::use_fiat_currency;
use dioxus::prelude::*;
/// Load the most recent BTC price in the default fiat currency.
pub fn use_latest_price() -> Resource<Option<f64>> {
    let fiat = use_fiat_currency();
    use_resource(move || {
        let symbol = fiat.code();
        async move { latest_price_api(symbol).await.ok().map(|price| price.close) }
    })
}
