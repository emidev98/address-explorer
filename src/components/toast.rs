//! Transient status messages ("toasts") for everything that happens in the app.
//!
//! Pages and dialogs report outcomes through the app-wide [`Toast`] channel
//! instead of rendering their own status paragraphs. The [`ToastHost`],
//! mounted once at the root, shows the current toast above everything else —
//! including modal dialogs — and dismisses it automatically after a few
//! seconds.
use super::icon::{Icon, IconName};
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
/// App-wide toast channel, provided once in `App` via [`use_toast_provider`] and
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
///
/// Named as a hook (`use_*`) because it calls `use_signal` and
/// `use_context_provider`: the name is what tells `dx check` that a hook may
/// be called here, and it must only ever be invoked from a component body.
pub fn use_toast_provider() {
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
    let (tone_class, role, live, icon) = match message.kind {
        ToastKind::Success => (
            "md3-toast-success",
            "status",
            "polite",
            IconName::CheckCircle,
        ),
        ToastKind::Error => (
            "md3-toast-error",
            "alert",
            "assertive",
            IconName::ErrorCircle,
        ),
        ToastKind::Info => ("md3-toast-info", "status", "polite", IconName::InfoCircle),
    };
    rsx! {
        div { class: "md3-toast-host",
            div { class: "md3-toast {tone_class}", role, aria_live: live,
                Icon { name: icon, size: 20 }
                span { class: "md3-toast-text text-body-medium", "{message.text}" }
                button {
                    r#type: "button",
                    class: "md3-icon-button md3-toast-dismiss",
                    title: "Dismiss",
                    aria_label: "Dismiss",
                    onclick: move |_| toast.current.set(None),
                    Icon { name: IconName::Close, size: 20 }
                }
            }
        }
    }
}
