use dioxus::prelude::*;

use crate::components::{use_toast, Toast};
use crate::domain::address::{
    AddressEntity, AddressImportResult, AddressSaveOutcome, AddressScope,
};

/// Owner kind chosen in the import dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerKind {
    Exchange,
    Wallet,
}

/// Mode of the dialog: import a new address or modify a stored one.
///
/// Import is create-only: an already stored address is rejected as a
/// duplicate and the dialog stays open. Modify updates the description and
/// owner classification of the address it is prefilled with; the address
/// itself is the identity of the row and cannot be edited.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImportAddressDialogMode {
    /// Import a new address (the default).
    #[default]
    Import,
    /// Modify the stored address the dialog is prefilled with.
    Modify(AddressEditData),
}

/// Stored-address data prefilling the dialog in modify mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressEditData {
    pub address: String,
    pub description: Option<String>,
    /// Owner classification, carried with the catalog ids the selects
    /// preselect.
    pub scope: AddressScope,
}

/// Description and owner reference gathered by the dialog for one import.
///
/// At most one of `exchange_id`/`wallet_id` is provided: the dialog renders a
/// catalog select only for the chosen owner kind, so the two selections stay
/// mutually exclusive in the UI, and the server rejects conflicting pairs as
/// a final boundary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ImportDraft {
    description: String,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
}

/// Modal dialog that imports a Bitcoin address or modifies a stored one.
///
/// The dialog is mounted only while it is open, in the given `mode`.
///
/// In **import** mode `clipboard_text` (set only after the page validates it
/// with rust-bitcoin) pastes into the input; nothing is imported until the
/// user reviews the options and confirms. Confirming stores the address and
/// indexes its transaction history through `save_address_api`, which
/// validates the input on the server. An address that is already stored is
/// reported as a duplicate: nothing is saved and the dialog stays open.
///
/// In **modify** mode the form is prefilled from the stored row, the
/// address input is read-only (it is the identity of the row), and
/// confirming updates the description and owner classification through
/// `update_address_api` without touching statistics or indexed
/// transactions.
#[component]
pub fn ImportAddressDialog(
    clipboard_text: Signal<Option<String>>,
    on_close: EventHandler<()>,
    on_imported: EventHandler<AddressImportResult>,
    #[props(default)] on_updated: EventHandler<AddressEntity>,
    #[props(default)] mode: ImportAddressDialogMode,
) -> Element {
    let edit = match &mode {
        ImportAddressDialogMode::Modify(edit) => Some(edit.clone()),
        ImportAddressDialogMode::Import => None,
    };
    let (edit_owner, edit_exchange_id, edit_wallet_id) = match &edit {
        Some(edit) => match &edit.scope {
            AddressScope::Exchange { id, .. } => (Some(OwnerKind::Exchange), Some(*id), None),
            AddressScope::Wallet { id, .. } => (Some(OwnerKind::Wallet), None, Some(*id)),
            AddressScope::Unassigned => (None, None, None),
        },
        None => (None, None, None),
    };

    let toast = use_toast();
    let modifying = edit.is_some();
    let mut address = use_signal(|| edit.clone().map(|edit| edit.address).unwrap_or_default());
    let loading = use_signal(|| false);
    let mut handled_clipboard = use_signal(|| Option::<String>::None);
    let mut description = use_signal(|| edit.and_then(|edit| edit.description).unwrap_or_default());
    let mut selected_owner = use_signal(|| edit_owner);
    let mut exchange_id = use_signal(|| edit_exchange_id);
    let mut wallet_id = use_signal(|| edit_wallet_id);

    let exchanges = use_resource(|| async move {
        crate::api::catalogs::list_exchange_types_api()
            .await
            .map_err(|err| err.to_string())
    });
    let wallets = use_resource(|| async move {
        crate::api::catalogs::list_wallet_types_api()
            .await
            .map_err(|err| err.to_string())
    });

    // A validated clipboard address only fills the input: importing always
    // waits for the user to review the options and confirm. In modify mode
    // the address is the identity of the row, so the clipboard never
    // overwrites it.
    use_effect(move || {
        if modifying {
            return;
        }
        let Some(text) = clipboard_text() else {
            return;
        };
        let clipboard_address = text.trim().to_string();
        if clipboard_address.is_empty() || handled_clipboard() == Some(clipboard_address.clone()) {
            return;
        }
        handled_clipboard.set(Some(clipboard_address.clone()));

        address.set(clipboard_address);
    });

    let close = move |_| on_close.call(());
    let submit = move |_| {
        let mut toast = toast;
        let current = address().trim().to_string();
        if current.is_empty() {
            toast.error("Enter a Bitcoin address.");
            return;
        }

        address.set(current.clone());

        let draft = ImportDraft {
            description: description().trim().to_string(),
            exchange_id: exchange_id(),
            wallet_id: wallet_id(),
        };

        // The owner kind drives which catalog select is shown; a chosen kind
        // without an entry means the selection is not finished yet.
        match selected_owner() {
            Some(OwnerKind::Exchange) if draft.exchange_id.is_none() => {
                toast.error("Choose an exchange to tag the address with.");
                return;
            }
            Some(OwnerKind::Wallet) if draft.wallet_id.is_none() => {
                toast.error("Choose a wallet to tag the address with.");
                return;
            }
            _ => {}
        }

        if modifying {
            update_address(draft, loading, toast, on_updated, current);
        } else {
            import_address(address, draft, loading, toast, on_imported, current);
        }
    };

    let (title, guidance) = if modifying {
        (
            "Modify address",
            "Update the description or the owner of this address. Its statistics and indexed transactions are untouched; use the sync action to refresh them.",
        )
    } else {
        (
            "Import Address",
            "Enter a Bitcoin address or paste one from your clipboard. Confirming imports the address and indexes its transactions, which can take a moment for long histories.",
        )
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
                aria_labelledby: "import-address-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "import-address-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "{title}"
                }
                p { class: "text-body-medium text-on-surface-variant", "{guidance}" }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "import-address-input",
                    "Bitcoin address"
                }
                div { class: "md3-text-field",
                    input {
                        id: "import-address-input",
                        class: "md3-input",
                        value: "{address}",
                        placeholder: "Bitcoin address",
                        autofocus: true,
                        oninput: move |event| address.set(event.value()),
                        disabled: modifying || *loading.read(),
                    }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "import-address-description",
                    "Description (optional)"
                }
                div { class: "md3-text-field",
                    input {
                        id: "import-address-description",
                        class: "md3-input",
                        value: "{description}",
                        placeholder: "e.g. Savings, cold storage",
                        oninput: move |event| description.set(event.value()),
                        disabled: *loading.read(),
                    }
                }

                label { class: "text-body-medium text-on-surface-variant", "Owner (optional)" }
                div {
                    class: "md3-chip-group",
                    role: "group",
                    aria_label: "Owner",
                    button {
                        r#type: "button",
                        class: if selected_owner().is_none() { "md3-chip md3-chip-selected" } else { "md3-chip" },
                        aria_pressed: selected_owner().is_none().to_string(),
                        disabled: *loading.read(),
                        onclick: move |_| {
                            selected_owner.set(None);
                            exchange_id.set(None);
                            wallet_id.set(None);
                        },
                        "No owner"
                    }
                    button {
                        r#type: "button",
                        class: if selected_owner() == Some(OwnerKind::Wallet) { "md3-chip md3-chip-selected" } else { "md3-chip" },
                        aria_pressed: (selected_owner() == Some(OwnerKind::Wallet)).to_string(),
                        disabled: *loading.read(),
                        onclick: move |_| {
                            selected_owner.set(Some(OwnerKind::Wallet));
                            exchange_id.set(None);
                        },
                        "Wallet"
                    }
                    button {
                        r#type: "button",
                        class: if selected_owner() == Some(OwnerKind::Exchange) { "md3-chip md3-chip-selected" } else { "md3-chip" },
                        aria_pressed: (selected_owner() == Some(OwnerKind::Exchange)).to_string(),
                        disabled: *loading.read(),
                        onclick: move |_| {
                            selected_owner.set(Some(OwnerKind::Exchange));
                            wallet_id.set(None);
                        },
                        "Exchange"
                    }
                }

                if let Some(OwnerKind::Wallet) = selected_owner() {
                    div { class: "md3-text-field",
                        select {
                            id: "import-address-wallet",
                            class: "md3-input",
                            value: wallet_id().map(|id| id.to_string()).unwrap_or_default(),
                            onchange: move |event| {
                                wallet_id.set(event.value().parse::<i64>().ok());
                            },
                            disabled: *loading.read(),
                            option { value: "", "Select a wallet type" }
                            if let Some(Ok(list)) = wallets() {
                                for wallet in list {
                                    option { value: "{wallet.id}", "{wallet.name}" }
                                }
                            }
                        }
                    }
                }

                if let Some(OwnerKind::Exchange) = selected_owner() {
                    div { class: "md3-text-field",
                        select {
                            id: "import-address-exchange",
                            class: "md3-input",
                            value: exchange_id().map(|id| id.to_string()).unwrap_or_default(),
                            onchange: move |event| {
                                exchange_id.set(event.value().parse::<i64>().ok());
                            },
                            disabled: *loading.read(),
                            option { value: "", "Select an exchange" }
                            if let Some(Ok(list)) = exchanges() {
                                for exchange in list {
                                    option { value: "{exchange.id}", "{exchange.name}" }
                                }
                            }
                        }
                    }
                }

                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        onclick: close,
                        disabled: *loading.read(),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        onclick: submit,
                        disabled: *loading.read(),
                        if *loading.read() {
                            if modifying {
                                "Saving..."
                            } else {
                                "Importing and indexing..."
                            }
                        } else if modifying {
                            "Save changes"
                        } else {
                            "Import and index"
                        }
                    }
                }
            }
        }
    }
}

fn import_address(
    mut address: Signal<String>,
    draft: ImportDraft,
    mut loading: Signal<bool>,
    mut toast: Toast,
    on_imported: EventHandler<AddressImportResult>,
    address_value: String,
) {
    if *loading.read() {
        return;
    }

    loading.set(true);
    address.set(address_value.clone());

    spawn(async move {
        // Validation with rust-bitcoin (plus the description and owner
        // reference) happens inside the server function before it contacts
        // the mempool or writes to SQLite. The same call indexes the
        // transaction history; its outcome rides along in the result.
        match crate::api::addresses::save_address_api(
            address_value,
            Some(draft.description),
            draft.exchange_id,
            draft.wallet_id,
        )
        .await
        {
            Ok(AddressSaveOutcome::Imported(result)) => {
                loading.set(false);
                on_imported.call(*result);
            }
            // Create-only import: the address is already stored, nothing was
            // saved; the dialog stays open so the user can switch to the
            // modify flow instead.
            Ok(AddressSaveOutcome::Duplicate) => {
                loading.set(false);
                toast.error("This address is already stored. Use its edit action to modify it.");
            }
            Err(error) => {
                loading.set(false);
                toast.error(format!("Could not import address: {error}"));
            }
        }
    });
}

fn update_address(
    draft: ImportDraft,
    mut loading: Signal<bool>,
    mut toast: Toast,
    on_updated: EventHandler<AddressEntity>,
    address_value: String,
) {
    if *loading.read() {
        return;
    }

    loading.set(true);

    spawn(async move {
        match crate::api::addresses::update_address_api(
            address_value,
            Some(draft.description),
            draft.exchange_id,
            draft.wallet_id,
        )
        .await
        {
            Ok(updated) => {
                loading.set(false);
                on_updated.call(updated);
            }
            Err(error) => {
                loading.set(false);
                toast.error(format!("Could not update the address: {error}"));
            }
        }
    });
}
