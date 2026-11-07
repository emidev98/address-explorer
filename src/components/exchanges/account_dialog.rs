use super::*;
use dioxus::prelude::*;

/// Edit an exchange account's name and provider.
#[component]
pub fn ExchangeAccountDialog(
    account: ExchangeAccount,
    on_close: EventHandler<()>,
    on_saved: EventHandler<ExchangeAccount>,
) -> Element {
    let mut toast = use_toast();
    let mut name = use_signal(|| account.name.clone());
    let mut provider_id = use_signal(|| account.provider_id);
    let mut loading = use_signal(|| false);
    let providers = use_resource(|| async move {
        catalog_options_api(CatalogKind::Exchange.as_str().to_string())
            .await
            .map_err(|error| error.to_string())
    });
    let account_id = account.id;

    let save = move |_| {
        if name().trim().is_empty() {
            toast.error("Enter an exchange name.");
            return;
        }
        let name_value = name().trim().to_string();
        let provider_value = provider_id();
        loading.set(true);
        spawn(async move {
            match crate::api::exchanges::update_exchange_account_api(
                account_id,
                name_value,
                provider_value,
            )
            .await
            {
                Ok(updated) => on_saved.call(updated),
                Err(error) => toast.error(format!("Could not update the exchange: {error}")),
            }
            loading.set(false);
        });
    };

    let provider_entries = providers().and_then(|state| state.ok()).unwrap_or_default();
    let providers_loading = providers().is_none();

    rsx! {
    Modal {
        title: "Edit exchange".to_string(),
        title_id: "exchange-edit-title".to_string(),
        on_close: move |_| on_close.call(()),
        label { class: "text-body-medium text-on-surface-variant", "Exchange name *" }
            div { class: "md3-text-field",
                input {
                    class: "md3-input",
                    value: "{name}",
                    disabled: *loading.read(),
                    oninput: move |event| name.set(event.value()),
                }
            }
            label { class: "text-body-medium text-on-surface-variant", "Provider *" }
            CatalogOptionsSelect {
                kind: CatalogKind::Exchange,
                value: provider_id(),
                on_change: move |selected| provider_id.set(selected),
                disabled: *loading.read(),
                id: "exchange-edit-provider".to_string(),
                placeholder: "Manual Import".to_string(),
                entries: provider_entries,
                loading: providers_loading,
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *loading.read(),
                    onclick: move |_| on_close.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *loading.read(),
                    onclick: save,
                    if *loading.read() {
                        "Saving..."
                    } else {
                        "Save"
                    }
                }
            }
        }
    }
}
