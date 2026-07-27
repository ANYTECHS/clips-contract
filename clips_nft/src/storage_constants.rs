//! Centralized storage constants — key prefixes, limits, and defaults.
//!
//! All storage-related magic numbers and namespace identifiers live here so
//! migration, deserialization, and domain modules share a single source of truth.

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

// ─── Defaults ─────────────────────────────────────────────────────────────────

pub const DEFAULT_ROYALTY_BPS: u32 = 500;
pub const DEFAULT_PLATFORM_FEE_BPS: u32 = 0;
pub const DEFAULT_PAUSED: bool = false;
pub const DEFAULT_NEXT_TOKEN_ID: u32 = 0;
pub const DEFAULT_NEXT_BATCH_ID: u64 = 0;
pub const DEFAULT_TOTAL_SUPPLY: u32 = 0;
pub const DEFAULT_TOKEN_COUNTER: u32 = 0;
pub const DEFAULT_UPGRADE_TIMESTAMP: u64 = 0;

// ─── Storage key namespace prefixes ───────────────────────────────────────────

/// Logical prefixes for instance-scoped (contract-global) storage keys.
pub mod instance_prefix {
    pub const ADMIN: &str = "admin";
    pub const CONFIG: &str = "config";
    pub const VERSION: &str = "version";
    pub const PAUSED: &str = "paused";
    pub const SIGNER: &str = "signer";
    pub const SUPPLY: &str = "supply";
    pub const FEE: &str = "fee";
    pub const ROYALTY: &str = "royalty";
}

/// Logical prefixes for persistent (per-token / per-address) storage keys.
pub mod persistent_prefix {
    pub const TOKEN: &str = "token";
    pub const METADATA: &str = "metadata";
    pub const ROYALTY: &str = "royalty";
    pub const CLIP: &str = "clip";
    pub const APPROVAL: &str = "approval";
    pub const OPERATOR: &str = "operator";
    pub const BLACKLIST: &str = "blacklist";
}

// ─── Event key prefixes ───────────────────────────────────────────────────────

pub mod event_keys {
    pub const CONFIG_UPDATE: &str = "config_update";
    pub const MINT: &str = "mint";
    pub const TRANSFER: &str = "transfer";
    pub const BURN: &str = "burn";
    pub const ROYALTY_PAID: &str = "royalty_paid";
    pub const PAUSED: &str = "paused";
    pub const UNPAUSED: &str = "unpaused";
    pub const MIGRATION: &str = "migration";
    pub const CREATOR: &str = "creator";
}
