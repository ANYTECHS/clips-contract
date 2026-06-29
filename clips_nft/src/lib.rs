#![no_std]

pub mod royalty_config;
pub mod safe_math;

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
mod config;
mod config_guard;
mod config_validator;
mod creator_storage;
mod default_royalty;
mod minted_clip_index;
mod payment_currency;
mod platform_fee;
mod royalty_storage;
mod token_approval;
mod token_metadata_storage;
mod types;
mod wallet_token_index;

pub use blacklist::{add_wallet, is_blacklisted, remove_wallet};
pub use creator_storage::{get_creator, set_creator};
pub use frozen_token::{freeze_token, is_frozen, unfreeze_token};
pub use token_uri_storage::{get_token_uri, set_token_uri};
pub use wallet_token_index::{add_token_to_wallet, get_wallet_tokens, remove_token_from_wallet};
pub use config::{get_config, set_config, Config, CONTRACT_VERSION};
pub use default_royalty::{
    get_default_royalty_bps, set_default_royalty_bps, DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS,
};
pub use operator_approval::{is_operator, remove_operator, save_operator};
pub use pause_state::{get_pause_state, save_pause_state};
pub use platform_fee::{get_platform_fee, set_platform_fee, MAX_PLATFORM_FEE_BPS};
pub use config_guard::require_config_admin;
pub use config_validator::{
    validate_collection_limit, validate_config, validate_fee, validate_royalty_bps, validate_uri,
    MAX_COLLECTION_LIMIT,
};
pub use payment_currency::{add_currency, get_currencies, is_supported, remove_currency};
pub use types::{DataKey, Error, MintEvent, Royalty, RoyaltyInfo, TokenData, TokenId};

use soroban_sdk::{
    contract, contractimpl, BytesN, Env, String,
    Address,
};

#[contract]
pub struct ClipCashNFT;

#[contractimpl]
impl ClipCashNFT {
    // ─── Initialization ──────────────────────────────────────────────────────

    /// Initialize the contract and set the admin.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &0u32);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    // ─── Config ───────────────────────────────────────────────────────────────

    /// Persist a full [`Config`] snapshot. Admin only.
    /// Uses the configuration guard and validator.
    pub fn set_config(env: Env, admin: Address, cfg: Config) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        config_validator::validate_config(&env, &cfg)?;
        config::set_config(&env, cfg)
    }

    /// Return the current [`Config`], or `None` before initialization.
    pub fn get_config(env: Env) -> Option<Config> {
        config::get_config(&env)
    }

    /// Return max batch mint size (defaults to MAX_BATCH_MINT_SIZE if config not set).
    pub fn get_max_batch_mint_size(env: Env) -> u32 {
        config::get_config(&env)
            .map(|c| c.max_batch_mint_size)
            .unwrap_or(MAX_BATCH_MINT_SIZE)
    }

    /// Set max batch mint size (1–100). Admin only.
    pub fn set_max_batch_mint_size(env: Env, admin: Address, value: u32) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        if value < 1 || value > 100 {
            return Err(Error::InvalidConfig);
        }
        let mut cfg = config::get_config(&env).ok_or(Error::NotInitialized)?;
        let old = cfg.max_batch_mint_size;
        cfg.max_batch_mint_size = value;
        if old != value {
            env.events().publish(
                ("config_update",),
                config::ConfigUpdateEvent {
                    key: soroban_sdk::String::from_str(&env, "max_batch_mint_size"),
                    old_value: old,
                    new_value: value,
                    updater: admin.clone(),
                },
            );
        }
        env.storage().instance().set(&DataKey::Config, &cfg);
        Ok(())
    }

    /// Return max collection size (defaults to MAX_COLLECTION_SIZE if config not set).
    pub fn get_max_collection_size(env: Env) -> u32 {
        config::get_config(&env)
            .map(|c| c.max_collection_size)
            .unwrap_or(MAX_COLLECTION_SIZE)
    }

    /// Set max collection size (1–100 000). Admin only.
    pub fn set_max_collection_size(env: Env, admin: Address, value: u32) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        if value < 1 || value > 100_000 {
            return Err(Error::InvalidConfig);
        }
        let mut cfg = config::get_config(&env).ok_or(Error::NotInitialized)?;
        let old = cfg.max_collection_size;
        cfg.max_collection_size = value;
        if old != value {
            env.events().publish(
                ("config_update",),
                config::ConfigUpdateEvent {
                    key: soroban_sdk::String::from_str(&env, "max_collection_size"),
                    old_value: old,
                    new_value: value,
                    updater: admin.clone(),
                },
            );
        }
        env.storage().instance().set(&DataKey::Config, &cfg);
        Ok(())
    }

    // ─── Default Royalty ─────────────────────────────────────────────────────

    /// Set the contract-wide default royalty in basis points (max 10 000 = 100 %).
    /// Admin only. Uses configuration guard.
    pub fn set_default_royalty_bps(env: Env, admin: Address, bps: u32) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        default_royalty::set_default_royalty_bps(&env, bps)
    }

    /// Return the default royalty in basis points (defaults to 500 = 5 %).
    pub fn get_default_royalty_bps(env: Env) -> u32 {
        default_royalty::get_default_royalty_bps(&env)
    }

    // ─── Platform Fee ────────────────────────────────────────────────────────

    /// Set the platform fee in basis points (max 1 000 = 10 %).
    /// Admin only.
    pub fn set_platform_fee(env: Env, admin: Address, fee_bps: u32) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        platform_fee::set_platform_fee(&env, fee_bps)
    }

    /// Return the current platform fee in basis points.
    pub fn get_platform_fee(env: Env) -> u32 {
        platform_fee::get_platform_fee(&env)
    }

    // ─── Payment Currencies ────────────────────────────────────────────────

    /// Add a supported payment currency. Admin only.
    pub fn add_currency(env: Env, admin: Address, currency: Address) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        payment_currency::add_currency(&env, currency)
    }

    /// Remove a supported payment currency. Admin only.
    pub fn remove_currency(env: Env, admin: Address, currency: Address) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        payment_currency::remove_currency(&env, &currency)
    }

    /// Get the list of supported payment currencies.
    pub fn get_currencies(env: Env) -> soroban_sdk::Vec<Address> {
        payment_currency::get_currencies(&env)
    }

    /// Check if a currency is supported for payments.
    pub fn is_currency_supported(env: Env, currency: Address) -> bool {
        payment_currency::is_supported(&env, &currency)
    }

    // ─── Signer ──────────────────────────────────────────────────────────────

    /// Register or rotate the backend Ed25519 signer public key.
    pub fn set_signer(env: Env, admin: Address, pubkey: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Signer, &pubkey);
        Ok(())
    }

    /// Return the currently registered signer, if any.
    pub fn get_signer(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&DataKey::Signer)
    }

    // ─── Pause ───────────────────────────────────────────────────────────────

    /// Pause the contract — blocks mint and transfer.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish(("paused",), ());
        Ok(())
    }

    /// Unpause the contract.
    pub fn unpause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish(("unpaused",), ());
        Ok(())
    }

    /// Returns `true` if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    // ─── Mint ────────────────────────────────────────────────────────────────

    /// Mint a new NFT for a video clip.
    pub fn mint(
        env: Env,
        admin: Address,
        to: Address,
        clip_id: u32,
        metadata_uri: String,
        royalty: Royalty,
        _signature: BytesN<64>,
    ) -> Result<TokenId, Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        Self::require_not_paused(&env)?;

        if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
            return Err(Error::ClipAlreadyMinted);
        }
        if royalty.basis_points > 10_000 {
            return Err(Error::InvalidBasisPoints);
        }

        let token_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);

        token_storage::set_token(&env, token_id, &TokenData { owner: to.clone(), clip_id });
        token_storage::set_metadata(&env, token_id, &metadata_uri);
        token_storage::set_royalty(&env, token_id, &royalty);
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &token_id);
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));

        env.events().publish(
            ("mint",),
            MintEvent {
                to,
                clip_id,
                token_id,
                metadata_uri,
            },
        );
        Ok(token_id)
    }

    // ─── Transfer / Burn ─────────────────────────────────────────────────────

    /// Transfer NFT ownership.
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: TokenId,
    ) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        from.require_auth();
        Self::require_not_paused(&env)?;
        let mut data = token_storage::get_token(&env, token_id)?;
        if data.owner != from {
            return Err(Error::Unauthorized);
        }
        data.owner = to.clone();
        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        env.events().publish(("transfer",), TransferEvent { from, to, token_id });
        Ok(())
    }

    /// Burn an NFT. Only the current owner may burn.
    pub fn burn(env: Env, owner: Address, token_id: TokenId) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        owner.require_auth();
        let data = token_storage::get_token(&env, token_id)?;
        if data.owner != owner {
            return Err(Error::Unauthorized);
        }
        env.storage().persistent().remove(&DataKey::Token(token_id));
        env.storage().persistent().remove(&DataKey::Metadata(token_id));
        env.storage().persistent().remove(&DataKey::Royalty(token_id));
        env.events().publish(("burn",), BurnEvent { owner, token_id });
        Ok(())
    }

    // ─── Queries ─────────────────────────────────────────────────────────────

    /// Returns the owner of a token.
    pub fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        Ok(token_storage::get_token(&env, token_id)?.owner)
    }

    /// Returns the metadata URI of a token.
    pub fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error> {
        token_storage::get_metadata(&env, token_id)
    }

    /// Alias for `token_uri`.
    pub fn get_metadata(env: Env, token_id: TokenId) -> Result<String, Error> {
        Self::token_uri(env, token_id)
    }

    /// Look up token ID by clip ID.
    pub fn clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::ClipIdMinted(clip_id))
            .ok_or(Error::TokenNotFound)
    }

    /// Returns the royalty struct for a token.
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        token_storage::get_royalty(&env, token_id)
    }

    /// Returns royalty receiver and computed amount for a given sale price.
    pub fn royalty_info(
        env: Env,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<RoyaltyInfo, Error> {
        let r = token_storage::get_royalty(&env, token_id)?;
        let amount = sale_price * r.basis_points as i128 / 10_000;
        Ok(RoyaltyInfo {
            receiver: r.recipient,
            royalty_amount: amount,
            asset_address: r.asset_address,
        })
    }

    /// Pay royalties for a token sale. Emits a RoyaltyPaidEvent.
    pub fn pay_royalty(
        env: Env,
        payer: Address,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<(), Error> {
        payer.require_auth();
        if sale_price <= 0 {
            return Err(Error::InvalidBasisPoints);
        }
        let r = Self::get_royalty(env.clone(), token_id)?;
        let amount = sale_price * r.basis_points as i128 / 10_000;
        env.events().publish(
            ("royalty_paid",),
            RoyaltyPaidEvent {
                token_id,
                payer,
                receiver: r.recipient,
                amount,
                asset_address: r.asset_address,
            },
        );
        Ok(())
    }

    /// Update royalty config for a token. Admin only.
    pub fn set_royalty(
        env: Env,
        admin: Address,
        token_id: TokenId,
        new_royalty: Royalty,
    ) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();
        if new_royalty.basis_points > 10_000 {
            return Err(Error::InvalidBasisPoints);
        }
        if !token_storage::token_exists(&env, token_id) {
            return Err(Error::TokenNotFound);
        }
        token_storage::set_royalty(&env, token_id, &new_royalty);
        Ok(())
    }

    /// Returns total minted token count.
    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::NextTokenId).unwrap_or(0)
    }

    /// Returns true if the token exists.
    pub fn exists(env: Env, token_id: TokenId) -> bool {
        token_storage::token_exists(&env, token_id)
    }

    // ─── Internal helpers ────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if *caller != admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), Error> {
        if env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(Error::ContractPaused);
        }
        Ok(())
    }
}
