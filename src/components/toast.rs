//! Transient status messages ("toasts") for everything that happens in the app.
//!
//! Pages and dialogs report outcomes through the app-wide [`Toast`] channel
//! instead of rendering their own status paragraphs. The [`ToastHost`],
//! mounted once at the root, shows the current toast above everything else —
//! including modal dialogs — and dismisses it automatically after a few
//! seconds.

use dioxus::prelude::*;

/// How long a toast stays visible before it is dismissed automatically.
const TOAST_DURATION_MS: u32 = 4_000;

/// Visual and semantic tone of a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// Something completed successfully.
    Success,
    /// Something failed or the input is invalid.
    Error,
    /// Neutral information, e.g. an operation paused or found nothing.
    Info,
}

/// One toast currently on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToastMessage {
    /// Monotonic id: a dismiss timer only clears its own toast, so a new
    /// toast is never cut short by the timer of the previous one.
    id: u64,
    kind: ToastKind,
    text: String,
}

/// App-wide toast channel, provided once in `App` via [`provide_toast`] and
/// consumed with [`use_toast`]. The copyable handle is cheap to capture in
/// event handlers and spawned tasks; the signals inside stay shared.
#[derive(Debug, Clone, Copy)]
pub struct Toast {
    current: Signal<Option<ToastMessage>>,
    next_id: Signal<u64>,
}

impl Toast {
    /// Show a toast, replacing any toast currently visible.
    pub fn show(&mut self, kind: ToastKind, text: impl Into<String>) {
        let id = *self.next_id.read();
        self.next_id.set(id + 1);
        self.current.set(Some(ToastMessage {
            id,
            kind,
            text: text.into(),
        }));
    }

    /// Show a success toast.
    pub fn success(&mut self, text: impl Into<String>) {
        self.show(ToastKind::Success, text);
    }

    /// Show an error toast.
    pub fn error(&mut self, text: impl Into<String>) {
        self.show(ToastKind::Error, text);
    }
}

/// Create the app-wide toast channel and provide it to the component tree.
/// Called once by the root component.
pub fn provide_toast() {
    let toast = Toast {
        current: use_signal(|| None),
        next_id: use_signal(|| 0),
    };
    use_context_provider(|| toast);
}

/// Access the app-wide toast channel.
pub fn use_toast() -> Toast {
    use_context::<Toast>()
}

/// Renders the current toast, if any. Mounted once by the root component,
/// after the router so it layers above page content and modal dialogs.
#[component]
pub fn ToastHost() -> Element {
    let mut toast = use_toast();

    // Auto-dismiss the current toast after a few seconds. The timer runs in
    // the webview because tokio timers do not run on wasm (the same pattern
    // as the clipboard helpers); the id check keeps the timer of an old
    // toast from clearing the toast that replaced it.
    use_effect(move || {
        let Some(message) = toast.current.read().clone() else {
            return;
        };
        let mut toast = toast;
        spawn(async move {
            let mut eval = document::eval(&format!(
                "setTimeout(() => dioxus.send({id}), {TOAST_DURATION_MS});",
                id = message.id,
            ));
            if eval.recv::<u64>().await.is_err() {
                return;
            }
            let still_current = toast
                .current
                .read()
                .as_ref()
                .is_some_and(|current| current.id == message.id);
            if still_current {
                toast.current.set(None);
            }
        });
    });

    let Some(message) = toast.current.read().clone() else {
        return rsx! {};
    };

    let (tone_class, role, live, icon_path) = match message.kind {
        ToastKind::Success => ("md3-toast-success", "status", "polite", ICON_CHECK),
        ToastKind::Error => ("md3-toast-error", "alert", "assertive", ICON_ERROR),
        ToastKind::Info => ("md3-toast-info", "status", "polite", ICON_INFO),
    };

    rsx! {
        div { class: "md3-toast-host",
            div { class: "md3-toast {tone_class}", role, aria_live: live,
                {icon(icon_path)}
                span { class: "md3-toast-text text-body-medium", "{message.text}" }
                button {
                    r#type: "button",
                    class: "md3-icon-button md3-toast-dismiss",
                    title: "Dismiss",
                    aria_label: "Dismiss",
                    onclick: move |_| toast.current.set(None),
                    {icon(ICON_CLOSE)}
                }
            }
        }
    }
}

// Material icon paths (24px viewBox): check circle, error, info, close.
const ICON_CHECK: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z";
const ICON_ERROR: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z";
const ICON_INFO: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z";
const ICON_CLOSE: &str =
    "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";

/// Render one Material icon path as an inline SVG.
fn icon(path: &str) -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "20",
            height: "20",
            fill: "currentColor",
            path { d: "{path}" }
        }
    }
}
