//! Domain types, validation, and pure business rules shared by every layer.
//!
//! Modules here are plain data and pure functions: they MUST NOT contain SQL,
//! HTTP, RSX, or Dioxus signal code (Section 7.2 of CONTRIBUTING.md).

pub mod address;
pub mod catalog;
pub mod civil_time;
pub mod exchange;
pub mod price;
pub mod table;
pub mod theme;
pub mod transaction;
