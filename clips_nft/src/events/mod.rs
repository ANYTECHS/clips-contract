//! Centralized event helpers (Issue: marketplace & offer events).
//!
//! Groups listing-related and offer-related event emitters into a single
//! `events::listing` / `events::offer` namespace so entry-point code in
//! [`crate::ClipsNftContract`] can call `events::listing::emit_*` /
//! `events::offer::emit_*` without importing individual modules.

pub mod listing;
pub mod offer;
