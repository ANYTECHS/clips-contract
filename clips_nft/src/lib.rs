#![no_std]

pub mod metadata;
pub mod safe_math;
pub mod virality_score;
pub mod social_platform;
pub mod video_reference;
pub mod metadata_uri_builder;

mod blacklist;
mod clip_id_storage;
mod config;
mod config_guard;
mod config_validator;
mod creator_storage;
mod default_royalty;
mod minted_clip_index;
mod payment_currency;
mod platform_fee;
mod platform_recipient;
mod royalty_storage;
mod token_approval;
mod token_metadata_storage;
mod token_storage;
mod types;
mod wallet_token_index;
mod collection_info;
mod metadata_config;

pub use config::{
    get_config, set_config, Config, CONTRACT_VERSION, MAX_BATCH_MINT_SIZE, MAX_COLLECTION_SIZE,
};
pub use config_guard::require_config_admin;
pub use config_validator::MAX_COLLECTION_LIMIT;
pub use default_royalty::{
    get_default_royalty_bps, set_default_royalty_bps, DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS,
};
pub use payment_currency::{add_currency, get_currencies, is_supported, remove_currency};
pub use platform_fee::{get_platform_fee, set_platform_fee, MAX_PLATFORM_FEE_BPS};
pub use platform_recipient::{
    get_platform_recipient, save_platform_recipient, update_platform_recipient,
};
pub use collection_info::{
    get_name, get_symbol, set_name, set_symbol, DEFAULT_NAME, DEFAULT_SYMBOL,
};
pub use metadata_config::{
    get_max_metadata_size, set_max_metadata_size, validate_metadata_size, DEFAULT_MAX_METADATA_SIZE,
};

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, xdr::ToXdr, Address, Bytes,
    BytesN, Env, String, Vec,
};

pub const VERSION: u32 = 1;
pub const DEFAULT_MINT_COOLDOWN_SECONDS: u64 = 0;
pub const DEFAULT_CIRCUIT_BREAKER_ENABLED: bool = false;
pub const DEFAULT_CIRCUIT_BREAKER_THRESHOLD: u64 = 100;
pub const DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS: u64 = 60;

const GAS_BASE_MINT: u64 = 50_000;
const GAS_BASE_TRANSFER: u64 = 30_000;
const MAX_BATCH_MINT: u32 = 25;
const PERSISTENT_BUMP_THRESHOLD: u32 = 172_800;
const PERSISTENT_BUMP_AMOUNT: u32 = 535_680;

// =============================================================================
// Errors
// =============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Error {
    Unauthorized = 1,
    InvalidTokenId = 2,
    ClipAlreadyMinted = 3,
    RoyaltyTooHigh = 4,
    InvalidRecipient = 5,
    InvalidSalePrice = 6,
    ContractPaused = 7,
    InvalidSignature = 8,
    SignerNotSet = 9,
    InvalidRoyaltySplit = 10,
    SoulboundTransferBlocked = 11,
    RoyaltyOverflow = 12,
    ClipBlacklisted = 13,
    NotAuthorizedToApprove = 14,
    /// Wallet address is blacklisted
    WalletBlacklisted = 15,
    WithdrawalStillLocked = 15,
    NoWithdrawalRequest = 16,
    BatchTooLarge = 17,
    TokenFrozen = 18,
    InsufficientBalance = 19,
    MetadataRefreshTooSoon = 20,
    UnsupportedProtocol = 21,
    MalformedUrl = 22,
    MintCooldownActive = 23,
    Reentrancy = 24,
    MintingPaused = 25,
    CircuitBreakerTripped = 26,
    MetadataLocked = 27,
    MaxSupplyReached = 28,
    InvalidMaxSupply = 29,
    InvalidRecoverAmount = 30,
}

// =============================================================================
// Types
// =============================================================================

pub type TokenId = u32;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub trait_type: String,
    pub value: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipient {
    pub recipient: Address,
    pub basis_points: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Royalty {
    pub recipients: Vec<RoyaltyRecipient>,
    pub asset_address: Option<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenData {
    pub owner: Address,
    pub clip_id: u32,
    pub is_soulbound: bool,
    pub metadata_uri: String,
    pub image: Option<String>,
    pub animation_url: Option<String>,
    pub description: Option<String>,
    pub external_url: Option<String>,
    pub attributes: Vec<Attribute>,
    pub royalty: Royalty,
    pub is_locked: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyInfo {
    pub receiver: Address,
    pub royalty_amount: i128,
    pub asset_address: Option<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub symbol: String,
    pub version: u32,
    pub owner: Address,
    pub platform_fee: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequest {
    pub amount: i128,
    pub unlock_time: u64,
}

// =============================================================================
// Storage keys
// =============================================================================

#[contracttype]
pub enum DataKey {
    Admin,
    PendingOwner,
    NextTokenId,
    Paused,
    PauseUnlockTime,
    MintingPaused,
    Name,
    Symbol,
    Signer,
    BackendAddress,
    PlatformRecipient,
    /// Task 2: platform fee in basis points
    PlatformFeeBps,
    /// Task 2: default royalty in basis points
    DefaultRoyaltyBps,
    DefaultRoyaltyAsset,
    MintCooldownSeconds,
    ReentrancyLock,
    TotalSupply,
    MaxSupply,
    ContractVersion,
    CircuitBreakerEnabled,
    CircuitBreakerThreshold,
    CircuitBreakerWindowSeconds,
    CircuitBreakerWindowStart,
    CircuitBreakerWindowCount,
    WithdrawXlmRequest,
    LastWithdrawalTime,
    TotalGasMint,
    CountMint,
    TotalGasTransfer,
    CountTransfer,
    /// Collection name (instance storage)
    Name,
    /// Collection symbol (instance storage)
    Symbol,
    /// Blacklisted clip IDs (persistent storage)
    BlacklistedClip(u32),
    /// Blacklisted wallet addresses (persistent storage)
    BlacklistedWallet(Address),
    /// Per-token operator approval (persistent storage)
    Approved(TokenId),
    /// Operator approvals across all owner tokens (persistent storage).
    /// Key is SHA-256(owner_xdr || operator_xdr) — compact 32-byte form
    /// instead of storing two full addresses, halving the ledger footprint.
    ApprovalForAll(BytesN<32>),
    /// Total platform royalty revenue collected (instance storage)
    TotalPlatformFees,
    Token(TokenId),
    ClipIdMinted(u32),
    MintedClip(u32),
    CustomTokenUri(TokenId),
    Approved(TokenId),
    MetadataUpdateCount(TokenId),
    ApprovalForAll(Address, Address),
    BlacklistedClip(u32),
    Balance(Address),
    Frozen(TokenId),
    MetadataRefreshTime(TokenId),
    RoyaltyBalance(TokenId),
    LastMintTimestamp(Address),
    /// Task 3: global enumeration index
    TokenIndex(u32),
    /// Task 3: per-owner enumeration index
    OwnerTokenIndex(Address, u32),
    /// Task 1: per-wallet nonce for mint_with_signature replay protection
    LastMintNonce(Address),
    /// Task 1: used signature hashes for replay protection
    UsedSignature(BytesN<32>),
    /// Issue #299: optional human-readable reason provided when pausing
    PauseReason,
    /// Issue #471: configurable fee charged per NFT mint (in stroops)
    MintFee,
}

// =============================================================================
// Events
// =============================================================================

// Emitted on mint completion and useful for frontend tokens/indexing.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent { pub to: Address, pub clip_id: u32, pub token_id: TokenId }

// Emitted when a token is destroyed.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnEvent { pub owner: Address, pub token_id: TokenId, pub clip_id: u32 }

// Emitted on token ownership transfer.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEvent { pub token_id: TokenId, pub from: Address, pub to: Address }

/// Event emitted when a wallet address is blacklisted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletBlacklistEvent {
    pub wallet: Address,
}

/// Event emitted when a wallet address is removed from the blacklist.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletUnblacklistEvent {
    pub wallet: Address,
}

/// Event emitted when token approval is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalEvent {
    pub owner: Address,
    pub operator: Address,
    pub token_id: TokenId,
}
// Emitted when a single-token approval is granted.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalEvent { pub owner: Address, pub operator: Address, pub token_id: TokenId }

// Emitted when operator approval is toggled for all tokens of an owner.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalForAllEvent { pub owner: Address, pub operator: Address, pub approved: bool }

// Emitted when royalty is paid during a transfer or sale.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPaidEvent { pub token_id: TokenId, pub from: Address, pub to: Address, pub amount: i128 }

// Emitted when the primary royalty recipient changes.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipientUpdatedEvent { pub token_id: TokenId, pub old_recipient: Address, pub new_recipient: Address }

// Emitted when royalty parameters are updated for a token.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyUpdatedEvent { pub token_id: TokenId }

// Emitted when royalties are claimed from contract-held balances.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyClaimedEvent { pub token_id: TokenId, pub recipient: Address, pub amount: i128, pub asset: Address }

// Emitted when a token URI is updated for a custom owner override.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenUriChangedEvent { pub token_id: TokenId, pub owner: Address, pub new_uri: String }

// Emitted when metadata fields are refreshed by admin or backend.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdatedEvent { pub token_id: TokenId }

// Emitted when metadata is permanently locked.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataLockedEvent { pub token_id: TokenId, pub owner: Address }

// Emitted after a batch mint completes.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchMintEvent { pub to: Address, pub count: u32, pub first_token_id: TokenId }

// Emitted when a clip is blacklisted.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlacklistEvent { pub clip_id: u32 }

// Emitted when a token freezes transfers.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenFrozenEvent { pub token_id: TokenId }

// Emitted when a token freeze is lifted.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenUnfrozenEvent { pub token_id: TokenId }

// Emitted when the backend signer public key changes.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerUpdatedEvent { pub new_pubkey: BytesN<32> }

// Emitted when pause is scheduled and becomes active.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseScheduledEvent { pub active_at: u64 }

// Emitted when pause is scheduled with an optional admin-provided reason.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseWithReasonEvent { pub active_at: u64, pub reason: Option<String> }

// Emitted when the contract is unpaused.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnpausedEvent { pub _unused: () }

// Emitted when minting is paused.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseMintingEvent { pub _unused: () }

// Emitted when minting is resumed.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnpauseMintingEvent { pub _unused: () }

// Emitted when the backend address is updated.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendAddressUpdatedEvent { pub new_backend_address: Address }

// Emitted when the platform recipient changes.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformRecipientUpdatedEvent { pub new_recipient: Address }

// Emitted when the default royalty asset is updated.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultRoyaltyAssetUpdatedEvent { pub asset_address: Option<Address> }

// Emitted when the mint cooldown value changes.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintCooldownUpdatedEvent { pub seconds: u64 }

// Emitted when core config values change.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdatedEvent { pub key: String, pub new_value: i128 }

// Emitted when circuit breaker counters are reset by admin.
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerResetEvent { pub _unused: () }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangedEvent { pub old_admin: Address, pub new_admin: Address }

// Emitted when contract ownership is fully transferred (two-step, #320).
#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipTransferredEvent { pub previous_owner: Address, pub new_owner: Address }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundedEvent { pub token_id: TokenId, pub recipient: Address, pub amount: i128 }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerTriggeredEvent { pub mint_count: u64, pub threshold: u64, pub window_seconds: u64 }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoulboundRecoveredEvent { pub token_id: TokenId, pub old_owner: Address, pub new_owner: Address }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigratedEvent { pub from_version: u32, pub to_version: u32 }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeEvent { pub new_wasm_hash: BytesN<32> }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequestedEvent { pub amount: i128, pub unlock_time: u64 }

#[contracttype] #[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawExecutedEvent { pub amount: i128, pub recipient: Address }

// =============================================================================
// Contract
// =============================================================================

use types::{DataKey, Error, MintEvent, Royalty, RoyaltyInfo, TokenData, TokenId};

#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    // -------------------------------------------------------------------------
    // Init
    // -------------------------------------------------------------------------

//! ClipCashNFT — Soroban smart contract entry point.
//!
//! This module is the single gateway for all public contract methods.
//! It re-exports types and registers the contract implementation
//! via the `#[contract]` / `#[contractimpl]` macros.

#![no_std]

mod blacklist;
mod clip_id_storage;
mod collection_supply;
mod config;
mod config_guard;
mod config_validator;
mod creator_storage;
mod default_royalty;
pub mod metadata;
mod minted_clip_index;
mod operator_approval;
mod pause_state;
mod payment_currency;
mod platform_fee;
mod platform_revenue;
mod royalty_history;
mod royalty_recipient;
mod royalty_storage;
mod safe_math;
pub mod storage;
mod token_approval;
mod token_metadata_storage;
mod token_storage;
mod token_uri_storage;
mod types;
mod virality_score;
mod wallet_token_index;

mod event_counter_storage;

#[cfg(test)]
mod tests;

pub use types::{DataKey, Error};

// Re-export metadata module components
pub use metadata::{
    Attribute, TokenMetadata,
    validate_url, validate_metadata_uri, validate_image_url, validate_animation_url,
    validate_external_url, validate_description, validate_attributes, SUPPORTED_PROTOCOLS,
};

use soroban_sdk::{contract, contractimpl, Address, Env};
use types::Config;

#[contract]
pub struct ClipCashNFT;

#[contractimpl]
impl ClipCashNFT {
    /// Initialize the contract, setting the admin and default config.
    ///
    /// Panics if already initialized.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        let default_cfg = Config {
            admin: admin.clone(),
            max_royalty_bps: 10_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 0,
        };
        storage::config::set_config(&env, &default_cfg);
    }

    /// Return the current contract config.
    pub fn get_config(env: Env) -> Config {
        storage::config::get_config(&env)
    }

    /// Update contract config. Admin only; validates basis-point ranges.
    pub fn set_config(env: Env, admin: Address, cfg: Config) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        storage::config::validate_config(&cfg)?;
        storage::config::set_config(&env, &cfg);
        Ok(())
    }
}

/// Backward-compatible alias used by configuration tests.
pub type ClipCashNFT = ClipsNftContract;
