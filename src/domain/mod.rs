//! Domain types, validation, and pure business rules shared by every layer.
//!
//! Modules here are plain data and pure functions: they MUST NOT contain SQL,
//! HTTP, RSX, or Dioxus signal code.
pub mod address;
pub mod backup;
pub mod calendar;
pub mod catalog;
pub mod civil_time;
pub mod csv;
pub mod date_presets;
pub mod date_selection;
pub mod encryption;
pub mod exchange;
pub mod flow;
pub mod health;
pub mod interface_settings;
pub mod price;
pub mod secret;
pub mod security_settings;
pub mod table;
pub mod theme;
pub mod transaction;
