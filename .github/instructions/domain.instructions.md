---
applyTo: "src/domain/**,src/application/**,src/clients/**"
---

# Domain, application, and client rules

Full specification: `CONTRIBUTING.md` §7 and §8.

- One module, one responsibility, statable in one sentence without "and".
- Dependencies point inward: controllers → services → repositories/clients →
  models. An inner layer never imports an outer one.
- Domain types live in `src/domain/`. Wire types live in
  `src/clients/<provider>/types.rs` and are mapped to domain types at the client
  boundary. External API schemas never leak into services or UI.
- Services contain business rules. No SQL, no HTTP details, no RSX, no Dioxus
  signals.
- Clients are the only code that speaks to an external API. One provider per
  folder, with `http.rs`, `types.rs`, `config.rs`, `error.rs` as needed.
- Introduce a trait (port) only when a test must substitute the side effect. Do
  not add a trait for a single trivial call.
- Fallible functions return `Result`; `Option` expresses a legitimate absence.
  No routine `unwrap()`/`expect()`, no `let _ =` on a real error.
- Comments explain why. Do not restate a signature, narrate obvious control flow,
  or cite CONTRIBUTING section numbers.
- Pure logic must be testable without Dioxus, a database file, or the network.
