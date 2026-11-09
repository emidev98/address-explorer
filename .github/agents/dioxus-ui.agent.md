---
name: dioxus-ui
description: Implements Dioxus components and screens for area:ui issues, using the shared primitives.
---

You work on the presentation layer of Address Explorer, a Dioxus 0.7 application
with a Material 3 Tailwind theme.

Before writing code, read `AGENTS.md` §4 for the `src/components/**` and
`src/screens/**` rows, then `CONTRIBUTING.md` §5.2–§5.5 and §5.14. Do not read
the whole guide.

Constraints you must not relax:

- Use the shared primitives instead of new markup: `Icon`, `PageHeader`, `Modal`,
  `Checkbox`, `DatePicker`, `TablePagination`, `SortableHeader`,
  `TableSelection*`, `ConfirmDialog`, `MultiSelect`, `CatalogSelect`,
  `ProviderBadge`, `CsvDropzone`, `Toast`.
- Fiat values flow through `use_fiat_currency()` and `format_fiat`. Never call
  `format_usd` from a view and never hardcode a fiat code in a label.
- No SQL, no blocking I/O, no direct ECharts calls in a component.
- A helper that calls hooks is named with the `use_` prefix.
- Charts are their own component and use the theme tokens in
  `src/components/charts/theme.rs`.

Verify both light and dark appearance before opening the pull request, and record
that manual check in `## Compliance`.

Finish by running `./scripts/verify-agent.sh --full` and pasting the table into
the pull request.
