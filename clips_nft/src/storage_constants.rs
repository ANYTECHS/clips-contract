//! Centralized storage constants — key prefixes, limits, and defaults (issue #1186).
//!
//! All storage-related magic numbers and namespace identifiers live here so
//! migration, deserialization, and domain modules share a single source of truth.
//!
//! # Organization
//! - **Version constants** — contract and migration version numbers.
//! - **Limits** — upper and lower bounds enforced by the contract.
//! - **Defaults** — initial/fallback values for optional settings.
//! - [`instance_prefix`] — string prefixes for instance-scoped storage keys.
//! - [`persistent_prefix`] — string prefixes for persistent per-token/per-address keys.
//! - [`temporary_prefix`] — string prefixes for temporary (short-TTL) storage keys.
//! - [`event_keys`] — topic strings used when publishing contract events.

// ─── Contract / migration version ───────────────────────────────────────────

/// Compile-time contract interface version (bump on breaking ABI changes).
pub const CONTRACT_VERSION: u32 = 1;

/// Migration version before any upgrade has run (legacy deployments).
pub const INITIAL_MIGRATION_VERSION: u32 = 0;

/// Target migration version for the current contract release.
pub const CURRENT_MIGRATION_VERSION: u32 = 1;

// ─── Basis points scale ─────────────────────────────────────────────────────

pub const BASIS_POINTS_SCALE: u32 = 10_000;

// ─── Limits ───────────────────────────────────────────────────────────────────

pub const MAX_ROYALTY_BPS: u32 = 10_000;
pub const MAX_PLATFORM_FEE_BPS: u32 = 1_000;
pub const MAX_BATCH_MINT_SIZE: u32 = 50;
pub const MAX_COLLECTION_SIZE: u32 = 10_000;
pub const MAX_COLLECTION_LIMIT: u32 = 100_000;
pub const MIN_BATCH_MINT_SIZE: u32 = 1;
pub const MAX_BATCH_MINT_SIZE_LIMIT: u32 = 100;
pub const MIN_COLLECTION_SIZE: u32 = 1;
/// Maximum number of individual transfers allowed in a single batch call.
pub const MAX_BATCH_TRANSFER_SIZE: u32 = 50;
/// Minimum number of transfers required to form a valid batch.
pub const MIN_BATCH_TRANSFER_SIZE: u32 = 1;
/// Hard upper bound on `Config::max_batch_transfer_size` (operator-configurable cap).
pub const MAX_BATCH_TRANSFER_SIZE_LIMIT: u32 = 100;

// ─── Additional limits (issue #1186) ──────────────────────────────────────────

/// Maximum number of royalty recipients allowed per token.
pub const MAX_ROYALTY_RECIPIENTS: u32 = 10;
/// Maximum allowed metadata URI length in bytes.
pub const MAX_METADATA_URI_BYTES: u32 = 512;
/// Maximum allowed collection name length in bytes.
pub const MAX_COLLECTION_NAME_BYTES: u32 = 64;
/// Maximum allowed collection symbol length in bytes.
pub const MAX_COLLECTION_SYMBOL_BYTES: u32 = 16;
/// Maximum number of administrators the contract may hold at once.
pub const MAX_ADMINISTRATOR_COUNT: u32 = 20;
/// Minimum platform fee basis points (0 = free).
pub const MIN_PLATFORM_FEE_BPS: u32 = 0;
/// Maximum listing/offer sale price (guards against i128 overflow in royalty math).
pub const MAX_SALE_PRICE: i128 = i128::MAX / 2;
/// Maximum mint cooldown period in seconds (~30 days).
pub const MAX_MINT_COOLDOWN_SECS: u64 = 30 * 24 * 60 * 60;

// ─── Defaults ─────────────────────────────────────────────────────────────────

pub const DEFAULT_ROYALTY_BPS: u32 = 500;
pub const DEFAULT_PLATFORM_FEE_BPS: u32 = 0;
pub const DEFAULT_PAUSED: bool = false;
pub const DEFAULT_NEXT_TOKEN_ID: u32 = 0;
pub const DEFAULT_NEXT_BATCH_ID: u64 = 0;
pub const DEFAULT_TOTAL_SUPPLY: u32 = 0;
pub const DEFAULT_TOKEN_COUNTER: u32 = 0;
pub const DEFAULT_UPGRADE_TIMESTAMP: u64 = 0;
/// Default maximum metadata URI size in bytes (overridable by admin).
pub const DEFAULT_MAX_METADATA_SIZE: u32 = MAX_METADATA_URI_BYTES;
/// Default mint cooldown in seconds (0 = no cooldown).
pub const DEFAULT_MINT_COOLDOWN_SECS: u64 = 0;
/// Default metadata schema version for fresh deployments.
pub const DEFAULT_METADATA_VERSION: u32 = 1;
/// Default listing/offer expiry window in seconds (~7 days).
pub const DEFAULT_LISTING_EXPIRY_SECS: u64 = 7 * 24 * 60 * 60;

// ─── Storage key namespace prefixes (issue #1186) ─────────────────────────────

/// Logical prefixes for instance-scoped (contract-global) storage keys.
///
/// These strings are informational — the on-chain keys are the `DataKey` enum
/// variants.  The prefixes document the conceptual namespace each key belongs to
/// so developers can reason about key collision risk.
pub mod instance_prefix {
    /// Administrator / owner address.
    pub const ADMIN: &str = "admin";
    /// Global contract configuration struct.
    pub const CONFIG: &str = "config";
    /// Contract interface / migration version.
    pub const VERSION: &str = "version";
    /// Pause / unpause state flag.
    pub const PAUSED: &str = "paused";
    /// Backend Ed25519 signer public key.
    pub const SIGNER: &str = "signer";
    /// Total token supply counter.
    pub const SUPPLY: &str = "supply";
    /// Platform fee basis points.
    pub const FEE: &str = "fee";
    /// Default royalty basis points.
    pub const ROYALTY: &str = "royalty";
    /// Auto-increment next token ID counter.
    pub const NEXT_TOKEN_ID: &str = "next_token_id";
    /// Auto-increment next batch ID counter.
    pub const NEXT_BATCH_ID: &str = "next_batch_id";
    /// Platform revenue accumulator.
    pub const PLATFORM_REVENUE: &str = "platform_revenue";
    /// Collection human-readable name.
    pub const NAME: &str = "name";
    /// Collection ticker symbol.
    pub const SYMBOL: &str = "symbol";
    /// Global royalty payments enabled/disabled flag.
    pub const ROYALTY_PAYMENTS_DISABLED: &str = "royalty_payments_disabled";
}

/// Logical prefixes for persistent (per-token / per-address) storage keys.
///
/// Each prefix documents the conceptual namespace of a family of keyed entries
/// in persistent storage (e.g. `TOKEN` covers every `DataKey::Token(token_id)`).
pub mod persistent_prefix {
    /// Core token data (owner + clip ID).
    pub const TOKEN: &str = "token";
    /// Metadata URI for a token.
    pub const METADATA: &str = "metadata";
    /// Royalty configuration for a token.
    pub const ROYALTY: &str = "royalty";
    /// Clip ID ↔ token ID bidirectional mapping.
    pub const CLIP: &str = "clip";
    /// Single-token approval record.
    pub const APPROVAL: &str = "approval";
    /// Operator (all-token) approval record.
    pub const OPERATOR: &str = "operator";
    /// Address blacklist flag.
    pub const BLACKLIST: &str = "blacklist";
    /// Original creator address for a token.
    pub const CREATOR: &str = "creator";
    /// Frozen (soulbound) status of a token.
    pub const FROZEN_TOKEN: &str = "frozen_token";
    /// Burned status of a token.
    pub const BURNED_TOKEN: &str = "burned_token";
    /// Per-token royalty percentage in basis points.
    pub const ROYALTY_PERCENTAGE: &str = "royalty_percentage";
    /// Per-token royalty recipient address.
    pub const ROYALTY_RECIPIENT: &str = "royalty_recipient";
    /// Portfolio of token IDs owned by a wallet.
    pub const OWNER_TOKENS: &str = "owner_tokens";
    /// Portfolio of token IDs created by a creator.
    pub const CREATOR_TOKENS: &str = "creator_tokens";
    /// Active marketplace listing for a token.
    pub const LISTING: &str = "listing";
    /// Open buyer offer for a token.
    pub const OFFER: &str = "offer";
    /// Replay-protection entry for a consumed payment ID.
    pub const USED_PAYMENT: &str = "used_payment";
    /// Replay-protection entry for a consumed signature hash.
    pub const USED_SIGNATURE: &str = "used_signature";
    /// Per-address nonce counter for signature replay prevention.
    pub const NONCE: &str = "nonce";
    /// Thumbnail image URI for a token.
    pub const THUMBNAIL_URI: &str = "thumbnail_uri";
    /// Preview video URI for a token.
    pub const PREVIEW_VIDEO_URI: &str = "preview_video_uri";
    /// Metadata update timestamps (creation + last-updated epoch seconds).
    pub const METADATA_TIMESTAMPS: &str = "metadata_timestamps";
    /// Registered metadata record existence marker (keyed by URI).
    pub const METADATA_RECORD: &str = "metadata_record";
    /// Cumulative royalty earnings generated by a token.
    pub const ROYALTY_EARNINGS: &str = "royalty_earnings";
    /// Cumulative earnings accrued by a creator/recipient address.
    pub const CREATOR_EARNINGS: &str = "creator_earnings";
}

/// Logical prefixes for temporary (short-TTL) storage keys.
///
/// Temporary storage entries expire automatically after a configurable number
/// of ledger closings.  Use these for rate-limit windows and other ephemeral state.
pub mod temporary_prefix {
    /// Mint cooldown window — tracks when an address last minted.
    pub const MINT_COOLDOWN: &str = "mint_cooldown";
    /// Short-lived signature nonce validity window.
    pub const SIGNATURE_WINDOW: &str = "sig_window";
}

// ─── Event key prefixes ───────────────────────────────────────────────────────

/// Topic strings used when publishing contract events.
///
/// All `env.events().publish(...)` calls should reference these constants
/// instead of bare string literals.
pub mod event_keys {
    pub const CONFIG_UPDATE: &str = "config_update";
    pub const MINT: &str = "mint";
    pub const BATCH_MINT: &str = "batch_mint";
    pub const TRANSFER: &str = "transfer";
    pub const BURN: &str = "burn";
    pub const ROYALTY_PAID: &str = "royalty_paid";
    pub const ROYALTY_UPDATED: &str = "royalty_updated";
    pub const ROYALTY_FROZEN: &str = "royalty_frozen";
    pub const PAUSED: &str = "paused";
    pub const UNPAUSED: &str = "unpaused";
    pub const MIGRATION: &str = "migration";
    pub const CREATOR: &str = "creator";
    pub const METADATA_CHANGED: &str = "metadata_changed";
    pub const LISTING_CREATED: &str = "listing_created";
    pub const LISTING_CANCELLED: &str = "listing_cancelled";
    pub const LISTING_SOLD: &str = "listing_sold";
    pub const OFFER_CREATED: &str = "offer_created";
    pub const OFFER_ACCEPTED: &str = "offer_accepted";
    pub const TOKEN_FROZEN: &str = "token_frozen";
    pub const TOKEN_UNFROZEN: &str = "token_unfrozen";
    pub const APPROVAL_GRANTED: &str = "approval_granted";
    pub const APPROVAL_REVOKED: &str = "approval_revoked";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_internally_consistent() {
        assert!(MIN_BATCH_MINT_SIZE <= MAX_BATCH_MINT_SIZE);
        assert!(MAX_BATCH_MINT_SIZE <= MAX_BATCH_MINT_SIZE_LIMIT);
        assert!(MIN_BATCH_TRANSFER_SIZE <= MAX_BATCH_TRANSFER_SIZE);
        assert!(MAX_BATCH_TRANSFER_SIZE <= MAX_BATCH_TRANSFER_SIZE_LIMIT);
        assert!(MIN_COLLECTION_SIZE <= MAX_COLLECTION_SIZE);
        assert!(MAX_COLLECTION_SIZE <= MAX_COLLECTION_LIMIT);
        assert!(MIN_PLATFORM_FEE_BPS <= MAX_PLATFORM_FEE_BPS);
        assert!(MAX_PLATFORM_FEE_BPS < MAX_ROYALTY_BPS);
        assert!(MAX_ROYALTY_BPS == BASIS_POINTS_SCALE);
    }

    #[test]
    fn defaults_are_within_limits() {
        assert!(DEFAULT_ROYALTY_BPS <= MAX_ROYALTY_BPS);
        assert!(DEFAULT_PLATFORM_FEE_BPS <= MAX_PLATFORM_FEE_BPS);
        assert!(DEFAULT_NEXT_TOKEN_ID == 0);
        assert!(DEFAULT_NEXT_BATCH_ID == 0);
        assert!(DEFAULT_TOTAL_SUPPLY == 0);
        assert!(DEFAULT_TOKEN_COUNTER == 0);
        assert!(DEFAULT_MINT_COOLDOWN_SECS <= MAX_MINT_COOLDOWN_SECS);
        assert!(DEFAULT_MAX_METADATA_SIZE <= MAX_METADATA_URI_BYTES);
    }

    #[test]
    fn version_constants_are_ordered() {
        assert!(INITIAL_MIGRATION_VERSION < CURRENT_MIGRATION_VERSION);
    }

    #[test]
    fn event_key_strings_are_non_empty() {
        let keys = [
            event_keys::CONFIG_UPDATE,
            event_keys::MINT,
            event_keys::BATCH_MINT,
            event_keys::TRANSFER,
            event_keys::BURN,
            event_keys::ROYALTY_PAID,
            event_keys::PAUSED,
            event_keys::UNPAUSED,
        ];
        for k in keys {
            assert!(!k.is_empty());
        }
    }
}
