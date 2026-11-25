//! Mint events under the centralized `events` namespace (issue #914).
//!
//! The canonical payload definitions live in [`crate::types`] and the emitters
//! live in [`crate::mint_event`]. This module re-exports both under
//! `events::mint` so every contract event can be reached through a single,
//! organized module tree without re-implementing (and therefore duplicating)
//! the emission logic.
//!
//! # Topics
//! - `"mint"`     — legacy lightweight mint event (owner, clip, token, URI).
//! - `"nft_mint"` — rich NFT-minted event with all acceptance-criteria fields.
//!
//! # Usage
//! ```rust,ignore
//! events::mint::emit_nft_minted(&env, token_id, clip_id, &creator, &owner, &uri, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{MintEvent, NFTMintedEvent};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::mint_event::{build_nft_minted_event, emit_mint, emit_nft_minted};
