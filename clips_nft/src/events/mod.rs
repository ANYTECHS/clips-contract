//! Centralized event module for the ClipCash smart contract.
//!
//! # Overview
//!
//! Every contract event is defined and emitted from this single module so that
//! event shapes and their emission logic live in exactly one place. Previously,
//! event publishing was scattered across feature modules (e.g.
//! `royalty_frozen_event`, `marketplace::listing_validator`), which led to
//! duplicated topic labels and inconsistent payloads. This module is the single
//! source of truth for:
//!
//! - **Listing events** — creation, updates, cancellation, and sale (`listing`).
//! - **Offer events** — placement, acceptance, and cancellation (`offer`).
//!
//! # Layout
//!
//! ```text
//! events/
//! ├── mod.rs     ← this file: module docs and re-exports
//! ├── listing.rs ← listing + sale event types and `emit_*` helpers
//! └── offer.rs   ← offer event types and `emit_*` helpers
//! ```
//!
//! # Usage
//!
//! Callers should use the `emit_*` helpers rather than publishing raw topics:
//!
//! ```rust,ignore
//! events::listing::emit_listing_cancelled(&env, token_id, &seller, &caller, ts);
//! ```
//!
//! This guarantees a stable topic label, a typed payload, and avoids copy-paste
//! mistakes across the codebase.

pub mod listing;
pub mod offer;
