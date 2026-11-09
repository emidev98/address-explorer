---
applyTo: "src/components/**,src/screens/**"
---

# UI layer rules

Full specification: `CONTRIBUTING.md` §5.2–§5.14 and §3.1.

- `rsx!` for markup, `#[component]` for components, `UpperCamelCase` names.
- Props are owned values (`String`, `Vec<T>`). They satisfy the pinned Dioxus
  component bounds.
- Components render from props and reactive state. No blocking database,
  filesystem, network, or CPU work during render.
- `use_signal` for local state; `use_memo` or a direct expression for derived
  state. No second mutable source of truth.
- A helper that calls hooks MUST be named with the `use_` prefix.
- Use the shared primitives: `Icon`, `PageHeader`, `Modal`, `Checkbox`,
  `DatePicker`, `TablePagination`, `SortableHeader`, `TableSelection*`,
  `ConfirmDialog`, `MultiSelect`, `CatalogSelect`, `ProviderBadge`, `CsvDropzone`,
  `Toast`, and the formatters in `src/components/format.rs`. Do not hand-roll a
  dialog scrim, checkbox, calendar, pagination bar, or sort header.
- Fiat values: read `use_fiat_currency()` and format with `format_fiat`. Never
  call `format_usd` from a view and never hardcode a fiat code in a label.
- Charts: one component per chart under `src/components/charts/`, receiving
  domain data only. Never build an ECharts option in a screen. Colors come from
  `src/components/charts/theme.rs` tokens, never hardcoded values.
- Tables: `TableQuery`-flattening endpoint, `TablePage<T>` result, allowlisted
  sort enum parsed at the service boundary, shared pagination and sort controls.
- Verify palettes and appearance in both light and dark before requesting review.
