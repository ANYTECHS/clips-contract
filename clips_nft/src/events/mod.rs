//! Centralized event module for the ClipCash smart contract.
//!
//! # Overview
//!
//! Every contract event is reachable from this single module so that event
//! shapes and their emission logic have exactly one discoverable home.
//! Previously, event publishing was scattered across feature modules (e.g.
//! `royalty_frozen_event`, `mint_event`, `marketplace::listing_validator`),
//! which led to duplicated topic labels and inconsistent payloads. This module
//! is the single source of truth for:
//!
//! - **Listing events** — creation, updates, cancellation, and sale (`listing`).
//! - **Offer events** — placement, acceptance, and cancellation (`offer`).
//! - **Transfer events** — NFT ownership changes (`transfer`, issue #958).
//! - **Metadata events** — metadata updates (`metadata_updated`, issue #961).
//! - **Mint / burn / freeze events** — NFT lifecycle (`mint`, `burn`, `freeze`).
//! - **Royalty events** — assigned, updated, paid, frozen (`royalty`).
//! - **Creator events** — creator assignment (`creator`, issue #962).
//! - **Approval events** — approval granted / revoked (`approval`).
//! - **Administrative events** — configuration and pause/unpause
//!   (`config`, `pause`, `cfg_pause`).
//! - **Batch events** — batch mint completion (`batch_mint`).
//!
//! # Layout
//!
//! ```text
//! events/
//! ├── mod.rs              ← this file: module docs, declarations, re-exports
//! ├── amount.rs           ← shared `AmountInfo` payload helper
//! ├── timestamp.rs        ← shared `LedgerTimestamp` helper
//! ├── listing.rs          ← listing created / updated / cancelled + NFT sold
//! ├── offer.rs            ← offer made / accepted / cancelled
//! ├── transfer.rs         ← NFT transferred (`nft_xfer`, issue #958)
//! ├── metadata_updated.rs ← metadata updated (`meta_upd`, issue #961)
//! ├── mint.rs             ← mint (`mint`, `nft_mint`, issue #914)
//! ├── burn.rs             ← burn (`nft_burn`, issue #916)
//! ├── freeze.rs           ← frozen / unfrozen (`nft_frz`, `nft_unfrz`)
//! ├── royalty.rs          ← royalty assigned / updated / paid / frozen
//! ├── creator.rs          ← creator assigned (`creator`, issue #962)
//! ├── approval.rs         ← approval granted / revoked
//! ├── config.rs           ← configuration updated (`cfg_updt`, issue #932)
//! ├── pause.rs            ← contract paused / unpaused (`cfg_pause`, issue #1372)
//! └── batch_mint.rs       ← batch mint completed (`btch_done`, issue #697)
//! ```
//!
//! # Usage
//!
//! Callers should use the exported `emit_*` helpers rather than publishing raw
//! topics:
//!
//! ```rust,ignore
//! events::listing::emit_listing_cancelled(&env, listing_id, token_id, &seller, ts);
//! events::mint::emit_nft_minted(&env, token_id, clip_id, &creator, &owner, &uri, ts);
//! ```
//!
//! This guarantees a stable topic label, a typed payload, and avoids copy-paste
//! mistakes across the codebase.
//!
//! # Avoid duplicated event logic
//!
//! The NFT lifecycle, royalty, creator, approval, configuration, pause, and
//! batch submodules below do **not** re-implement their emitters. They
//! re-export the canonical payload definitions (from [`crate::types`]) and the
//! canonical `emit_*` helpers, so each topic and payload is still defined in
//! exactly one place. The original standalone `*_event.rs` modules are kept for
//! backward compatibility, so nothing that imports them breaks.
//!
//! # Standalone module coverage
//!
//! | Standalone module | Centralized home |
//! |-------------------|------------------|
//! | `mint_event` | [`mint`] |
//! | `burn_event` | [`burn`] |
//! | `nft_frozen_event`, `nft_unfrozen_event` | [`freeze`] |
//! | `royalty_paid_event`, `royalty_assigned_event`, `royalty_updated_event`, `royalty_frozen_event` | [`royalty`] |
//! | `creator_event` | [`creator`] |
//! | `approval_granted_event`, `approval_revoked_event` | [`approval`] |
//! | `config_updated_event` | [`config`] |
//! | `pause_event` | [`pause`] |
//! | `batch_mint_event` | [`batch_mint`] |
//! | `transfer_event` | [`transfer`] |
//! | `metadata_updated_event` | [`metadata_updated`] |
//! | `nft_listed_event`, `nft_sold_event`, `listing_cancelled_event` | [`listing`] |
//! | `offer_created_event`, `offer_accepted_event` | [`offer`] |

// ─── Shared payload helpers ──────────────────────────────────────────────────
pub mod amount;
pub mod timestamp;

// ─── Marketplace events ──────────────────────────────────────────────────────
pub mod listing;
pub mod offer;

// ─── NFT lifecycle events ────────────────────────────────────────────────────
pub mod burn;
pub mod freeze;
pub mod metadata_updated;
pub mod mint;
pub mod transfer;

// ─── Royalty / permission events ─────────────────────────────────────────────
pub mod approval;
pub mod creator;
pub mod royalty;

// ─── Administrative / batch events ───────────────────────────────────────────
pub mod batch_mint;
pub mod config;
pub mod pause;

#[cfg(test)]
mod tests;
