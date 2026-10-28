# Contributing Guide

This document is the single canonical source for repository development instructions: Dioxus 0.7 conventions, Rust architecture and clean-code standards, `rusqlite`/SQLite persistence, testing, platform compatibility, and code review. Agent-assisted changes and human contributions **MUST** follow the same requirements. The former Dioxus-specific instructions have been merged into this guide; no separate policy document is required.

The repository configuration (`Cargo.toml`, `Cargo.lock`, `Dioxus.toml`, CI definitions, and established project conventions) is authoritative. If a guide and the actual configuration disagree, contributors **MUST** resolve the discrepancy deliberately and update this document or the project configuration rather than silently changing project-wide assumptions.

## 1. Normative language (RFC 2119 / RFC 8174)

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **NOT RECOMMENDED**, **MAY**, and **OPTIONAL** are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174). Only these words written in all uppercase have their defined normative meaning.

- **MUST / MUST NOT** specify an absolute requirement or prohibition.
- **SHOULD / SHOULD NOT** specify a strong recommendation. A deviation requires a specific, understood reason; contributors **SHOULD** document that reason in the pull request or review discussion.
- **MAY / OPTIONAL** identify a permitted choice, not a requirement.

These meanings apply to this guide and its normative statements. Lowercase forms in ordinary prose do not carry the RFC-defined meaning.

## 2. Engineering principles and source of truth

Contributions **MUST** prioritize correctness, clarity, security, accessibility, and maintainability over cleverness or premature optimization. The detailed requirements below cover UI architecture, persistence, clean code, testing, and platform compatibility; contributors **SHOULD** use those sections as the implementation checklist rather than maintaining separate local rules that diverge from this guide.

When sources disagree, contributors **MUST** prioritize project configuration and code verified by tests, followed by official documentation for the exact dependency version, this guide, and then third-party tutorials or blog posts. An existing implementation is not proof that a behavior is intentional or safe.

## 3. Supported stack and repository layout

- **Language:** stable Rust unless the repository explicitly documents a toolchain exception.
- **UI:** Dioxus. The version pinned by `Cargo.toml`/`Cargo.lock` determines the usable API. Section 5 describes the Dioxus conventions; contributors **MUST** update that section deliberately when the dependency baseline changes rather than mixing APIs.
- **Charts:** Apache ECharts, vendored in `assets/vendor/echarts/`, is the default and only charting library. Section 5.10 describes the required chart component boundaries.
- **Persistence:** SQLite through `rusqlite` for native application builds. Browser/WASM builds **MUST NOT** assume that a native `rusqlite` persistence implementation can be reused unchanged; a browser target requires an explicitly supported storage adapter.
- **Targets:** Linux, macOS, and Android. Platform-specific APIs **SHOULD** be isolated behind narrow modules or target-specific compilation.

Contributors **MUST NOT** upgrade Dioxus, `rusqlite`, SQLite features, or other major dependencies as an incidental part of an unrelated change. Dependency changes **SHOULD** state the reason, compatibility impact, and tests performed. For an application, `Cargo.lock` **MUST** be committed so dependency resolution is reviewed and reproducible.

### Suggested project layout

The layout example **SHOULD** be adapted to the repository as it exists. Contributors **MUST NOT** reorganize the project solely to match this example.

```text
src/
├── main.rs                 # Application entry point and composition root
├── app.rs                  # Root component and top-level navigation
├── api/                    # Server-function API controllers (fullstack entry points)
├── components/             # Reusable presentation components
│   └── charts/             # One component per chart (Apache ECharts)
├── screens/                # Screen/page components and screen-specific state
├── application/            # Services: use cases and orchestration, one module per domain
├── domain/                 # Domain types, validation, pure business rules
├── data/
│   ├── mod.rs
│   ├── database.rs         # Connection creation and SQLite configuration
│   ├── migrations.rs       # Ordered schema migrations
│   └── repositories/       # SQL access grouped by domain entity
├── clients/                # External API clients, one module per provider
├── workers/                # CPU-intensive work and long-running workers
└── error.rs                # Shared application error types, if needed
```

A small application **MAY** use fewer modules. Modules **SHOULD** be introduced when they establish a meaningful boundary, not to increase the number of files. A separate `core` crate **MAY** be useful when the same domain/computation logic must be tested independently or reused by multiple frontends; it is a Rust library boundary, not a separate process or service.

## 4. Development setup and quality commands

Contributors **SHOULD** install the stable Rust toolchain, platform prerequisites for the selected Dioxus renderer, and the Dioxus CLI version recommended for the pinned dependency. Linux desktop builds generally require the GTK/WebKitGTK development libraries for the selected renderer. Android development requires the applicable Android SDK/NDK and Java toolchain. Contributors **MUST** follow the official, version-matched platform guide for exact package names and setup steps.

Typical checks from the repository root are:

```sh
# Apply Rust formatting
cargo fmt --all

# Format RSX when supported by the pinned Dioxus CLI
dx fmt

# Compile and test the workspace
cargo check --workspace
cargo test --workspace

# Treat Clippy warnings as errors
cargo clippy --workspace --all-targets -- -D warnings

# Inspect commands supported by the pinned Dioxus CLI
dx --help
```

Before opening a pull request, contributors **MUST** run Rust formatting, compilation, tests, and Clippy for the affected workspace. If a command is unavailable for the pinned toolchain, contributors **SHOULD** use the documented equivalent and **MUST** record the limitation in the pull request. `dx fmt` **SHOULD** be run when RSX has changed and the installed CLI supports it. Platform-specific build or bundle commands **SHOULD** be run for every affected target.

Dioxus bundling is platform-sensitive. A successful build on one operating system does not prove that a native bundle for another operating system can be produced from that host. Release automation **SHOULD** use a build matrix with native runners wherever cross-compilation is unsupported or unreliable. Signing, notarization, store submission, and release credentials **MUST** be handled outside ordinary development checks and **MUST NOT** be committed to the repository.

## 5. Dioxus application development

This section defines the Dioxus-specific conventions for the application. The example API baseline is **Dioxus 0.7.1**; the version pinned in `Cargo.toml` and `Cargo.lock` always takes precedence. Contributors **MUST NOT** mix APIs from different Dioxus versions. In Dioxus 0.7, the legacy `cx`, `Scope`, and `use_state` APIs **MUST NOT** be used.

### 5.1 Dependency features and application entry point

The Dioxus dependency and feature selection **MUST** match the targets the application intends to build. The following is an illustrative feature mapping, not a requirement to enable every target or to replace the repository's existing manifest:

```toml
[dependencies]
dioxus = "0.7.1"

[features]
default = ["desktop"]
web = ["dioxus/web"]
desktop = ["dioxus/desktop"]
mobile = ["dioxus/mobile"]
```

The default feature **SHOULD** reflect the normal local development target, and build commands **MUST** select the intended target features. Fullstack/server features **MUST NOT** be enabled merely to build a standalone native application; they are appropriate only when the product intentionally includes server functionality.

A native or web application **MUST** initialize the Dioxus runtime through the API documented for its pinned version. The minimal Dioxus 0.7 shape is:

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! { "Hello, Dioxus!" }
}
```

### 5.2 Components, props, and RSX

- UI markup **MUST** use `rsx!`; components **MUST** use `#[component]` and a naming convention supported by the pinned Dioxus version, normally `UpperCamelCase`.
- Component props **SHOULD** be owned values. For ordinary props, prefer `String` and `Vec<T>` over borrowed `&str` and slices. Props and custom prop types **MUST** satisfy the trait bounds required by the pinned Dioxus component macro, including `Clone` and `PartialEq` where required.
- Components **SHOULD** render from props and reactive state. They **MUST NOT** perform blocking database, filesystem, network, or CPU-intensive work as an incidental rendering side effect.
- Straightforward repeated and conditional nodes **SHOULD** use RSX's native `for` and `if` forms. Rust expressions embedded in RSX **MUST** be enclosed in braces; iterator expressions used to produce nodes **MUST** also be enclosed in braces.
- Attributes, event handlers, and inline styles **SHOULD** remain readable. Complex expressions **SHOULD** be computed before the `rsx!` block when that makes the markup easier to understand.

Example: repeated and conditional nodes.

```rust
rsx! {
    section {
        class: "items",
        for item in items {
            ItemRow { item }
        }
        if show_empty_state {
            p { "No items found." }
        }
    }
}
```

### 5.3 Assets, styles, and accessibility

- Local project assets **SHOULD** be referenced with `asset!` rather than hard-coded runtime filesystem paths.
- Paths passed to `asset!` **MUST** start with `/` and be interpreted relative to the configured project asset root for the pinned Dioxus/Manganis version.
- A project stylesheet **SHOULD** use the documented `document::Stylesheet` component or the project's established stylesheet configuration.
- Images **SHOULD** have meaningful `alt` text unless they are purely decorative and receive an appropriate accessibility treatment.

Example: local assets and a stylesheet.

```rust
rsx! {
    document::Stylesheet {
        href: asset!("/assets/styles.css"),
    }
    img {
        src: asset!("/assets/image.png"),
        alt: "Descriptive alternative text",
    }
}
```

#### Styling: Tailwind CSS v4 and Material 3

The interface uses a Material 3 theme implemented with Tailwind CSS v4 in
`tailwind.css`. `assets/tailwind.css` is the compiled stylesheet that the
application loads, so contributors **MUST** rebuild it after changing either
the theme or the Tailwind classes in the Rust views:

```bash
npx @tailwindcss/cli@4.1.5 -i tailwind.css -o assets/tailwind.css
```

The theme provides:

- Material 3 color roles as utilities, such as `bg-primary`,
  `bg-surface-container`, and `text-on-surface-variant`.
- The full type scale, such as `text-headline-medium`, `text-body-medium`,
  and `text-label-large`.
- Shape tokens (`rounded-md3-md`), elevation (`shadow-md3-1`), and motion
  easings (`ease-md3-standard`).
- Component classes used by the views, including `md3-top-app-bar`,
  `md3-screen`, `md3-button-filled`, `md3-text-field`, `md3-table`,
  `md3-card`, and `md3-fab`.

The available palettes are Ocean, Forest, Sunset, Orchid (the default), and
Graphite Lime. `data-theme` selects the palette and `data-mode` selects
`light`, `dark`, or `system`; both are applied to the document root. When no
mode has been stored yet, the theme follows `prefers-color-scheme`.

The palette and appearance are changed in **Settings → Theme**. Preferences
are saved automatically to the SQLite `setting` table, created by
`src/clients/database/migrations/0003_settings.sql`. The theme is loaded
during server rendering, applied in the document head before the app markup
is parsed, and serialized into the page so hydration starts with the same
values instead of flashing the default palette. Palette definitions live in
`assets/material3-tailwind-palettes.css`, which `tailwind.css` imports. The
theme aliases the `--md-*` palette variables to the `--md-sys-color-*` roles,
so palette changes affect both the component classes and the Tailwind color
utilities. Changes that alter palettes, color roles, or component classes
**MUST** be verified in both light and dark appearance modes before review.

Charts draw on a canvas and cannot consume the component classes or Tailwind
utilities: when a new style is added it **MUST** also be available for the
charts. New color roles and visual treatments **MUST** be expressed through
the chart theme tokens in `src/components/charts/theme.rs`, which reference
the same `--md-sys-color-*` variables (Section 5.10); chart styling **MUST
NOT** fork the theme with hardcoded colors.

### 5.4 Signals, derived state, context, and hooks

- Local reactive state **SHOULD** use `use_signal`. Values derivable from other reactive values **SHOULD** use direct expressions or `use_memo` rather than a second independently mutable source of truth.
- Hooks **MUST** be called in a consistent order during component evaluation; they **MUST NOT** be called conditionally inside branches or loops.
- Shared state **MAY** be passed through props or provided with `use_context_provider` and retrieved with `use_context`. The choice **SHOULD** make ownership and dependencies clear.

Counter example:

```rust
#[component]
fn Counter() -> Element {
    let mut count = use_signal(|| 0_i64);
    let doubled = use_memo(move || count() * 2);

    rsx! {
        p { "Count: {count}" }
        p { "Doubled: {doubled}" }
        button {
            onclick: move |_| *count.write() += 1,
            "Increment"
        }
    }
}
```

Context example:

```rust
#[component]
fn App() -> Element {
    let theme = use_signal(|| "light".to_string());
    use_context_provider(|| theme);
    rsx! { ThemeLabel {} }
}

#[component]
fn ThemeLabel() -> Element {
    let theme = use_context::<Signal<String>>();
    rsx! { p { "Current theme: {theme}" } }
}
```

### 5.5 Rendering, event handlers, and persistence lifecycle

- Components **SHOULD NOT** contain substantial SQL, domain algorithms, complex parsing, or multi-step workflows. Those operations **SHOULD** live in application, domain, worker, or data modules.
- Event handlers **MAY** validate simple input, update transient UI state, and start an application operation. They **MUST NOT** synchronously wait for blocking I/O or a long calculation.
- Reactive state **SHOULD** represent transient interface state, such as selected tabs, form values, loading status, or displayed results. SQLite **MUST** remain the durable source of truth for persisted records; a signal showing a saved value **MUST NOT** be treated as proof that a database transaction committed successfully.
- Writes **MUST** occur in explicit user actions or application workflows, not repeatedly because a component re-rendered. Effects **SHOULD** be reserved for lifecycle-related or reactive side effects; user-initiated operations **SHOULD** start from their event handler or application service.
- Form values **MUST** be validated at the domain/application boundary, not only by visual constraints. User-facing errors **SHOULD** be understandable and actionable; SQL text, stack traces, filesystem internals, and sensitive data **MUST NOT** be exposed to users.

### 5.6 Resources, async I/O, CPU work, and cancellation

`use_resource` **SHOULD** be used for component data backed by an asynchronous operation when its dependency-tracking and lifecycle behavior fit the use case. A resource's initial/loading state **MUST** be handled explicitly. Where relevant, the UI **SHOULD** distinguish loading, success, empty, and failure states.

Illustrative resource shape:

```rust
let record = use_resource(move || async move {
    load_record().await
});

match record() {
    Some(Ok(value)) => rsx! { RecordView { value } },
    Some(Err(_error)) => rsx! { "Unable to load the record." },
    None => rsx! { "Loading..." },
}
```

Adapt the result type and error presentation to the actual application API. The example is not a requirement to use `use_resource` for every operation.

Async execution and parallel computation are different: an async task does not automatically move CPU-heavy synchronous work onto a dedicated thread. Therefore:

- SQLite connection ownership and worker constraints **MUST** follow Section 6.2.
- CPU-heavy algorithms **MUST** use a suitable blocking-task facility, dedicated worker, or thread pool such as Rayon when background or parallel execution is required.
- Network and other non-blocking I/O **SHOULD** use async APIs when suitable.
- Worker results **MUST** return through a mechanism supported by the pinned Dioxus runtime. Worker threads **MUST NOT** rely on undocumented cross-thread signal access.
- Long-running work **SHOULD** expose progress and cancellation when that materially improves usability. Async workflows **MUST** define relevant cancellation behavior; cancellation **MUST NOT** leave partially committed data or a permanently active loading indicator.
- Background work **MUST** have a clear lifetime and shutdown strategy. Detached infinite loops and unbounded thread creation are **NOT RECOMMENDED**. Locks **SHOULD NOT** be held across `.await` points; critical sections **SHOULD** remain short and release locks before yielding.

CPU-intensive algorithms **SHOULD** be ordinary Rust functions with clear inputs and outputs, testable without Dioxus. Rayon **MAY** be used for data-parallel work when the workload justifies the dependency and overhead. A dedicated `core` crate **MAY** be useful when logic needs independent testing or reuse, but it remains a library compiled into the application, not a separate process or service.

### 5.7 Routing

When the application needs in-app URL routing, contributors **MAY** use Dioxus Router. The `router` feature **MUST** be enabled before using the API. Routes **SHOULD** be declared in one `Routable` enum; shared page structure **SHOULD** use layouts and `Outlet<Route>` where appropriate. Routing **SHOULD NOT** be introduced for a simple single-screen interface unless it materially improves navigation or structure.

Example: route definition and router entry point.

```rust
use dioxus::prelude::*;

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/items/:id")]
    Item { id: i64 },
}

#[component]
fn App() -> Element {
    rsx! { Router::<Route> {} }
}
```

### 5.8 Fullstack and hydration boundaries

When fullstack/server functionality is intentionally enabled, server functions **MUST** be treated as client/server boundaries: server-only operations **MUST NOT** be assumed to execute locally in a native application without a configured server.

For a server-rendered web application, the initial client render **MUST** match the server-rendered UI for hydration to succeed. In a fullstack SSR flow, server-dependent initial data **SHOULD** use the documented `use_server_future` mechanism where appropriate. Browser-only APIs **MUST** run only after hydration, for example in `use_effect`, and **MUST NOT** be accessed during server rendering. These requirements apply only when the project intentionally enables and deploys the relevant fullstack/SSR architecture.

### 5.9 Dioxus development workflow

Contributors **SHOULD** use the Dioxus CLI version recommended for the dependency baseline and commands documented for the intended target. `dx serve` **MAY** be used for local development when configured for that target. Formatting commands are defined in Section 4; platform bundling, system dependencies, and release packaging are covered in Sections 4 and 10. Version-matched official Dioxus documentation takes precedence over outdated snippets.

### 5.10 Charts (Apache ECharts)

Apache ECharts is the default charting library for this project. The browser bundle and its licenses are vendored under `assets/vendor/echarts/` so charts work without a CDN or network connection. Contributors **MUST NOT** add another charting library, load ECharts from a third-party CDN, or enable a second charting stack without updating this guide and documenting the reason.

- Every chart **MUST** be its own dedicated component, normally under `src/components/charts/`. The component **MUST** own the complete presentation: the ECharts option, layout, colors, labels, arrows, tooltips, animations, responsive behavior, and chart-specific interactions.
- Chart components **MUST** receive only their business data, plus optional presentation metadata such as an accessible label. A screen or page **MUST NOT** build an ECharts option, call the ECharts JavaScript API, embed chart JSON, or choose chart internals; it passes data to the chart component and lets the component decide how to display it.
- Chart props and data types **MUST** stay domain-oriented. They **MUST NOT** expose raw ECharts options, canvas elements, or other rendering details to the calling screen. The component converts its data into an option internally.
- Chart components **MUST NOT** fetch, persist, or own business records. Loading, empty, and error states belong to the screen or application layer that supplies the data. Pure helpers such as layout calculation and option construction **SHOULD** be separate functions with unit tests.
- The shared hosts in `src/components/charts/echarts.rs` — `EChartsHost`, plus `EChartsZoomHost` for charts that forward ECharts interaction events such as `datazoom` back to Rust — are the only place that may initialize, update, resize, or dispose ECharts. A chart component passes its serialized option to one of those hosts instead of manipulating the DOM or JavaScript directly.
- Options **MUST** be generated with typed serialization such as `serde_json`, never by concatenating untrusted values into JavaScript source. Any custom JavaScript **MUST** escape data with the same care.
- Chart elements that should follow the app theme **MUST** use the color tokens from `src/components/charts/theme.rs` instead of hardcoded colors. A token is a `var(--md-sys-color-*)` string referencing the same Material 3 color role the CSS uses, with an optional `~alpha` suffix (for example `var(--md-sys-color-primary~0.15)`) for translucent fills the canvas cannot express through CSS. The host resolves every token in the option against the computed page style when it applies the option, and re-applies all options when `data-theme`, `data-mode`, or the system color scheme changes, so charts track palette and appearance switches without a reload. When filling a shape that carries text, the fill and label colors **MUST** be a paired role and its `on-*` counterpart (for example `primary` with `on-primary`) so the palette keeps the contrast it guarantees.
- Charts **MUST** provide an accessible label and remain legible in the supported light and dark appearance modes. Contributors **SHOULD** verify the rendered chart, resizing, and both modes before requesting review.
- When updating ECharts, replace the vendored bundle and license files together, record the new version in this guide or the vendoring README, and run the checks in Section 4.

Example chart component shape:

```rust
/// Business data received from the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct BalancePoint {
    pub label: String,
    pub value: f64,
}

/// The component owns option construction and rendering.
#[component]
pub fn BalanceChart(points: Vec<BalancePoint>) -> Element {
    let option = build_option(&points).to_string();

    rsx! {
        EChartsHost { option, aria_label: "Balance chart".to_string() }
    }
}
```

The screen only supplies data:

```rust
rsx! {
    BalanceChart { points }
}
```

### 5.11 Data tables: pagination and sorting

Every screen that lists rows in a table **MUST** use the shared backend-driven table pattern defined here, so all tables paginate and sort the same way. The historical prices screen (`src/screens/prices.rs`) is the reference implementation.

**Query contract.**

- A table listing endpoint **MUST** be a server function taking a single query struct that flattens the shared [`TableQuery`](src/domain/table.rs) (`#[serde(flatten)]`) and adds the table's specific filters as typed fields (for example `PriceTableQuery` adds `symbol`, `from_time`, `to_time`). It **MUST** return the shared `TablePage<T>` (`items` plus the filtered `total` across all pages).
- `TableQuery` carries the 0-based `page`, the `page_size`, and the optional `sort_by` key with its `sort_dir`. Page numbers are 0-based in the query and 1-based in the UI. Services **MUST** clamp `page_size` to a documented maximum.
- Any change to filters or sorting **MUST** reset the page to 0.

**Pagination bar.**

- Screens **MUST** render the shared `TablePagination` component (from `src/components/table_controls.rs`) directly above the table: Previous and Next buttons around an editable 1-based page index and the totals (` of N · M entries`).
- The page index **MUST** be editable: entering a number navigates to that page, clamped to the valid range. The endpoint **MUST** therefore accept arbitrary page indexes, not only previous/next steps.

**Sorting.**

- Sorting **MUST** run in the backend, never in the UI over one loaded page. Column headers of sortable columns **MUST** use the shared `SortableHeader` component; clicking the active column toggles its direction, clicking another column selects it.
- Sortable columns **MUST** be a fixed allowlist enum owned by the table's repository (for example `PriceSortColumn`), parsed from the client's string key and mapped to a fixed `ORDER BY` expression. Client input **MUST NOT** reach SQL as an identifier (Section 6.3), and an unknown sort key **MUST** be rejected at the service boundary.
- The `ORDER BY` expression **MUST** sort by the column's typed storage class: dates and timestamps as epoch-integer (or ISO date) columns, numbers as `REAL`/`INTEGER` columns — never as formatted display strings. Dates must compare as dates and numbers as numbers ("2024-…" and `100.0 > 20.0`, never lexicographic `"100.0" < "20.0"`). A deterministic secondary key (for example the primary key or timestamp) **MUST** follow the sort column so paginating a sorted listing never overlaps or skips rows.

**Extending the pattern.**

- When a table's behavior changes, the change **MUST** extend the shared definitions rather than hand-rolling equivalents: a new sortable column goes to the repository's allowlist enum and gains a `SortableHeader`, and a new filter becomes a typed field of the endpoint's query struct. Screens **MUST NOT** implement their own pagination bar or sort headers.
- A new table **MUST** be built from these pieces: a `TableQuery`-flattening endpoint, a service that validates the sort key and clamps the page size, repository `WHERE` fragments shared between the list and count queries, and the shared controls from `src/components/table_controls.rs`.

## 6. SQLite and `rusqlite` conventions

### 6.1 Connection lifecycle and file location

- The application **MUST** store a writable database in the operating system's application-data directory (or an equivalent platform-approved location), not in the source tree, current working directory, or read-only packaged resources.
- The database directory **MUST** be created before opening a new database file. Failure to create or open it **MUST** be reported as an application error.
- Database paths **MUST NOT** be hard-coded to a specific user's home directory. Paths **MUST** be built with `Path`/`PathBuf` or a platform-aware directory API.
- The database filename and application-data directory **SHOULD** remain stable across releases so upgrades can find existing user data.
- Tests **SHOULD** use an in-memory database for isolated repository tests and a temporary on-disk database for reopening, persistence, or file-system behavior.
- A `rusqlite::Connection` **SHOULD** have a clear owner and lifecycle. A connection **MUST NOT** be stored in a Dioxus UI signal.

Contributors **SHOULD** choose deliberately whether to use `rusqlite`'s `bundled` feature. Bundling SQLite can make the SQLite version and native dependency more consistent and reduce reliance on a system SQLite development package, but it can increase compile time and **MUST** be verified for every target. When using it, contributors **MUST** keep the dependency range compatible with the repository's supported version and **MUST** commit the lockfile. If linking system SQLite instead, CI and packaging documentation **MUST** describe the required system library and minimum version.

### 6.2 Threading and responsiveness

A `rusqlite::Connection` implements `Send`, but not `Sync`; it is not a general-purpose shared connection for simultaneous use from arbitrary threads. Contributors **MUST NOT** bypass the type system or use `unsafe` to share a connection concurrently.

**RECOMMENDED** default: serialize database access through a dedicated database worker or clearly owned worker context. That worker owns its connection, accepts typed requests, and returns owned result values. A smaller application **MAY** open a connection for an operation or use a mutex-protected connection where appropriate, provided access is serialized and blocking work remains off the UI path. A mutex **MUST NOT** be held while waiting for unrelated UI input, making network calls, or running unrelated CPU-heavy work.

SQLite supports concurrent readers but only one active writer per database at a time. Therefore:

- Contributors **SHOULD NOT** create a new operating-system thread for every query.
- Write operations that form one logical change **MUST** use a transaction.
- Lock waits **SHOULD** be bounded with an intentional busy-timeout policy if concurrent access is expected.
- WAL mode **MAY** help workloads with concurrent readers and a writer, but it is not a default requirement. It **SHOULD** be tested on all supported platforms and **MUST NOT** be assumed to remove SQLite's single-writer constraint.
- Database operations **MUST NOT** hold a global application lock while performing unrelated computation or waiting for user input.

### 6.3 SQL safety and repository boundaries

- External or user-provided SQL values **MUST** be bound with positional or named parameters (`?1`, `:name`, `params!`, or `named_params!`). They **MUST NOT** be inserted into SQL through `format!`, concatenation, or interpolation.
- SQL identifiers such as table names, column names, and sort directions cannot generally be bound as values. If identifiers are dynamic, they **MUST** be selected from a fixed allowlist.
- Queries **SHOULD** select explicit columns rather than `SELECT *`, so schema dependencies remain visible in review.
- Repository functions **SHOULD** return domain types rather than expose `rusqlite::Row`, `Statement`, or connection details to UI components.
- Prepared statements **MAY** be cached when queries repeat and caching improves clarity or measured performance.
- Database errors **SHOULD** retain their source for diagnostics while being translated to a safe user-facing message at the UI boundary.
- Dynamic query builders **SHOULD NOT** be introduced unless they solve a real query requirement more clearly than fixed queries.

Example repository functions:

```rust
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub title: String,
}

pub fn find_note(conn: &Connection, id: i64) -> rusqlite::Result<Option<Note>> {
    conn.query_row(
        "SELECT id, title FROM notes WHERE id = ?1",
        [id],
        |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        },
    )
    .optional()
}

pub fn rename_note(conn: &Connection, id: i64, title: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE notes SET title = ?1 WHERE id = ?2",
        params![title, id],
    )
}
```

The caller is responsible for running these synchronous functions on an appropriate database worker, not directly in a UI handler.

### 6.4 Transactions and consistency

- A group of operations representing one logical change **MUST** be atomic and **MUST** run in a transaction.
- Transactions **MUST** be committed explicitly after all operations succeed. An uncommitted `rusqlite::Transaction` rolls back when dropped by default; code **SHOULD** rely on this as the failure path, not as an obscure control-flow technique.
- Transactions **SHOULD** be short. Code **SHOULD NOT** perform network requests, wait on channels, or run CPU-heavy analysis while a transaction is open.
- Constraints such as `PRIMARY KEY`, `UNIQUE`, `NOT NULL`, `CHECK`, and foreign keys **SHOULD** encode invariants in the schema when feasible.
- Foreign-key enforcement **MUST** be enabled on every connection where the schema relies on it, for example using `PRAGMA foreign_keys = ON`.
- Code **MUST** distinguish “not found” from other query failures when the domain semantics require that distinction.

Example transaction:

```rust
use rusqlite::{Connection, Result, TransactionBehavior};

pub fn create_note_and_audit(conn: &mut Connection, title: &str) -> Result<i64> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    tx.execute("INSERT INTO notes (title) VALUES (?1)", [title])?;
    let note_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO audit_log (entity_id, action) VALUES (?1, ?2)",
        (note_id, "note_created"),
    )?;

    tx.commit()?;
    Ok(note_id)
}
```

Code **SHOULD** use the application's error type around `rusqlite::Result` when a workflow combines database errors with domain validation or other operations.

### 6.5 Schema migrations and initialization

Schema changes **MUST** be versioned and repeatable. A contributor changing the schema **MUST** include a migration and tests for a fresh database and the relevant supported previous schema version.

- Migrations **SHOULD** use ordered, checked-in SQL migration files or a small explicit migration module; the repository **SHOULD** use one approach consistently.
- SQLite's `PRAGMA user_version` **MAY** be used for simple local schema versioning, provided each migration updates it in the same transaction as its schema changes.
- Each migration **MUST** run transactionally where SQLite permits it and **MUST** fail loudly when an unexpected schema version is encountered.
- Migrations **MUST NOT** silently discard user data. Destructive changes **MUST** have an explicit migration strategy, review, and tests.
- A migration that may already have shipped **MUST NOT** be edited in place; contributors **MUST** add a new migration instead.
- Migrations **SHOULD** be deterministic. The migration runner **MUST** track applied versions so an already-applied migration is not run again.

Database initialization **MUST** follow this order: resolve the platform-approved application-data directory; create it if needed; open and configure the connection; run pending migrations; then permit normal repository operations. The exact path-resolution mechanism is platform-specific and **MUST NOT** assume the current working directory is writable.

Each connection **MUST** apply the settings the schema relies on, such as foreign-key enforcement and a deliberate busy timeout. Connection pragmas **SHOULD** be chosen intentionally and **MUST** be tested on every supported target.

## 7. Single Responsibility Principle and module design

The Single Responsibility Principle (SRP) states that a module, struct, or function **SHOULD** have exactly one responsibility — one reason to change. In this repository, a "reason to change" is a single actor or concern that can request a change: the UI design, a business rule, the database schema, an external API contract, or a platform API. A unit of code that changes for two different reasons **MUST** be split.

### 7.1 Applying SRP at every level

SRP applies recursively — to the crate, each module, each struct, and each function:

- Every module **MUST** have a single responsibility that can be stated in one sentence without "and" (for example: "`frankfurter::http` performs HTTP requests to the Frankfurter API"). A module whose responsibility needs an "and" ("parses responses *and* decides which assets to display") **SHOULD** be split.
- A unit that mixes pure computation with I/O (SQL, HTTP, filesystem, clocks) **MUST** be split so the pure part can be tested without the side effect.
- A unit that talks to two different external systems (for example, SQLite and a network API) **SHOULD** be split into two focused units plus a third that orchestrates them.
- A file that both wires the application together (composition) and implements behavior **SHOULD** be split: wiring lives in the entry point, behavior in modules.
- A useful extraction test: a module **SHOULD** be movable into its own crate without dragging in unrelated code. If extraction would pull in UI, persistence, and networking at once, the boundary is wrong.

### 7.2 Layer responsibilities

The repository follows a layered architecture in which each layer has exactly one job. The classic *controller → service → repository* layering maps onto this Dioxus application as follows:

| Layer | Location | Responsibility |
| --- | --- | --- |
| Controllers (entry points) | `src/api/` (server functions), `src/screens/`, event handlers in `src/components/` | Translate requests and user input into service calls; render results |
| Services (business logic) | `src/application/` (one module per domain) | Encapsulate business rules; orchestrate repositories and clients |
| Repositories | `src/data/repositories/` | The only modules that execute SQL (Section 6.3) |
| API clients | `src/clients/<provider>/` | The only modules that speak to an external API |
| Models (structs) | `src/domain/`, `src/clients/<provider>/types.rs` | Plain data types shared between layers |

Rules for each layer:

- **API controllers** (`src/api/`) are the `#[server]` entry points of the application. They **MUST** be thin: open the database, call exactly one service operation, and translate errors into `ServerFnError`. They **MUST NOT** contain business rules or SQL.
- **Controllers (screens and event handlers)** are the entry points for user actions. They **MUST** be thin: parse input, call exactly one service operation (directly or through an API controller), and render the outcome. They **MUST NOT** contain business rules, SQL, or direct HTTP calls. Section 5.5 applies.
- **Services** encapsulate the business logic of one domain each. A service **MUST** own the rules and workflow for its domain and **SHOULD** expose one public function per use case, orchestrating repositories and clients to fulfill it. Services **MUST NOT** contain SQL text, HTTP details, RSX, or Dioxus signal code.
- **Repositories** are the only place where SQL appears (Section 6.3). One repository **SHOULD** cover one domain entity or aggregate. Repositories translate between rows and domain types and **MUST NOT** make business decisions (validation rules, pricing, formatting) — those belong to services or the domain.
- **API clients** are the entry points for external APIs. There **SHOULD** be one client module per provider, and transport (`http.rs`), wire types (`types.rs`), configuration (`config.rs`), and errors (`error.rs`) **SHOULD** each live in their own file inside the provider folder, as the existing `src/clients/` modules do.
- **Models (structs)** are plain data. A model **MUST NOT** contain SQL, HTTP, another layer's transport details, or Dioxus code. Small behavior that belongs to the value itself, such as validation or formatting, **MAY** live on the type.

Where the current repository does not yet contain a listed folder, contributors **SHOULD** create it when the first real use case appears and **MUST NOT** instead place that logic in a screen or component.

### 7.3 Dependency rule

Dependencies **MUST** point inward: controllers → services → repositories/clients → models. An inner layer **MUST NOT** import an outer layer:

- Models **MUST NOT** import services, repositories, clients, screens, or components.
- Services **MUST NOT** import screens, components, or RSX code.
- Repositories and clients **MUST NOT** import services; a service composes them, never the reverse.
- Circular dependencies between modules are **NOT RECOMMENDED** and usually indicate a missing module or a misplaced type.

When a service needs a side effect that tests must replace (clock, network, randomness), define a narrow trait — a *port* — owned by the service layer, and implement it in the outer *adapter* (ports-and-adapters / hexagonal architecture). A port **SHOULD NOT** be introduced for a single trivial use without a concrete testing or substitution need (Section 8.3). Passing the dependency as a parameter (dependency injection) **SHOULD** be preferred over constructing it inside the function; construction and wiring **SHOULD** happen once, at the entry point (the *composition root*).

### 7.4 Where structs and models live

All shared data types **MUST** have a designated home; contributors **MUST NOT** redefine the same concept ad hoc in whichever file happens to need it:

- **Domain types** (the application's own concepts, such as an address entry or a balance) live in the domain module for their domain (`src/domain/` in the Section 3 layout).
- **Wire types** for an external API live in that provider's `types.rs` (for example `src/clients/mempool/bitcoin/types.rs`) and **MUST** be mapped to domain types at the client boundary — external API schemas **MUST NOT** leak into services or UI.
- **Persistence types**, when they differ from domain types, live next to their repository and **MUST NOT** cross the repository boundary.
- **View types** used only for rendering (for example `BalancePoint` in Section 5.10) live next to the component that consumes them.

When data crosses a layer boundary, it **SHOULD** be mapped explicitly (a DTO/mapping step) rather than reusing another layer's type for convenience, unless the type is a genuine shared domain type.

### 7.5 Additional structural patterns

- **Use-case entry points:** each user-visible operation **SHOULD** correspond to one public service function named from the domain vocabulary (`rename_address`, `refresh_exchange_rates`). An event handler **SHOULD** call exactly one such function.
- **Unit of work:** when one logical change spans multiple repository calls, the coordinating service **MUST** run them in one transaction (Section 6.4); a repository **SHOULD NOT** silently open cross-entity transactions on its own.
- **Anti-patterns** that **MUST NOT** be introduced: god modules that accumulate unrelated functions; `Manager`/`Util`/`Helpers` grab-bag modules; business logic inside event handlers or wiring code; SQL or HTTP outside repositories/clients; one struct serving as the model for every layer.

### 7.6 Module-building checklist

Before adding or extending a module, contributors **SHOULD** be able to answer "yes" to each of the following:

1. The module's responsibility fits in one sentence without "and".
2. The module belongs to exactly one layer in Section 7.2 and imports only from inner layers.
3. Its public surface is minimal — items are `pub` only when another layer needs them; otherwise `pub(crate)` or private.
4. Its types come from the locations in Section 7.4 rather than ad-hoc redefinition.
5. Its pure logic can be unit-tested without Dioxus, a database file, or the network (Sections 5.6 and 9).

## 8. Clean-code standards for Rust

### 8.1 Naming and function design

- Names **MUST** describe intent. Contributors **SHOULD** prefer `load_recent_notes` over `get_data`, and `DatabaseWorker` over `Manager` when that is what the type does.
- Functions **SHOULD** do one coherent job. Split a function when it mixes input validation, SQL, transformation, UI updates, and error presentation.
- Functions **SHOULD** use early returns to handle invalid states and keep the main path easy to read.
- Booleans **SHOULD** read naturally at the call site. Contributors **SHOULD** prefer a named enum when a boolean's meaning is unclear or more than two states are likely.
- Unnecessary abbreviations, unexplained numeric literals, deeply nested conditionals, and excessively long parameter lists **SHOULD** be avoided.
- Typed domain structures and enums **SHOULD** be preferred over loosely related tuples or stringly typed state when the structure has meaning.

### 8.2 Errors, invariants, and panic policy

- Fallible functions **SHOULD** return `Result` and use `Option` for a legitimate absence rather than panic.
- `unwrap()` and `expect()` **MAY** be used in tests or for a truly impossible invariant explained by a specific message. They **MUST NOT** be routine production error handling.
- A shared error enum or a library such as `thiserror` **SHOULD** be used when it makes error sources and variants clearer. Code **SHOULD NOT** convert every failure to an uninformative string too early.
- Input **MUST** be validated at boundaries. Once validation succeeds, internal functions **SHOULD** rely on explicit invariants.
- Errors **MUST NOT** be silently swallowed with `let _ = ...` or empty `Err` branches. Ignoring an error **MAY** be acceptable only when intentional and documented.

### 8.3 Abstractions, comments, and unsafe code

- A trait, generic framework, macro, or dependency **SHOULD NOT** be introduced for one trivial use without a concrete benefit.
- Duplicated business rules **SHOULD** be extracted when duplication could allow behavior to diverge.
- Comments **SHOULD** explain why a non-obvious decision exists, especially around concurrency, migrations, platform limitations, and security; comments **SHOULD NOT** merely restate the code.
- Public modules and non-obvious public APIs **SHOULD** have documentation comments describing contracts, errors, and important edge cases.
- `unsafe` **MUST** be avoided unless necessary. Every unsafe block **MUST** include a nearby `// SAFETY:` comment explaining why its requirements are upheld.
- Clippy warnings **SHOULD** be resolved rather than broadly allowed. A targeted lint allowance **MAY** be used with a comment explaining its rationale.

### 8.4 Dependencies, side effects, and privacy

- A dependency **SHOULD** be added only when it provides meaningful value over the standard library or existing dependencies.
- Filesystem access, database access, clocks, environment variables, and OS APIs **SHOULD** be kept at clear boundaries so domain logic remains testable.
- SQL, serialization details, or platform-specific paths **SHOULD NOT** be embedded in domain types unless the type genuinely represents that detail.
- Secrets, private user data, authentication tokens, and sensitive database contents **MUST NOT** be logged.
- Analytics, telemetry, or network behavior **MUST NOT** be added without documenting its purpose and privacy implications.

## 9. Tests and quality gates

A change **MUST** include tests when it changes business rules, persistence behavior, data validation, migrations, concurrency behavior, or error handling. UI-only changes **SHOULD** include component tests or documented manual verification appropriate to the feature.

Coverage expectations by change type:

- **Domain logic:** unit tests for ordinary cases, edge cases, and invalid inputs.
- **Repository/SQL:** SQLite tests using an isolated in-memory database or temporary file.
- **Migrations:** fresh-database migration, migration from supported prior schema versions, and assertions that existing data remains correct.
- **Concurrency:** tests or a documented verification plan for cancellation, worker shutdown, ordering, and race-prone behavior where relevant.
- **UI behavior:** verification of loading, success, empty, and failure states when applicable.
- **Platform-specific code:** build or execute checks on the affected platform; mock the OS boundary when practical.

Tests **MUST NOT** depend on a developer's real database, home directory, network connection, or manually prepared application state. Tests **SHOULD** be deterministic and clean up temporary files they create.

The baseline pre-PR commands are listed in Section 4. Release builds and target-specific bundle commands **SHOULD** be tested for platform-sensitive changes. If a check cannot be run, the pull request **MUST** identify which check was skipped and why.

## 10. Cross-platform compatibility

- Platform-specific imports and APIs **SHOULD** be kept behind narrow modules or `cfg` gates.
- Code **MUST NOT** assume that the current working directory is writable or points to the application directory.
- UI layout, fonts, keyboard behavior, touch targets, scrolling, and accessibility **SHOULD** be checked on each supported renderer/platform. CSS and WebView behavior can differ.
- Android lifecycle transitions, background restrictions, permission prompts, and low-memory behavior **SHOULD** be considered for long-running work.
- A feature unavailable on a platform **MUST** fail gracefully or be explicitly marked unsupported; it **MUST NOT** silently report success.
- CI **SHOULD** include Linux and macOS builds and Android compilation/package checks. Platform packaging, signing, and notarization requirements **MUST** be documented separately from ordinary development builds.

## 11. Pull requests and review

Every pull request **SHOULD** be small enough to review as one logical change. A pull request **MUST** explain:

- what changed and why;
- relevant user-visible behavior or API/schema impact;
- how it was tested and on which platforms;
- migration or compatibility implications, if any;
- known limitations or follow-up work.

Before requesting review, contributors **MUST**:

- [ ] Format Rust and RSX code.
- [ ] Run compilation, tests, and Clippy for the affected workspace.
- [ ] Add or update tests and documentation where needed.
- [ ] Include a migration for database schema changes.
- [ ] Check that no secrets, local database files, build artifacts, or unrelated formatting changes are included.
- [ ] Describe any validation that could not be performed.

Reviewers **SHOULD** evaluate correctness, readability, error handling, concurrency, SQL safety, migration safety, privacy, respect for the layer boundaries in Section 7, and platform compatibility—not only whether the code compiles.

## 12. References

Contributors **MUST** use documentation matching the versions pinned by the repository.

- [Dioxus 0.7 documentation](https://dioxuslabs.com/learn/0.7/)
- [Dioxus hooks](https://dioxuslabs.com/learn/0.7/essentials/basics/hooks/)
- [Dioxus state and signals](https://dioxuslabs.com/learn/0.7/tutorial/state/)
- [Dioxus resources](https://dioxuslabs.com/learn/0.7/essentials/basics/resources/)
- [Dioxus Router](https://dioxuslabs.com/learn/0.7/essentials/router/)
- [Dioxus platform support](https://dioxuslabs.com/learn/0.7/guides/platforms/)
- [Dioxus desktop/mobile bundling](https://dioxuslabs.com/learn/0.7/tutorial/bundle/)
- [Apache ECharts documentation](https://echarts.apache.org/en/index.html)
- [Apache ECharts option reference](https://echarts.apache.org/en/option.html)
- [`rusqlite` documentation](https://docs.rs/rusqlite/latest/rusqlite/)
- [`rusqlite::Connection`](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html)
- [`rusqlite::Transaction`](https://docs.rs/rusqlite/latest/rusqlite/struct.Transaction.html)
- [SQLite documentation](https://www.sqlite.org/docs.html)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119)
- [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174)

When an example conflicts with official documentation for the pinned dependency version, the official documentation and a focused reproducible test **MUST** take precedence over an outdated snippet.
