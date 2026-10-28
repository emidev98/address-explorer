use dioxus::prelude::*;

use crate::app::Route;

/// Top-level navigation bar. Renders one entry per application screen and
/// highlights the currently active screen.
#[component]
pub fn Topbar() -> Element {
    let current = use_route::<Route>();

    let screens = [
        (Route::Addresses {}, "Addresses"),
        (Route::Flow {}, "Flow"),
        (Route::Exchanges {}, "Exchanges"),
        (Route::Prices {}, "Historical Prices"),
        (Route::Settings {}, "Settings"),
    ];

    rsx! {
        header { class: "md3-top-app-bar",
            nav {
                for (route, label) in screens {
                    Link {
                        to: route.clone(),
                        class: if current == route { "active" } else { "" },
                        "{label}"
                    }
                }
            }
        }
        main { id: "screens", Outlet::<Route> {} }
    }
}
