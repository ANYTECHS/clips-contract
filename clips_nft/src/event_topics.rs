//! Centralized event topic constants for the Clips NFT contract.
//!
//! Resolves issue #908: provides a single source of truth for all event
//! topic symbols used across the contract. Every `*_event.rs` module
//! should import topics from here instead of defining ad-hoc
//! `symbol_short!` calls.
//!
//! # Naming convention (issue #907)
//! - Topic symbols are 9 characters or fewer (Soroban `symbol_short!` limit).
//! - Format: `<entity>_<action>` using snake_case abbreviations.
//! - Entity prefix groups related events for indexer filtering.
//!
//! # NFT Lifecycle Events
//! - `nft_mint`  — NFT successfully minted (tokens newly created)
//! - `nft_xfer`  — NFT transferred between owners
//! - `nft_burn`  — NFT permanently destroyed
//! - `nft_froz`  — NFT frozen (transfer-disabled)
//! - `nft_unfrz` — NFT unfrozen (transfer-enabled)
//!
//! # Marketplace Events
//! - `nft_list`  — NFT listed for sale
//! - `nft_sold`  — NFT sold via marketplace
//! - `nft_unlst` — NFT listing cancelled
//!
//! # Royalty Events
//! - `rlyt_paid`  — Royalty payment completed
//! - `rlyt_asgn`  — Royalty assigned to token
//! - `rlyt_upd`   — Royalty configuration updated
//! - `rlyt_frzn`  — Royalty configuration frozen
//!
//! # Admin Events
//! - `ctr_pause`  — Contract paused
//! - `ctr_unpse`  — Contract unpaused
//! - `cfg_upd`    — Config value updated
//! - `aprv_grnt`  — Operator approval granted
//! - `aprv_rvkd`  — Approval revoked
//! - `creator`    — Creator assigned to token
//!
//! # Offer Events
//! - `ofr_crtd` — Offer created
//! - `ofr_acpt` — Offer accepted
//! - `ofr_made` — Offer placed
//! - `ofr_can`  — Offer cancelled

use soroban_sdk::Symbol;

// ── NFT lifecycle ───────────────────────────────────────────────────────────

/// NFT minted — emitted after a new token is created.
pub const TOPIC_MINT: Symbol = soroban_sdk::symbol_short!("nft_mint");

/// NFT transferred — emitted on every ownership change.
pub const TOPIC_TRANSFER: Symbol = soroban_sdk::symbol_short!("nft_xfer");

/// NFT burned — emitted when a token is permanently destroyed.
pub const TOPIC_BURN: Symbol = soroban_sdk::symbol_short!("nft_burn");

/// NFT frozen — emitted when a token is frozen (transfer-disabled).
pub const TOPIC_FREEZE: Symbol = soroban_sdk::symbol_short!("nft_froz");

/// NFT unfrozen — emitted when a frozen token is unfrozen.
pub const TOPIC_UNFREEZE: Symbol = soroban_sdk::symbol_short!("nft_unfrz");

// ── Marketplace ─────────────────────────────────────────────────────────────

/// NFT listed for sale.
pub const TOPIC_LISTING: Symbol = soroban_sdk::symbol_short!("nft_list");

/// NFT listing cancelled.
pub const TOPIC_LISTING_CANCELLED: Symbol = soroban_sdk::symbol_short!("nft_unlst");

/// NFT sold / purchased.
pub const TOPIC_SALE: Symbol = soroban_sdk::symbol_short!("nft_sold");

// ── Royalty ─────────────────────────────────────────────────────────────────

/// Royalty payment completed.
pub const TOPIC_ROYALTY_PAID: Symbol = soroban_sdk::symbol_short!("rlyt_paid");

/// Royalty configuration assigned.
pub const TOPIC_ROYALTY_ASSIGNED: Symbol = soroban_sdk::symbol_short!("rlyt_asgn");

/// Royalty configuration updated.
pub const TOPIC_ROYALTY_UPDATED: Symbol = soroban_sdk::symbol_short!("rlyt_upd");

/// Royalty frozen (no further payments).
pub const TOPIC_ROYALTY_FROZEN: Symbol = soroban_sdk::symbol_short!("rlyt_frzn");

// ── Approval / Authorization ───────────────────────────────────────────────

/// Operator approval granted.
pub const TOPIC_APPROVAL: Symbol = soroban_sdk::symbol_short!("appr_grnt");

/// Operator approval revoked.
pub const TOPIC_APPROVAL_REVOKED: Symbol = soroban_sdk::symbol_short!("aprv_rvkd");

// ── Configuration / Admin ───────────────────────────────────────────────────

/// Contract configuration updated.
pub const TOPIC_CONFIG_UPDATED: Symbol = soroban_sdk::symbol_short!("cfg_upd");

/// Contract paused or unpaused.
pub const TOPIC_PAUSE: Symbol = soroban_sdk::symbol_short!("cfg_pause");

/// Creator registered on-chain.
pub const TOPIC_CREATOR: Symbol = soroban_sdk::symbol_short!("creator");

// ── Offers ─────────────────────────────────────────────────────────────────

/// Marketplace offer created.
pub const TOPIC_OFFER_CREATED: Symbol = soroban_sdk::symbol_short!("ofr_crtd");

/// Marketplace offer accepted.
pub const TOPIC_OFFER_ACCEPTED: Symbol = soroban_sdk::symbol_short!("ofr_acpt");

// ── Batch operations ────────────────────────────────────────────────────────

/// Batch mint operation completed.
pub const TOPIC_BATCH_MINT: Symbol = soroban_sdk::symbol_short!("bat_mint");

/// Batch mint assigned (pre-mint state).
pub const TOPIC_BATCH_ASSIGN: Symbol = soroban_sdk::symbol_short!("bat_asgn");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_topics_are_within_symbol_limit() {
        // symbol_short! panics if > 9 chars; just verify they exist
        let _ = TOPIC_MINT;
        let _ = TOPIC_TRANSFER;
        let _ = TOPIC_BURN;
        let _ = TOPIC_FREEZE;
        let _ = TOPIC_UNFREEZE;
        let _ = TOPIC_LISTING;
        let _ = TOPIC_LISTING_CANCELLED;
        let _ = TOPIC_SALE;
        let _ = TOPIC_ROYALTY_PAID;
        let _ = TOPIC_ROYALTY_ASSIGNED;
        let _ = TOPIC_ROYALTY_UPDATED;
        let _ = TOPIC_ROYALTY_FROZEN;
        let _ = TOPIC_APPROVAL;
        let _ = TOPIC_APPROVAL_REVOKED;
        let _ = TOPIC_CONFIG_UPDATED;
        let _ = TOPIC_PAUSE;
        let _ = TOPIC_CREATOR;
        let _ = TOPIC_OFFER_CREATED;
        let _ = TOPIC_OFFER_ACCEPTED;
        let _ = TOPIC_BATCH_MINT;
        let _ = TOPIC_BATCH_ASSIGN;
    }

    #[test]
    fn topics_are_distinct() {
        let topics = [
            TOPIC_MINT,
            TOPIC_TRANSFER,
            TOPIC_BURN,
            TOPIC_FREEZE,
            TOPIC_UNFREEZE,
            TOPIC_LISTING,
            TOPIC_LISTING_CANCELLED,
            TOPIC_SALE,
            TOPIC_ROYALTY_PAID,
            TOPIC_ROYALTY_ASSIGNED,
            TOPIC_ROYALTY_UPDATED,
            TOPIC_ROYALTY_FROZEN,
            TOPIC_APPROVAL,
            TOPIC_APPROVAL_REVOKED,
            TOPIC_CONFIG_UPDATED,
            TOPIC_PAUSE,
            TOPIC_CREATOR,
            TOPIC_OFFER_CREATED,
            TOPIC_OFFER_ACCEPTED,
            TOPIC_BATCH_MINT,
            TOPIC_BATCH_ASSIGN,
        ];
        for i in 0..topics.len() {
            for j in (i + 1)..topics.len() {
                assert_ne!(topics[i], topics[j], "duplicate topic: {:?}", topics[i]);
            }
        }
    }
}
