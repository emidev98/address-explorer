//! CSV file drop zone shared by the exchange ledger importer and the
//! provider catalog importer.

use dioxus::html::HasFileData;
use dioxus::prelude::*;

#[component]
pub fn CsvDropzone(
    file_name: String,
    disabled: bool,
    on_file: EventHandler<(String, String)>,
) -> Element {
    let local_error = use_signal(|| Option::<String>::None);
    let accept = ".csv,.txt";

    rsx! {
        div {
            class: "md3-import-data-dropzone",
            ondragover: move |event| event.prevent_default(),
            ondrop: move |event| {
                event.prevent_default();
                if let Some(file) = event.files().into_iter().next() {
                    read_selected_file(file, on_file, local_error);
                }
            },
            label { class: "md3-import-data-dropzone-inner",
                input {
                    r#type: "file",
                    accept: "{accept}",
                    disabled,
                    class: "md3-import-data-file-input",
                    onchange: move |event| {
                        if let Some(file) = event.files().into_iter().next() {
                            read_selected_file(file, on_file, local_error);
                        }
                    },
                }
                span { class: "md3-import-data-dropzone-icon", "⇧" }
                strong {
                    if file_name.is_empty() {
                        "Choose a CSV file"
                    } else {
                        "Replace CSV file"
                    }
                }
                span { class: "text-body-small text-on-surface-variant",
                    if file_name.is_empty() {
                        "Drop a CSV export here or click to browse"
                    } else {
                        "{file_name}"
                    }
                }
            }
            if let Some(error) = local_error() {
                p { class: "text-body-small md3-exchange-error", "{error}" }
            }
        }
    }
}

fn read_selected_file(
    file: dioxus::html::FileData,
    on_file: EventHandler<(String, String)>,
    mut error: Signal<Option<String>>,
) {
    let name = file.name();
    error.set(None);
    spawn(async move {
        match file.read_string().await {
            Ok(text) => on_file.call((name, text)),
            Err(err) => error.set(Some(err.to_string())),
        }
    });
}
