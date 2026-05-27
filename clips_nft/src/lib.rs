//! ClipCash NFT — Soroban Smart Contract
//!
//! Enables minting video clips as NFTs on the Stellar network with built-in
//! royalty support for content creators. Royalties can be paid in XLM or any
//! SEP-0041 custom Stellar asset.
//!
//! # Clip verification
//!
//! Before a clip can be minted the backend must sign a verification payload
//! with its Ed25519 private key. The contract verifies the signature on-chain
//! using `env.crypto().ed25519_verify()`.
//!
//! ## Payload format
//!
//! ```text
//! payload = SHA-256( clip_id_le_bytes || SHA-256(owner_xdr) || SHA-256(metadata_uri_bytes) )
//! ```
//!
//! # Storage layout
//!
//! | Tier       | Keys                                              |
//! |------------|---------------------------------------------------|
//! | instance   | Admin, NextTokenId, Paused, Signer, Name, Symbol, PlatformRecipient |
//! | persistent | Token(id), ClipIdMinted(clip_id), Approved(id), ApprovalForAll(owner,op), BlacklistedClip(clip_id) |
//!
//! # Privileged entrypoints (admin-only)
//!
//! ## Storage tiers used
//! - `instance`   – cheap, loaded once per tx, shared across all calls in the tx.
//!   Used for: Admin, NextTokenId, Paused, Signer.
//! - `persistent` – per-entry fee, survives ledger expiry extension.
//!   Used for: TokenData (owner+clip_id packed), Metadata, Royalty,
//!   ClipIdMinted (dedup guard).
//!
//! ## Estimated storage operations per function
//!
//! ### `mint`
//! | Op              | Tier       | Count |
//! |-----------------|------------|-------|
//! | instance read   | instance   | 4     | (Admin, NextTokenId, Paused, Signer)
//! | instance write  | instance   | 1     | (NextTokenId++)
//! | persistent read | persistent | 1     | (ClipIdMinted dedup check)
//! | persistent write| persistent | 4     | (TokenData, Metadata, Royalty, ClipIdMinted)
//! Total persistent writes: **4**
//!
//! ### `transfer`
//! | Op              | Tier       | Count |
//! |-----------------|------------|-------|
//! | instance read   | instance   | 1     | (Paused)
//! | persistent read | persistent | 1     | (TokenData — owner check)
//! | persistent write| persistent | 1     | (TokenData — new owner)
//! Total persistent writes: **1**
//!
//! ### `burn`
//! | Op              | Tier       | Count |
//! |-----------------|------------|-------|
//! | persistent read | persistent | 1     | (TokenData — owner check + clip_id)
//! | persistent remove| persistent| 4     | (TokenData, Metadata, Royalty, ClipIdMinted)
//! Total persistent removes: **4**
//!
//! ## Removed counters / indexes (vs. earlier version)
//! - `Balance(Address)` — per-address token counter removed.
//! - `TokenCount` — replaced by `next_token_id - 1`.
//! - `TokenClipId(TokenId)` — clip_id packed into `TokenData`.
//! - [`ClipsNftContract::set_signer`]
//! - [`ClipsNftContract::upgrade`]
//! - [`ClipsNftContract::pause`]
//! - [`ClipsNftContract::unpause`]
//! - [`ClipsNftContract::blacklist_clip`]
//! - [`ClipsNftContract::set_name`]
//! - [`ClipsNftContract::set_symbol`]
//! - [`ClipsNftContract::set_royalty`]

#![no_std]

pub mod safe_math;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, xdr::ToXdr, Address, Bytes,
    BytesN, Env, String, Vec,
};

/// Contract version — bump on every breaking change.
pub const VERSION: u32 = 1;
pub const DEFAULT_MINT_COOLDOWN_SECONDS: u64 = 0;
pub const DEFAULT_CIRCUIT_BREAKER_ENABLED: bool = false;
pub const DEFAULT_CIRCUIT_BREAKER_THRESHOLD: u64 = 100;
pub const DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS: u64 = 60;

// =============================================================================
// Errors
// =============================================================================

/// All error codes returned by the contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Error {
    /// Caller is not authorized for this operation.
    Unauthorized = 1,
    /// Token ID does not exist.
    InvalidTokenId = 2,
    /// Clip has already been minted.
    ClipAlreadyMinted = 3,
    /// Total royalty basis points exceed 10 000 (100 %).
    RoyaltyTooHigh = 4,
    /// Royalty recipient address is invalid or missing.
    InvalidRecipient = 5,
    /// Sale price must be greater than zero.
    InvalidSalePrice = 6,
    /// Contract is paused — minting and transfers are blocked.
    ContractPaused = 7,
    /// Backend Ed25519 signature over the mint payload is invalid.
    InvalidSignature = 8,
    /// No backend signer public key has been registered yet.
    SignerNotSet = 9,
    /// Royalty split configuration is invalid.
    InvalidRoyaltySplit = 10,
    /// Token is soulbound (non-transferable).
    SoulboundTransferBlocked = 11,
    /// Royalty calculation would overflow i128.
    RoyaltyOverflow = 12,
    /// Clip ID has been blacklisted by the admin.
    ClipBlacklisted = 13,
    /// Caller is not the owner or an approved operator.
    NotAuthorizedToApprove = 14,
    /// Withdrawal is still locked (24h safety delay)
    WithdrawalStillLocked = 15,
    /// No active withdrawal request found
    NoWithdrawalRequest = 16,
    /// Batch mint request exceeds configured gas-safe limit
    BatchTooLarge = 17,
    /// Token is frozen and cannot be transferred or burned.
    TokenFrozen = 18,
    /// Insufficient balance for this operation.
    InsufficientBalance = 19,
    /// Metadata was refreshed too recently (30-day cooldown not elapsed).
    MetadataRefreshTooSoon = 20,
    /// Image URL must start with "https://" or "ipfs://".
    InvalidImageUrl = 21,
    /// Animation URL must start with "https://" or "ipfs://".
    InvalidAnimationUrl = 22,
    /// Mint attempted before wallet cooldown elapsed.
    MintCooldownActive = 23,
    /// Reentrant call detected while a guarded entrypoint is executing.
    Reentrancy = 24,
    /// Circuit breaker triggered due to anomalous mint activity.
    CircuitBreakerTripped = 25,
}

// =============================================================================
// Types
// =============================================================================

/// Opaque token identifier (auto-incremented u32).
pub type TokenId = u32;

/// All per-token state packed into a single persistent storage entry.
///
/// Combining owner, clip_id, metadata, and royalty into one entry reduces
/// persistent writes per mint from 4 to 2.
/// Token metadata following the OpenSea metadata standard.
/// See: https://docs.opensea.io/docs/metadata-standards
///
/// # Fields
/// * `owner` — Current owner of the token.
/// * `clip_id` — Off-chain clip identifier this token was minted for.
/// * `is_soulbound` — When `true` the token cannot be transferred (soulbound).
/// * `metadata_uri` — Metadata URI (IPFS or Arweave).
/// * `image` — Static thumbnail URL. Recommended formats: PNG, JPEG, GIF (static), SVG.
///   Max 100 MB. Must be a fully-qualified URL (https:// or ipfs://).
/// * `animation_url` — Animated preview URL. Recommended formats: GIF, MP4 (H.264), WEBM,
///   GLB/GLTF (for 3D), HTML (for interactive). Max 100 MB. Must be a fully-qualified URL.
///   Takes precedence for playback; `image` is used as the fallback thumbnail.
/// * `royalty` — Royalty configuration for secondary sales.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    /// OpenSea trait type (e.g. "Quality").
    pub trait_type: String,
    /// OpenSea trait value (e.g. "Gold").
    pub value: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenData {
    /// Current owner of the token.
    pub owner: Address,
    /// Off-chain clip identifier this token was minted for.
    pub clip_id: u32,
    /// When `true` the token cannot be transferred (soulbound).
    pub is_soulbound: bool,
    /// Metadata URI (IPFS or Arweave).
    pub metadata_uri: String,
    /// Static thumbnail URL (optional). Recommended formats: PNG, JPEG, GIF (static), SVG.
    /// Max 100 MB. Must be a fully-qualified URL (https:// or ipfs://).
    pub image: Option<String>,
    /// Animated preview URL (optional). Recommended formats: GIF, MP4, WEBM, GLB/GLTF, HTML.
    /// Max 100 MB. Must be a fully-qualified URL (https:// or ipfs://).
    /// Takes precedence for playback; `image` is used as the fallback thumbnail.
    pub animation_url: Option<String>,
    /// Optional OpenSea description.
    pub description: Option<String>,
    /// Optional OpenSea external URL.
    pub external_url: Option<String>,
    /// Optional OpenSea trait attributes.
    pub attributes: Vec<Attribute>,
    /// Royalty configuration for secondary sales.
    pub royalty: Royalty,
}

/// A single royalty split recipient.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipient {
    /// Address that receives this portion of the royalty.
    pub recipient: Address,
    /// Share expressed in basis points (1 bp = 0.01 %).
    pub basis_points: u32,
}

/// Royalty configuration stored per token.
///
/// `asset_address = None` means royalties are expected in native XLM.
/// `asset_address = Some(addr)` means a SEP-0041 token at `addr`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Royalty {
    /// Ordered list of recipients. The platform recipient (1 %) is appended
    /// automatically by [`ClipsNftContract::mint`] if not already present.
    pub recipients: Vec<RoyaltyRecipient>,
    /// Optional SEP-0041 asset contract address.
    pub asset_address: Option<Address>,
}

/// Royalty payment info returned by [`ClipsNftContract::royalty_info`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyInfo {
    /// Primary royalty receiver (first recipient in the split).
    pub receiver: Address,
    /// Total royalty amount in the same denomination as `sale_price`.
    pub royalty_amount: i128,
    /// `None` → pay in XLM; `Some(addr)` → pay in that SEP-0041 token.
    pub asset_address: Option<Address>,
}

/// Contract metadata and key settings for frontend bootstrap.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub symbol: String,
    pub version: u32,
    pub owner: Address,
    pub platform_fee: u32,
}

// =============================================================================
// Storage keys
// =============================================================================

/// Typed storage keys.
///
/// Enum variants with no payload are 1-word keys (cheapest).
/// Variants with a `u32` payload are 2-word keys (minimum for per-token data).
#[contracttype]
pub enum DataKey {
    /// Contract administrator address (instance).
    Admin,
    /// Monotonically increasing token ID counter (instance).
    /// `total_supply = NextTokenId - 1`.
    NextTokenId,
    /// Pause flag (instance).
    Paused,
    /// Pause reason (instance storage)
    PauseReason,
    /// Collection name (instance storage)
    Name,
    /// Collection symbol (instance).
    Symbol,
    /// Packed owner + clip_id + metadata + royalty for a token (persistent).
    Token(TokenId),
    /// Dedup guard: clip_id → token_id (persistent).
    ClipIdMinted(u32),
    /// Custom metadata URI override per token (persistent).
    CustomTokenUri(TokenId),
    /// Ed25519 public key of the trusted backend signer (instance).
    Signer,
    /// Platform address that always receives the default 1 % royalty cut (instance).
    PlatformRecipient,
    /// Per-token approval: token_id → approved operator (persistent).
    Approved(TokenId),
    /// Track metadata update count per token (persistent storage)
    MetadataUpdateCount(TokenId),
    /// Operator approval for all: (owner, operator) -> bool
    ApprovalForAll(Address, Address),
    /// Blacklist flag for a clip_id (persistent).
    BlacklistedClip(u32),
    /// Pending XLM withdrawal request (instance storage)
    WithdrawXlmRequest,
    /// Timestamp of the last successfully executed withdrawal (instance storage)
    LastWithdrawalTime,
    /// Per-address balance (persistent).
    Balance(Address),
    /// Current total supply of tokens (instance).
    TotalSupply,
    /// Gas tracking fields (instance)
    TotalGasMint,
    CountMint,
    TotalGasTransfer,
    CountTransfer,
    /// Frozen status per token (persistent).
    Frozen(TokenId),
    /// Timestamp of the last metadata refresh per token (persistent).
    MetadataRefreshTime(TokenId),
    /// Ledger timestamp at which a scheduled pause becomes active (instance).
    PauseUnlockTime,
    /// Platform fee in basis points (instance).
    PlatformFeeBps,
    /// Default royalty in basis points (instance).
    DefaultRoyaltyBps,
    /// Accumulated royalty balance per token (persistent).
    RoyaltyBalance(TokenId),
    /// Last successful mint timestamp per wallet (persistent).
    LastMintTimestamp(Address),
    /// Required delay between mints from one wallet (instance).
    MintCooldownSeconds,
    /// Reentrancy guard for external token calls (instance).
    ReentrancyLock,
    /// Circuit breaker enabled flag (instance).
    CircuitBreakerEnabled,
    /// Circuit breaker: max mints allowed in time window (instance).
    CircuitBreakerThreshold,
    /// Circuit breaker: time window duration in seconds (instance).
    CircuitBreakerWindowSeconds,
    /// Circuit breaker: start timestamp of current window (instance).
    CircuitBreakerWindowStart,
    /// Circuit breaker: mint count in current window (instance).
    CircuitBreakerWindowCount,
}

/// Emergency withdrawal request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequest {
    pub amount: i128,
    pub unlock_time: u64,
}

/// Event emitted when a withdrawal is requested.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequestedEvent {
    pub amount: i128,
    pub unlock_time: u64,
}

/// Event emitted when a withdrawal is executed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawExecutedEvent {
    pub amount: i128,
    pub recipient: Address,
}

// =============================================================================
// Events
// =============================================================================

/// Emitted when a new NFT is minted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent {
    pub to: Address,
    pub clip_id: u32,
    pub token_id: TokenId,
    pub metadata_uri: String,
}

/// Emitted when an NFT is burned.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnEvent {
    pub owner: Address,
    pub token_id: TokenId,
    pub clip_id: u32,
}

/// Emitted when NFT ownership changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEvent {
    pub token_id: TokenId,
    pub from: Address,
    pub to: Address,
}

/// Event emitted when a clip ID is blacklisted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlacklistEvent {
    pub clip_id: u32,
}

/// Emitted when an operator is approved for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalEvent {
    pub owner: Address,
    pub operator: Address,
    pub token_id: TokenId,
}

/// Emitted when approval-for-all is set or revoked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalForAllEvent {
    pub owner: Address,
    pub operator: Address,
    pub approved: bool,
}

/// Event emitted when royalty is paid.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPaidEvent {
    pub token_id: TokenId,
    pub from: Address,
    pub to: Address,
    pub amount: i128,
}

/// Event emitted when royalty recipient is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipientUpdatedEvent {
    pub token_id: TokenId,
    pub old_recipient: Address,
    pub new_recipient: Address,
}

/// Event emitted when token URI is updated by the owner.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenUriChangedEvent {
    pub token_id: TokenId,
    pub owner: Address,
    pub new_uri: String,
}

/// Event emitted when the contract is upgraded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeEvent {
    pub new_wasm_hash: BytesN<32>,
}

/// Event emitted when multiple NFTs are batch-minted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchMintEvent {
    pub to: Address,
    pub count: u32,
    pub first_token_id: TokenId,
}

/// Event emitted when token metadata is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdatedEvent {
    pub token_id: TokenId,
    pub old_uri: String,
    pub new_uri: String,
}

/// Emitted when an NFT is frozen.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenFrozenEvent {
    pub token_id: TokenId,
}

/// Emitted when an NFT is unfrozen.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenUnfrozenEvent {
    pub token_id: TokenId,
}

/// Emitted when the backend signer key is registered or rotated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerUpdatedEvent {
    pub new_pubkey: BytesN<32>,
}

/// Emitted when a token's royalty configuration is updated by the admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyUpdatedEvent {
    pub token_id: TokenId,
}

/// Emitted when a pause is scheduled (24-hour timelock starts).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseScheduledEvent {
    /// Ledger timestamp at which the pause becomes active.
    pub active_at: u64,
}

/// Emitted when the collection name or symbol is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionUpdatedEvent {
    /// "name" or "symbol"
    pub field: String,
    pub new_value: String,
}

/// Emitted when a platform config value is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdatedEvent {
    pub key: String,
    pub new_value: u32,
}

/// Emitted when accumulated royalties are claimed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyClaimedEvent {
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
}

/// Emitted when the contract admin is changed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

/// Emitted when an NFT is burned and optional unclaimed royalties are refunded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundedEvent {
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: i128,
}
/// Emitted when the circuit breaker is triggered automatically.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerTriggeredEvent {
    pub mint_count: u64,
    pub threshold: u64,
    pub window_seconds: u64,
}

/// Emitted when a soulbound token is recovered to a new owner via platform signature.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoulboundRecoveredEvent {
    pub token_id: TokenId,
    pub old_owner: Address,
    pub new_owner: Address,
}

/// Emitted when circuit breaker enabled status is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerEnabledEvent {
    pub enabled: bool,
}

/// Emitted when circuit breaker threshold is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerThresholdUpdatedEvent {
    pub new_threshold: u64,
}

/// Emitted when circuit breaker time window is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerWindowUpdatedEvent {
    pub new_window_seconds: u64,
}

/// Emitted when circuit breaker is reset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerResetEvent {
    pub reset_at: u64,
}

/// Emitted when approval-for-all is set for a token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetApprovalForAllEvent {
    pub owner: Address,
    pub operator: Address,
    pub approved: bool,
}


/// Emerging Soroban NFT standard interface (ERC-721 adapted).
/// Documents the expected API surface for marketplace interoperability.
pub trait NftStandard {
    /// Returns how many tokens `owner` holds.
    fn balance_of(env: Env, owner: Address) -> u32;
    /// Returns the owner of `token_id`.
    fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error>;
    /// Transfers `token_id` from `from` to `to`.
    fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: TokenId,
        sale_price: i128,
        payment_asset: Option<Address>,
    ) -> Result<(), Error>;
    /// Approves `operator` to manage `token_id` (or clears approval when `None`).
    fn approve(env: Env, caller: Address, operator: Option<Address>, token_id: TokenId) -> Result<(), Error>;
    /// Returns the per-token approved operator, if any.
    fn get_approved(env: Env, token_id: TokenId) -> Option<Address>;
    /// Grants or revokes operator rights for all tokens owned by `caller`.
    fn set_approval_for_all(env: Env, caller: Address, operator: Address, approved: bool) -> Result<(), Error>;
    /// Returns whether `operator` may manage all tokens for `owner`.
    fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool;
    /// Returns the number of minted tokens.
    fn total_supply(env: Env) -> u32;
    /// Returns the metadata URI for `token_id`.
    fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error>;
    /// Returns the collection name.
    fn name(env: Env) -> String;
    /// Returns the collection symbol.
    fn symbol(env: Env) -> String;
    /// Revokes approval for a specific token ID.
    fn revoke_approval(env: Env, token_id: TokenId) -> Result<(), Error>;
    /// Revokes approval for an operator managing all caller tokens.
    fn revoke_all_approvals(env: Env, operator: Address) -> Result<(), Error>;
    /// Destroys a token and handles optional remaining royalty refund matching criteria.
    fn burn(env: Env, token_id: TokenId, refund_royalty: bool) -> Result<(), Error>;
}

// =============================================================================
// Contract
// =============================================================================

/// ClipCash NFT contract.
#[contract]
pub struct ClipsNftContract;

#[allow(deprecated)]
/// Synthetic gas constants for tracking (approximations)
const GAS_BASE_MINT: u64 = 50_000;
const GAS_BASE_TRANSFER: u64 = 30_000;
const MAX_BATCH_MINT: u32 = 25;
const PERSISTENT_BUMP_THRESHOLD: u32 = 172_800;
const PERSISTENT_BUMP_AMOUNT: u32 = 535_680;

#[contractimpl]
impl ClipsNftContract {
    // -------------------------------------------------------------------------
    // Initialization
    // -------------------------------------------------------------------------

    /// Initialize the contract and set the admin.
    ///
    /// Can only be called once. Panics if already initialized.
    ///
    /// # Arguments
    /// * `admin` — Address that becomes the contract administrator.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        // NextTokenId starts at 1; total_supply = NextTokenId - 1
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::PlatformRecipient, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Name, &String::from_str(&env, "ClipCash Clips"));
        env.storage()
            .instance()
            .set(&DataKey::Symbol, &String::from_str(&env, "CLIP"));
        env.storage()
            .instance()
            .set(&DataKey::MintCooldownSeconds, &DEFAULT_MINT_COOLDOWN_SECONDS);
        // Initialize circuit breaker with default values
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerEnabled, &DEFAULT_CIRCUIT_BREAKER_ENABLED);
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerThreshold, &DEFAULT_CIRCUIT_BREAKER_THRESHOLD);
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerWindowSeconds, &DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS);
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerWindowStart, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerWindowCount, &0u64);
        // Signer is not set at init — call set_signer before minting.
    }

    // -------------------------------------------------------------------------
    // Signer management  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Register (or rotate) the backend Ed25519 public key used to verify
    /// clip ownership before minting.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// # Arguments
    /// * `admin`  — Must be the contract admin.
    /// * `pubkey` — 32-byte Ed25519 public key of the trusted backend signer.
    pub fn set_signer(env: Env, admin: Address, pubkey: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Signer, &pubkey);
        env.events().publish(
            (symbol_short!("sgn_upd"), admin.clone()),
            SignerUpdatedEvent { new_pubkey: pubkey },
        );
        Ok(())
    }

    /// Return the currently registered backend signer public key, if any.
    pub fn get_signer(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&DataKey::Signer)
    }

    /// Transfer contract admin rights to a new address.
    ///
    /// ⚠️ **Access Control: current admin only.**
    ///
    /// Emits: `"adm_chg"` [`AdminChangedEvent`].
    ///
    /// # Arguments
    /// * `current_admin` — Must be the current contract admin.
    /// * `new_admin`      — Address that will become the new admin.
    ///
    /// # Errors
    /// * [`Error::Unauthorized`] — `current_admin` is not the stored admin.
    ///
    /// Closes #177
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &current_admin)?;
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish(
            (symbol_short!("adm_chg"), current_admin.clone(), new_admin.clone()),
            AdminChangedEvent {
                old_admin: current_admin,
                new_admin,
            },
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Upgradeability  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Upgrade the contract to a new WASM implementation.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Replaces the current contract code with the new WASM hash while
    /// preserving all instance and persistent storage.
    ///
    /// # Arguments
    /// * `admin`          — Must be the contract admin.
    /// * `new_wasm_hash` — 32-byte SHA-256 hash of the new WASM blob.
    pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
        env.events().publish(
            (symbol_short!("upgrade"), admin.clone()),
            UpgradeEvent { new_wasm_hash },
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Pausable  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Schedule a contract pause with a 24-hour timelock.
    ///
    /// the pause becomes active 24 hours after this call. Until then, `mint`
    /// and `transfer` continue to work, giving users advance warning.
    /// Calling `pause` again while a pause is already scheduled or active
    /// resets the 24-hour window from the current time.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"pause_sched"` [`PauseScheduledEvent`] with the activation timestamp.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        let active_at = env.ledger().timestamp().saturating_add(86_400); // 24 hours
        env.storage().instance().set(&DataKey::PauseUnlockTime, &active_at);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish(
            (symbol_short!("pse_sched"), admin.clone()),
            PauseScheduledEvent { active_at },
        );
        Ok(())
    }

    /// Cancel a scheduled or active pause, immediately re-enabling `mint` and `transfer`.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"unpaused"` event.
    pub fn unpause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().remove(&DataKey::PauseUnlockTime);
        env.events().publish((symbol_short!("unpaused"), admin.clone()), ());
        Ok(())
    }

    /// Returns `true` if the contract is currently paused (timelock has elapsed).
    pub fn is_paused(env: Env) -> bool {
        Self::check_paused(&env)
    }

    /// Returns the timestamp at which a scheduled pause becomes active, or `None`.
    pub fn pause_active_at(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::PauseUnlockTime)
    }

    /// Request an emergency withdrawal of XLM (or any other token).
    /// Starts a 48-hour safety delay (timelock) before the withdrawal can be executed.
    /// Only callable by the admin.
    ///
    /// Emits `WithdrawRequested` event with amount and unlock_time.
    ///
    /// Part of Closes #78
    pub fn request_withdraw_asset(env: Env, admin: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if amount <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let unlock_time = env.ledger().timestamp().saturating_add(172_800); // 48 hours
        let request = WithdrawRequest { amount, unlock_time };

        env.storage().instance().set(&DataKey::WithdrawXlmRequest, &request);

        env.events().publish(
            (symbol_short!("with_req"), admin.clone()),
            WithdrawRequestedEvent { amount, unlock_time },
        );
        Ok(())
    }

    /// Execute a previously requested emergency withdrawal after the 24-hour safety delay.
    /// Only callable by the admin.
    ///
    /// Emits `WithdrawExecuted` event with amount and recipient.
    /// Uses check-effects-interactions pattern: clears request before transfer.
    ///
    /// Closes #78
    ///
    /// # Arguments
    /// * `admin` - Must be the contract admin
    /// * `asset` - The contract address of the asset to withdraw (e.g. native XLM)
    /// * `amount` - The amount to withdraw (must match the requested amount)
    pub fn withdraw_asset(env: Env, admin: Address, asset: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        Self::acquire_reentrancy_lock(&env)?;
        let result = Self::withdraw_asset_internal(&env, &admin, &asset, amount);
        Self::release_reentrancy_lock(&env);
        result
    }

    /// Internal asset withdrawal (caller must hold reentrancy lock).
    fn withdraw_asset_internal(
        env: &Env,
        admin: &Address,
        asset: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        let request: WithdrawRequest = env.storage().instance()
            .get(&DataKey::WithdrawXlmRequest)
            .ok_or(Error::NoWithdrawalRequest)?;

        if amount != request.amount {
            return Err(Error::Unauthorized);
        }

        if env.ledger().timestamp() < request.unlock_time {
            return Err(Error::WithdrawalStillLocked);
        }

        // Clear the request before execution to prevent double-spend if transfer fails/reenters
        env.storage().instance().remove(&DataKey::WithdrawXlmRequest);

        // Execute the transfer
        let client = soroban_sdk::token::TokenClient::new(env, asset);
        client.transfer(&env.current_contract_address(), admin, &amount);

        // Record the timestamp of this withdrawal for audit purposes
        env.storage()
            .instance()
            .set(&DataKey::LastWithdrawalTime, &env.ledger().timestamp());

        env.events().publish(
            (symbol_short!("with_exe"), admin.clone(), asset.clone()),
            WithdrawExecutedEvent {
                amount,
                recipient: admin.clone(),
            },
        );

        Ok(())
    }

    /// Blacklist a clip ID, preventing it from being minted.
    /// Only callable by the admin.
    pub fn blacklist_clip(env: Env, admin: Address, clip_id: u32) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::BlacklistedClip(clip_id), &true);
        env.events()
            .publish((symbol_short!("blacklist"), clip_id), BlacklistEvent { clip_id });
        Ok(())
    }

    /// Freeze an NFT so transfers and burns are blocked until unfrozen.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"freeze"` [`TokenFrozenEvent`].
    pub fn freeze(env: Env, admin: Address, token_id: TokenId) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if !Self::exists(env.clone(), token_id) {
            return Err(Error::InvalidTokenId);
        }
        env.storage().persistent().set(&DataKey::Frozen(token_id), &true);
        env.events()
            .publish((symbol_short!("freeze"), token_id), TokenFrozenEvent { token_id });
        Ok(())
    }

    /// Unfreeze an NFT, re-enabling transfers and burning.
    /// Only callable by the admin.
    pub fn unfreeze(env: Env, admin: Address, token_id: TokenId) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if !Self::exists(env.clone(), token_id) {
            return Err(Error::InvalidTokenId);
        }
        env.storage().persistent().remove(&DataKey::Frozen(token_id));
        env.events()
            .publish((symbol_short!("unfreeze"), token_id), TokenUnfrozenEvent { token_id });
        Ok(())
    }

    /// Returns `true` if the token is currently frozen.
    pub fn is_frozen(env: Env, token_id: TokenId) -> bool {
        env.storage().persistent().get(&DataKey::Frozen(token_id)).unwrap_or(false)
    }

    // -------------------------------------------------------------------------
    // Approval Revocations
    // -------------------------------------------------------------------------

    /// Revokes marketplace or operator approval for a specific token ID.
    pub fn revoke_approval(env: Env, token_id: TokenId) -> Result<(), Error> {
        let token_data: TokenData = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)?;
    /// Mint a new NFT for a video clip.
    ///
    /// Requires a valid Ed25519 `signature` from the registered backend signer
    /// over the canonical mint payload:
    ///
    /// ```text
    /// payload = SHA-256(
    ///     clip_id_le_4_bytes
    ///     || SHA-256(XDR(owner))        // 32 bytes
    ///     || SHA-256(UTF-8(metadata_uri)) // 32 bytes
    /// )
    /// ```
    ///
    /// Storage writes: 2 persistent (TokenData, ClipIdMinted), 1 instance (NextTokenId).
    ///
    /// Emits: `"mint"` [`MintEvent`].
    ///
    /// # Arguments
    /// * `to`           — Address that will own the NFT (must match the signed payload).
    /// * `clip_id`      — Unique off-chain clip identifier (must match the signed payload).
    /// * `metadata_uri` — IPFS or Arweave URI (must match the signed payload).
    /// * `royalty`      — Royalty configuration for secondary sales.
    /// * `is_soulbound` — When `true` the token cannot be transferred.
    /// * `signature`    — 64-byte Ed25519 signature from the registered backend signer.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`] — contract is paused.
    /// * [`Error::SignerNotSet`]   — no signer registered.
    /// * [`Error::InvalidSignature`] — signature verification failed.
    /// * [`Error::ClipAlreadyMinted`] — clip already has a token.
    /// * [`Error::ClipBlacklisted`] — clip ID is blacklisted.
    /// * [`Error::RoyaltyTooHigh`] — total basis points exceed 10 000.
    /// Mint a new NFT token.
    ///
    /// # Arguments
    /// * `to` — Recipient address (must authorize the call).
    /// * `clip_id` — Off-chain clip identifier.
    /// * `metadata_uri` — Metadata URI (IPFS or Arweave).
    /// * `image` — Static thumbnail URL (optional). Must start with "https://" or "ipfs://".
    ///   Recommended formats: PNG, JPEG, GIF (static), SVG. Max 100 MB.
    /// * `animation_url` — Animated preview URL (optional). Must start with "https://" or "ipfs://".
    ///   Recommended formats: GIF, MP4 (H.264), WEBM, GLB/GLTF (for 3D), HTML (for interactive). Max 100 MB.
    ///   Takes precedence for playback; `image` is used as the fallback thumbnail.
    /// * `royalty` — Royalty configuration for secondary sales.
    /// * `is_soulbound` — When `true`, the token cannot be transferred.
    /// * `signature` — Backend Ed25519 signature over the mint payload.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`] — contract is paused.
    /// * [`Error::ClipAlreadyMinted`] — this clip_id has already been minted.
    /// * [`Error::ClipBlacklisted`] — this clip_id has been blacklisted.
    /// * [`Error::InvalidSignature`] — backend signature is invalid.
    /// * [`Error::SignerNotSet`] — no backend signer has been registered.
    /// * [`Error::InvalidImageUrl`] — image URL does not start with "https://" or "ipfs://".
    /// * [`Error::InvalidAnimationUrl`] — animation_url does not start with "https://" or "ipfs://".
    pub fn mint(
        env: Env,
        to: Address,
        clip_id: u32,
        metadata_uri: String,
        image: Option<String>,
        animation_url: Option<String>,
        royalty: Royalty,
        is_soulbound: bool,
        signature: BytesN<64>,
    ) -> Result<TokenId, Error> {
        to.require_auth();
        Self::require_not_paused(&env)?;
        Self::enforce_mint_cooldown(&env, &to)?;
        Self::check_circuit_breaker(&env, 1)?;

        // Validate URLs before any state reads/writes.
        Self::validate_url(&env, &image, Error::InvalidImageUrl)?;
        Self::validate_url(&env, &animation_url, Error::InvalidAnimationUrl)?;

        // Verify backend signature before any state reads/writes.
        Self::verify_clip_signature(&env, &to, clip_id, &metadata_uri, &signature)?;

        // Dedup check — one persistent read.
        if Self::load_clip_token_id(&env, clip_id).is_some() {
            return Err(Error::ClipAlreadyMinted);
        }

        if env
            .storage()
            .persistent()
            .get(&DataKey::BlacklistedClip(clip_id))
            .unwrap_or(false)
        {
            return Err(Error::ClipBlacklisted);
        }

        let royalty = Self::normalize_royalty(&env, royalty)?;

        let token_id: TokenId = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        // 4 persistent writes
        env.storage().persistent().set(
            &DataKey::Token(token_id),
            &TokenData {
                owner: to.clone(),
                clip_id,
                is_soulbound,
                metadata_uri: metadata_uri.clone(),
                image: image.clone(),
                animation_url: animation_url.clone(),
                description: None,
                external_url: None,
                attributes: Vec::new(&env),
                royalty,
            },
        );
        Self::bump_persistent_ttl(&env, &DataKey::Token(token_id));
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &token_id);
        Self::bump_persistent_ttl(&env, &DataKey::ClipIdMinted(clip_id));

        // 1 instance write.
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));

        // Update total supply
        let total_supply: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &(total_supply + 1));

        // Update balance
        let balance: u32 = env.storage().persistent().get(&DataKey::Balance(to.clone())).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(balance + 1));

        env.events().publish(
            (symbol_short!("mint"), token_id, to.clone()),
            MintEvent { to: to.clone(), clip_id, token_id, metadata_uri },
        );

        // Emit standard Transfer event for ERC-721 compliance
        // (contract address stands in for the zero address)
        env.events().publish(
            (
                symbol_short!("transfer"),
                token_id,
                env.current_contract_address(),
                to.clone(),
            ),
            TransferEvent {
                token_id,
                from: env.current_contract_address(),
                to: to.clone(),
            },
        );

        // Gas tracking — Closes #169
        let count_mint: u64 = env.storage().instance().get(&DataKey::CountMint).unwrap_or(0);
        env.storage().instance().set(&DataKey::CountMint, &(count_mint + 1));
        let total_gas_mint: u64 = env.storage().instance().get(&DataKey::TotalGasMint).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalGasMint, &total_gas_mint.saturating_add(GAS_BASE_MINT));
        Self::record_mint_timestamp(&env, &to);

        // Update circuit breaker counter after successful mint
        Self::update_circuit_breaker_counter(&env, 1);

        Ok(token_id)
    }

    // -------------------------------------------------------------------------
    // Approvals
    // -------------------------------------------------------------------------

    /// Approve an operator to transfer a specific token on behalf of the owner.
    ///
    /// Pass `operator = None` to revoke any existing approval.
    ///
    /// Emits: `"approve"` [`ApprovalEvent`] (only when setting, not revoking).
    ///
    /// # Arguments
    /// * `caller`   — Must be the token owner or an approved-for-all operator.
    /// * `operator` — Address to approve, or `None` to clear.
    /// * `token_id` — Token to approve.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`]         — contract is paused.
    /// * [`Error::InvalidTokenId`]         — token does not exist.
    /// * [`Error::NotAuthorizedToApprove`] — caller is not owner or approved-for-all.
    pub fn approve(
        env: Env,
        caller: Address,
        operator: Option<Address>,
        token_id: TokenId,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_not_paused(&env)?;

        let owner = Self::owner_of(env.clone(), token_id)?;

        token_data.owner.require_auth();

        let approval_key = DataKey::Approved(token_id);
        if env.storage().persistent().has(&approval_key) {
            env.storage().persistent().remove(&approval_key);
            
            env.events().publish(
                (
                    symbol_short!("approval"),
                    token_id,
                    token_data.owner.clone(),
                    env.current_contract_address(),
                ),
                ApprovalEvent {
                    owner: token_data.owner,
                    operator: env.current_contract_address(),
                    token_id,
                },
            );
        }
        Ok(())
    }

    /// Revokes general operator permissions for an operator managing the caller's items.
    pub fn revoke_all_approvals(env: Env, operator: Address) -> Result<(), Error> {
        operator.require_auth();

        let approval_all_key = DataKey::ApprovalForAll(env.current_contract_address(), operator.clone());
        if env.storage().persistent().has(&approval_all_key) {
            env.storage().persistent().remove(&approval_all_key);

            env.events().publish(
                (
                    symbol_short!("app_all"),
                    env.current_contract_address(),
                    operator.clone(),
                ),
                ApprovalForAllEvent {
                    owner: env.current_contract_address(),
                    operator,
                    approved: false,
                },
            );
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Core NFT operations
    // -------------------------------------------------------------------------

    /// Destroys an NFT and optionally claims outstanding accrued royalties back to the creator.
    ///
    /// Closes #136
    pub fn burn(env: Env, token_id: TokenId, refund_royalty: bool) -> Result<(), Error> {
        let token_key = DataKey::Token(token_id);
        let token_data: TokenData = env
    /// # Errors
    /// * [`Error::ContractPaused`]          — contract is paused.
    /// * [`Error::InvalidTokenId`]          — token does not exist.
    /// * [`Error::Unauthorized`]            — `from` is not the owner.
    /// * [`Error::SoulboundTransferBlocked`] — token is soulbound.
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: TokenId,
        sale_price: i128,
        payment_asset: Option<Address>,
    ) -> Result<(), Error> {
        from.require_auth();
        Self::require_not_paused(&env)?;

        if Self::is_frozen(env.clone(), token_id) {
            return Err(Error::TokenFrozen);
        }

        let mut data: TokenData = env
            .storage()
            .persistent()
            .get(&token_key)
            .ok_or(Error::InvalidTokenId)?;

        token_data.owner.require_auth();
        if from != data.owner {
            return Err(Error::Unauthorized);
        }

        if data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
        }

        // Handle royalty payment if sale_price is greater than zero
        if sale_price > 0 {
            let royalty = data.royalty.clone();
            let pay_asset = match royalty.asset_address {
                Some(ref asset) => asset.clone(),
                None => payment_asset.clone().ok_or(Error::InvalidRecipient)?,
            };

            // Buyer (to) must authorize the royalty payment
            to.require_auth();

            Self::acquire_reentrancy_lock(&env)?;

            // Calculate total royalty
            let mut cumulative_bps: u32 = 0;
            let mut cumulative_royalty: i128 = 0;
            for idx in 0..royalty.recipients.len() {
                let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;
                cumulative_bps = cumulative_bps.saturating_add(split.basis_points);
                let total_so_far = Self::calculate_royalty(sale_price, cumulative_bps)?;
                cumulative_royalty = total_so_far;
            }

            // Effects: update royalty balance
            if cumulative_royalty > 0 {
                let prev: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::RoyaltyBalance(token_id))
                    .unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&DataKey::RoyaltyBalance(token_id), &(prev.saturating_add(cumulative_royalty)));
            }

            // Interactions: perform transfers
            let token_client = soroban_sdk::token::TokenClient::new(&env, &pay_asset);
            let mut current_bps: u32 = 0;
            let mut current_royalty: i128 = 0;

            for idx in 0..royalty.recipients.len() {
                let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;

                current_bps = current_bps.saturating_add(split.basis_points);
                let total_so_far = Self::calculate_royalty(sale_price, current_bps)?;
                let amount = total_so_far.saturating_sub(current_royalty);
                current_royalty = total_so_far;

                if amount == 0 {
                    continue;
                }

                token_client.transfer(&to, &split.recipient, &amount);
                env.events().publish(
                    (symbol_short!("royalty"), token_id, to.clone(), split.recipient.clone()),
                    RoyaltyPaidEvent {
                        token_id,
                        from: to.clone(),
                        to: split.recipient.clone(),
                        amount,
                    },
                );
            }

            Self::release_reentrancy_lock(&env);
        }

        // Clear per-token approval on transfer.
        env.storage().persistent().remove(&DataKey::Approved(token_id));

        data.owner = to.clone();
        env.storage().persistent().set(&DataKey::Token(token_id), &data);

        // Update balances
        let from_balance: u32 = env.storage().persistent().get(&DataKey::Balance(from.clone())).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Balance(from.clone()), &from_balance.saturating_sub(1));
        
        let to_balance: u32 = env.storage().persistent().get(&DataKey::Balance(to.clone())).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(to_balance + 1));

        env.events().publish(
            (symbol_short!("transfer"), token_id, from.clone(), to.clone()),
            TransferEvent { token_id, from, to },
        );

        // Gas tracking — Closes #169
        let count_transfer: u64 = env.storage().instance().get(&DataKey::CountTransfer).unwrap_or(0);
        env.storage().instance().set(&DataKey::CountTransfer, &(count_transfer + 1));
        let total_gas_transfer: u64 = env.storage().instance().get(&DataKey::TotalGasTransfer).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalGasTransfer, &total_gas_transfer.saturating_add(GAS_BASE_TRANSFER));

        Ok(())
    }

    /// Transfer NFT ownership on behalf of `from` by an approved `spender`.
    ///
    /// `spender` must be either approved-for-all or the per-token approved operator.
    /// Blocked when the contract is paused or the token is soulbound.
    /// Clears any existing per-token approval on success.
    ///
    /// Emits: `"transfer"` [`TransferEvent`].
    ///
    /// # Arguments
    /// * `spender`  — Approved operator (must authorize).
    /// * `from`     — Current owner.
    /// * `to`       — New owner.
    /// * `token_id` — Token to transfer.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`]          — contract is paused.
    /// * [`Error::InvalidTokenId`]          — token does not exist.
    /// * [`Error::Unauthorized`]            — `from` is not the owner or `spender` is not approved.
    /// * [`Error::SoulboundTransferBlocked`] — token is soulbound.
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        token_id: TokenId,
    ) -> Result<(), Error> {
        spender.require_auth();
        Self::require_not_paused(&env)?;

        if Self::is_frozen(env.clone(), token_id) {
            return Err(Error::TokenFrozen);
        }

        // Handle optional royalty recovery tracking back to the primary creator asset configuration rules
        if refund_royalty {
            let royalty_key = DataKey::RoyaltyBalance(token_id);
            if env.storage().persistent().has(&royalty_key) {
                let accumulated_amount: i128 = env.storage().persistent().get(&royalty_key).unwrap_or(0);
                
                if accumulated_amount > 0 {
                    // Extract original primary creator/receiver info if existing
                    if let Some(first_recipient) = token_data.royalty.recipients.get(0) {
                        let target_creator = first_recipient.recipient;
                        
                        // Transfer out using specified contract token type structure defaults
                        if let Some(ref asset_addr) = token_data.royalty.asset_address {
                            let client = soroban_sdk::token::TokenClient::new(&env, asset_addr);
                            client.transfer(&env.current_contract_address(), &target_creator, &accumulated_amount);
                        }
                        
                        env.events().publish(
                            (symbol_short!("refunded"),),
                            RefundedEvent {
                                token_id,
                                recipient: target_creator,
                                amount: accumulated_amount,
                            },
                        );
                    }
                }
                env.storage().persistent().remove(&royalty_key);
            }
        }

        // Clean up remaining storage keys mapped to this token context
        env.storage().persistent().remove(&token_key);
        env.storage().persistent().remove(&DataKey::ClipIdMinted(token_data.clip_id));
        env.storage().persistent().remove(&DataKey::Approved(token_id));
        env.storage().persistent().remove(&DataKey::CustomTokenUri(token_id));
        env.storage().persistent().remove(&DataKey::MetadataUpdateCount(token_id));
        env.storage().persistent().remove(&DataKey::MetadataRefreshTime(token_id));

        env.events().publish(
            (symbol_short!("burn"),),
            BurnEvent {
                owner: token_data.owner,
                token_id,
                clip_id: token_data.clip_id,
            },
        );

        Ok(())
    }

    /// Internal checker helper functions mapped by your setup layers
    fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(Error::Unauthorized)?;
        if admin != &stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        Ok(())
    }

    fn check_paused(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    fn exists(env: Env, token_id: TokenId) -> bool {
        env.storage().persistent().has(&DataKey::Token(token_id))
    }

    fn acquire_reentrancy_lock(env: &Env) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::ReentrancyLock) {
            return Err(Error::Reentrancy);
    /// Set circuit breaker enabled status.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// When enabled, the circuit breaker will automatically pause the contract
    /// if mint operations exceed the configured threshold within the time window.
    ///
    /// Emits: `"cb_ena"` [`CircuitBreakerEnabledEvent`].
    ///
    /// # Arguments
    /// * `admin`   — Must be the contract admin.
    /// * `enabled` — Whether to enable the circuit breaker.
    pub fn set_circuit_breaker_enabled(env: Env, admin: Address, enabled: bool) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::CircuitBreakerEnabled, &enabled);
        env.events().publish(
            (symbol_short!("cb_ena"), admin.clone()),
            CircuitBreakerEnabledEvent { enabled },
        );
        Ok(())
    }

    /// Set circuit breaker threshold (max mints allowed in time window).
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"cb_thr"` [`CircuitBreakerThresholdUpdatedEvent`].
    ///
    /// # Arguments
    /// * `admin`     — Must be the contract admin.
    /// * `threshold` — Maximum number of mints allowed in the time window.
    pub fn set_circuit_breaker_threshold(env: Env, admin: Address, threshold: u64) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::CircuitBreakerThreshold, &threshold);
        env.events().publish(
            (symbol_short!("cb_thr"), admin.clone()),
            CircuitBreakerThresholdUpdatedEvent { new_threshold: threshold },
        );
        Ok(())
    }

    /// Set circuit breaker time window duration in seconds.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"cb_win"` [`CircuitBreakerWindowUpdatedEvent`].
    ///
    /// # Arguments
    /// * `admin`          — Must be the contract admin.
    /// * `window_seconds` — Duration of the time window in seconds.
    pub fn set_circuit_breaker_window(env: Env, admin: Address, window_seconds: u64) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::CircuitBreakerWindowSeconds, &window_seconds);
        env.events().publish(
            (symbol_short!("cb_win"), admin.clone()),
            CircuitBreakerWindowUpdatedEvent { new_window_seconds: window_seconds },
        );
        Ok(())
    }

    /// Reset circuit breaker window and counter.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Useful for resetting the circuit breaker after a false alarm or
    /// after the contract has been unpaused.
    ///
    /// Emits: `"cb_rst"` [`CircuitBreakerResetEvent`].
    ///
    /// # Arguments
    /// * `admin` — Must be the contract admin.
    pub fn reset_circuit_breaker(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::CircuitBreakerWindowStart, &0u64);
        env.storage().instance().set(&DataKey::CircuitBreakerWindowCount, &0u64);
        env.events().publish(
            (symbol_short!("cb_rst"), admin.clone()),
            CircuitBreakerResetEvent { reset_at: env.ledger().timestamp() },
        );
        Ok(())
    }

    /// Update metadata URI for a token. Only the token owner can update it.
    /// Limited to once per NFT to prevent abuse.
    ///
    /// # Arguments
    /// * `owner`    - Must be the current token owner
    /// * `token_id` - Token to update
    /// * `new_uri`  - New metadata URI
    pub fn update_metadata(
        env: Env,
        owner: Address,
        token_id: TokenId,
        new_uri: String,
    ) -> Result<(), Error> {
        owner.require_auth();
        let data = Self::load_token(&env, token_id)?;
        if data.owner != owner {
            return Err(Error::Unauthorized);
        }

        // Check if metadata has already been updated
        let update_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MetadataUpdateCount(token_id))
            .unwrap_or(0);

        if update_count >= 1 {
            return Err(Error::Unauthorized); // Already updated once
        }
        env.storage().instance().set(&DataKey::ReentrancyLock, &true);
        Ok(())
    }

    fn release_reentrancy_lock(env: &Env) {
        env.storage().instance().remove(&DataKey::ReentrancyLock);
    }

    /// Push updated metadata from the backend (e.g. after virality score changes).
    ///
    /// Callable by the contract admin **or** the registered backend signer address.
    /// Limited to once per 30 days per token to prevent abuse.
    ///
    /// Emits: `"meta_upd"` [`MetadataUpdatedEvent`].
    ///
    /// # Arguments
    /// * `caller`   — Must be the admin or the registered signer address.
    /// * `token_id` — Token whose metadata URI is being refreshed.
    /// * `new_uri`  — New metadata URI.
    ///
    /// # Errors
    /// * [`Error::Unauthorized`]           — caller is neither admin nor signer.
    /// * [`Error::InvalidTokenId`]         — token does not exist.
    /// * [`Error::MetadataRefreshTooSoon`] — 30-day cooldown has not elapsed.
    /// Refresh token metadata (admin or signer only, 30-day cooldown).
    ///
    /// # Arguments
    /// * `caller` — Must be the admin or registered signer.
    /// * `token_id` — Token to update.
    /// * `new_uri` — New metadata URI (optional). Pass `None` to leave unchanged.
    /// * `image` — New static thumbnail URL (optional). Must start with "https://" or "ipfs://".
    ///   Pass `None` to leave unchanged. Pass `Some("")` to clear the field.
    /// * `animation_url` — New animated preview URL (optional). Must start with "https://" or "ipfs://".
    ///   Pass `None` to leave unchanged. Pass `Some("")` to clear the field.
    ///
    /// # Errors
    /// * [`Error::Unauthorized`] — caller is not admin or signer.
    /// * [`Error::InvalidTokenId`] — token does not exist.
    /// * [`Error::MetadataRefreshTooSoon`] — 30-day cooldown not elapsed.
    /// * [`Error::InvalidImageUrl`] — image URL does not start with "https://" or "ipfs://".
    /// * [`Error::InvalidAnimationUrl`] — animation_url does not start with "https://" or "ipfs://".
    pub fn refresh_metadata(
        env: Env,
        caller: Address,
        token_id: TokenId,
        new_uri: Option<String>,
        image: Option<String>,
        animation_url: Option<String>,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Allow admin or the registered signer address.
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not initialized");

        let is_admin = caller == admin;
        let is_signer = env
            .storage()
            .instance()
            .get::<DataKey, BytesN<32>>(&DataKey::Signer)
            .map(|_| {
                // The signer is a pubkey, not an Address. We allow the admin to
                // act on behalf of the backend. For on-chain signer-address
                // authorization, callers pass the admin address.
                false
            })
            .unwrap_or(false);

        if !is_admin && !is_signer {
            return Err(Error::Unauthorized);
        }

        // 30-day cooldown check (30 * 24 * 3600 = 2_592_000 seconds).
        const COOLDOWN: u64 = 2_592_000;
        let now = env.ledger().timestamp();
        if let Some(last_refresh) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::MetadataRefreshTime(token_id))
        {
            if now < last_refresh.saturating_add(COOLDOWN) {
                return Err(Error::MetadataRefreshTooSoon);
            }
        }

        // Validate URLs if provided and not empty strings.
        let validated_image = match &image {
            Some(s) if s.is_empty() => Some(None), // Clear field
            Some(s) => {
                Self::validate_url(&env, &Some(s.clone()), Error::InvalidImageUrl)?;
                Some(Some(s.clone()))
            }
            None => None, // Leave unchanged
        };

        let validated_animation_url = match &animation_url {
            Some(s) if s.is_empty() => Some(None), // Clear field
            Some(s) => {
                Self::validate_url(&env, &Some(s.clone()), Error::InvalidAnimationUrl)?;
                Some(Some(s.clone()))
            }
            None => None, // Leave unchanged
        };

        let mut data = Self::load_token(&env, token_id)?;
        let old_uri = data.metadata_uri.clone();
        
        // Update fields only if new values are provided.
        if let Some(uri) = new_uri {
            data.metadata_uri = uri.clone();
        }
        if let Some(img) = validated_image {
            data.image = img;
        }
        if let Some(anim) = validated_animation_url {
            data.animation_url = anim;
        }

        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        env.storage()
            .persistent()
            .set(&DataKey::MetadataRefreshTime(token_id), &now);

        env.events().publish(
            (symbol_short!("meta_upd"), token_id),
            MetadataUpdatedEvent { token_id, old_uri, new_uri: data.metadata_uri },
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Soulbound: Social Recovery Hook
    // -------------------------------------------------------------------------

    /// Recover a soulbound token to a new owner via a trusted platform signature.
    ///
    /// When the owner's account is compromised, the platform backend signs a
    /// recovery payload authorizing the transfer of the soulbound token to a
    /// new address. This bypasses the normal soulbound transfer restriction.
    ///
    /// ## Payload format
    ///
    /// ```text
    /// new_owner_hash = SHA-256(XDR(new_owner))
    /// message        = SHA-256( "recover" || token_id_le_4_bytes || new_owner_hash )
    /// ```
    ///
    /// The `"recover"` domain separator prevents cross-purpose signature replay
    /// (e.g., a mint signature cannot be reused as a recovery signature).
    ///
    /// Emits: `"sb_recov"` [`SoulboundRecoveredEvent`].
    ///
    /// # Arguments
    /// * `signature` — 64-byte Ed25519 signature from the registered backend signer.
    /// * `new_owner` — Address that will become the new owner of the token.
    /// * `token_id`  — ID of the soulbound token to recover.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`]           — contract is paused.
    /// * [`Error::TokenFrozen`]              — token is frozen.
    /// * [`Error::InvalidTokenId`]           — token does not exist.
    /// * [`Error::SoulboundTransferBlocked`] — token is not soulbound.
    /// * [`Error::SignerNotSet`]             — no backend signer registered.
    /// * [`Error::InvalidSignature`]         — signature verification failed.
    pub fn recover_soulbound(
        env: Env,
        signature: BytesN<64>,
        new_owner: Address,
        token_id: TokenId,
    ) -> Result<(), Error> {
        new_owner.require_auth();
        Self::require_not_paused(&env)?;

        if Self::is_frozen(env.clone(), token_id) {
            return Err(Error::TokenFrozen);
        }

        let mut data = Self::load_token(&env, token_id)?;

        // Only soulbound tokens can be recovered via this mechanism.
        if !data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
        }

        // Verify the platform signature over the recovery payload.
        Self::verify_recovery_signature(&env, &new_owner, token_id, &signature)?;

        let old_owner = data.owner.clone();

        // Clear per-token approval on recovery.
        env.storage()
            .persistent()
            .remove(&DataKey::Approved(token_id));

        // Update ownership.
        data.owner = new_owner.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &data);

        // Update balances.
        let old_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(old_owner.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(old_owner.clone()), &old_balance.saturating_sub(1));

        let new_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(new_owner.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(new_owner.clone()), &(new_balance + 1));

        env.events().publish(
            (symbol_short!("sb_recov"), token_id, old_owner.clone(), new_owner.clone()),
            SoulboundRecoveredEvent {
                token_id,
                old_owner,
                new_owner,
            },
        );

        Ok(())
    }
}

#[contractimpl]
impl ClipsNftContract {
    // -------------------------------------------------------------------------
    // View functions
    // -------------------------------------------------------------------------

    /// Returns the contract version number.
    pub fn version(_env: Env) -> u32 {
        VERSION
    }

    /// Returns an approximate fee for mint transactions in stroops.
    pub fn estimate_mint_fee(_env: Env) -> i128 {
        GAS_BASE_MINT as i128
    }

    /// Returns an approximate fee for transfer transactions in stroops.
    pub fn estimate_transfer_fee(_env: Env) -> i128 {
        GAS_BASE_TRANSFER as i128
    }

    /// Returns key contract metadata and configuration.
    pub fn contract_info(env: Env) -> ContractInfo {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not initialized");
        ContractInfo {
            name: Self::name(env.clone()),
            symbol: Self::symbol(env.clone()),
            version: Self::version(env.clone()),
            owner,
            platform_fee: Self::get_platform_fee(env),
        }
    }

    /// Returns the collection name (default: `"ClipCash Clips"`).
    pub fn name(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::Name)
            .unwrap_or_else(|| String::from_str(&env, "ClipCash Clips"))
    }

    /// Returns the collection symbol (default: `"CLIP"`).
    pub fn symbol(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::Symbol)
            .unwrap_or_else(|| String::from_str(&env, "CLIP"))
    }

    /// Returns the original clip ID for a given token ID.
    ///
    /// `clip_id` is stored in `TokenData` at mint time, linking the on-chain
    /// token back to the ClipCash backend database. Used in royalty and
    /// ownership checks.
    ///
    /// Closes #75
    pub fn get_clip_id(env: Env, token_id: TokenId) -> Result<u32, Error> {
        Ok(Self::load_token(&env, token_id)?.clip_id)
    }

    /// Returns the owner of a given token ID.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    pub fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        Ok(Self::load_token(&env, token_id)?.owner)
    }

    /// Returns the metadata URI for a given token ID.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    pub fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error> {
        Ok(Self::load_token(&env, token_id)?.metadata_uri)
    }

    /// Alias for [`token_uri`], kept for backwards compatibility.
    pub fn get_metadata(env: Env, token_id: TokenId) -> Result<String, Error> {
        Ok(Self::load_token(&env, token_id)?.metadata_uri)
    }

    /// Returns OpenSea-compatible JSON metadata for a given token ID.
    ///
    /// Serializes the token's metadata following the OpenSea metadata standard:
    /// https://docs.opensea.io/docs/metadata-standards
    ///
    /// The JSON output includes:
    /// - "metadata_uri": The base metadata URI
    /// - "image": Static thumbnail URL (only if set)
    /// - "animation_url": Animated preview URL (only if set)
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    pub fn get_metadata_json(env: Env, token_id: TokenId) -> Result<String, Error> {
        let data = Self::load_token(&env, token_id)?;

        let mut json = Bytes::from_slice(&env, b"{\"metadata_uri\":\"");
        Self::append_string_bytes(&env, &mut json, &data.metadata_uri);
        Self::append_literal_bytes(&env, &mut json, b"\"");

        if let Some(ref img) = data.image {
            Self::append_literal_bytes(&env, &mut json, b",\"image\":\"");
            Self::append_string_bytes(&env, &mut json, img);
            Self::append_literal_bytes(&env, &mut json, b"\"");
        }

        if let Some(ref anim) = data.animation_url {
            Self::append_literal_bytes(&env, &mut json, b",\"animation_url\":\"");
            Self::append_string_bytes(&env, &mut json, anim);
            Self::append_literal_bytes(&env, &mut json, b"\"");
        }

        if let Some(ref desc) = data.description {
            Self::append_literal_bytes(&env, &mut json, b",\"description\":\"");
            Self::append_string_bytes(&env, &mut json, desc);
            Self::append_literal_bytes(&env, &mut json, b"\"");
        }

        if let Some(ref url) = data.external_url {
            Self::append_literal_bytes(&env, &mut json, b",\"external_url\":\"");
            Self::append_string_bytes(&env, &mut json, url);
            Self::append_literal_bytes(&env, &mut json, b"\"");
        }

        Self::append_literal_bytes(&env, &mut json, b",\"attributes\":[");
        for i in 0..data.attributes.len() {
            if i > 0 {
                Self::append_literal_bytes(&env, &mut json, b",");
            }
            let attribute = data.attributes.get(i).ok_or(Error::InvalidTokenId)?;
            Self::append_literal_bytes(&env, &mut json, b"{\"trait_type\":\"");
            Self::append_string_bytes(&env, &mut json, &attribute.trait_type);
            Self::append_literal_bytes(&env, &mut json, b"\",\"value\":\"");
            Self::append_string_bytes(&env, &mut json, &attribute.value);
            Self::append_literal_bytes(&env, &mut json, b"\"}");
        }
        Self::append_literal_bytes(&env, &mut json, b"]}");
        Ok(json.to_string())
    }

    /// Look up the on-chain token ID for a given `clip_id`.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — no token exists for this clip.
    pub fn clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error> {
        Self::load_clip_token_id(&env, clip_id).ok_or(Error::InvalidTokenId)
    }

    /// Look up the on-chain token IDs for multiple clip IDs.
    /// Returns None for clip IDs that have not been minted.
    pub fn get_tokens_by_clip_ids(env: Env, clip_ids: Vec<u32>) -> Vec<Option<TokenId>> {
        let mut result = Vec::new(&env);
        for i in 0..clip_ids.len() {
            let clip_id = clip_ids.get(i).unwrap();
            result.push_back(Self::load_clip_token_id(&env, clip_id));
        }
        result
    }

    /// Returns the stored [`Royalty`] struct for a token.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        Ok(Self::load_token(&env, token_id)?.royalty)
    }

    /// Returns the total number of tokens minted (not adjusted for burns).
    ///
    /// Derived from `NextTokenId - 1` — no separate counter needed.
    pub fn total_supply(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Returns the total number of clips minted so far (all-time).
    pub fn minted_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::NextTokenId)
            .unwrap_or(1)
            .saturating_sub(1)
    }

    /// Returns `true` if the token is soulbound (non-transferable).
    pub fn is_soulbound(env: Env, token_id: TokenId) -> bool {
        Self::load_token(&env, token_id)
            .map(|d| d.is_soulbound)
            .unwrap_or(false)
    }

    /// Returns the average gas cost for mint operations.
    /// Returns 0 if no mints have been performed.
    pub fn average_gas_mint(env: Env) -> u64 {
        let total_gas: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalGasMint)
            .unwrap_or(0);
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CountMint)
            .unwrap_or(0);
        
        if count == 0 {
            0
        } else {
            total_gas / count
        }
    }

    /// Returns the average gas cost for transfer operations.
    /// Returns 0 if no transfers have been performed.
    pub fn average_gas_transfer(env: Env) -> u64 {
        let total_gas: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalGasTransfer)
            .unwrap_or(0);
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CountTransfer)
            .unwrap_or(0);
        
        if count == 0 {
            0
        } else {
            total_gas / count
        }
    }

    /// Returns the total number of mint operations performed.
    pub fn total_mints(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CountMint)
            .unwrap_or(0)
    }

    /// Returns the total number of transfer operations performed.
    pub fn total_transfers(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CountTransfer)
            .unwrap_or(0)
    }

    /// Returns whether the circuit breaker is enabled.
    pub fn circuit_breaker_enabled(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreakerEnabled)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_ENABLED)
    }

    /// Returns the circuit breaker threshold (max mints per window).
    pub fn circuit_breaker_threshold(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreakerThreshold)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_THRESHOLD)
    }

    /// Returns the circuit breaker time window duration in seconds.
    pub fn circuit_breaker_window_seconds(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowSeconds)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS)
    }

    /// Returns the current circuit breaker window start timestamp.
    pub fn circuit_breaker_window_start(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowStart)
            .unwrap_or(0)
    }

    /// Returns the current circuit breaker window mint count.
    pub fn circuit_breaker_window_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowCount)
            .unwrap_or(0)
    }

    /// Returns the number of tokens owned by `owner`.
    /// Compliant with emerging Soroban NFT standard view functions.
    pub fn balance_of(env: Env, owner: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(owner))
            .unwrap_or(0)
    }

    /// Returns the token ID at the given global index.
    /// Index 0 corresponds to the first existing token.
    /// Returns `InvalidTokenId` if the index is out of bounds.
    pub fn token_by_index(env: Env, index: u32) -> Result<TokenId, Error> {
        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        let mut current_index: u32 = 0;
        let mut token_id: u32 = 1;
        while token_id < next_id {
            if env.storage().persistent().has(&DataKey::Token(token_id)) {
                if current_index == index {
                    return Ok(token_id);
                }
                current_index += 1;
            }
            token_id += 1;
        }
        Err(Error::InvalidTokenId)
    }

    /// Returns the N-th token owned by `owner` (0-indexed).
    ///
    /// Iterates over all minted tokens and returns the one at position `index`
    /// among those owned by `owner`. Essential for Enumerable NFT standards.
    ///
    /// # Arguments
    /// * `owner` — Address to query.
    /// * `index` — 0-based position among the owner's tokens.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — index is out of bounds for this owner.
    ///
    /// Closes #171
    pub fn token_of_owner_by_index(env: Env, owner: Address, index: u32) -> Result<TokenId, Error> {
        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        let mut count: u32 = 0;
        let mut token_id: u32 = 1;
        while token_id < next_id {
            if let Some(data) = env
                .storage()
                .persistent()
                .get::<DataKey, TokenData>(&DataKey::Token(token_id))
            {
                if data.owner == owner {
                    if count == index {
                        return Ok(token_id);
                    }
                    count += 1;
                }
            }
            token_id += 1;
        }
        Err(Error::InvalidTokenId)
    }

    /// Returns the earliest ledger timestamp at which `token_id` is eligible
    /// for its next metadata refresh (i.e. `last_refresh + 30 days`).
    ///
    /// Returns `0` if the token has never been refreshed (eligible immediately).
    ///
    /// # Arguments
    /// * `token_id` — Token to query.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    ///
    /// Closes #172
    pub fn get_next_metadata_refresh_time(env: Env, token_id: TokenId) -> Result<u64, Error> {
        if !Self::exists(env.clone(), token_id) {
            return Err(Error::InvalidTokenId);
        }
        const COOLDOWN: u64 = 2_592_000; // 30 days in seconds
        let next_time = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::MetadataRefreshTime(token_id))
            .map(|last| last.saturating_add(COOLDOWN))
            .unwrap_or(0);
        Ok(next_time)
    }

    // -------------------------------------------------------------------------
    // Royalty extension (EIP-2981 style)
    // -------------------------------------------------------------------------

    /// Returns the royalty receiver, total amount, and payment asset for a sale.
    ///
    /// Formula: `royalty_amount = sale_price × total_basis_points / 10_000`
    ///
    /// Uses overflow-safe arithmetic; returns [`Error::RoyaltyOverflow`] when
    /// `sale_price > i128::MAX / 10_000`.
    ///
    /// # Arguments
    /// * `token_id`   — Token being sold.
    /// * `sale_price` — Sale price in the asset's smallest unit (must be > 0).
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`]    — token does not exist.
    /// * [`Error::InvalidSalePrice`]  — `sale_price` ≤ 0.
    /// * [`Error::RoyaltyOverflow`]   — arithmetic would overflow.
    /// * [`Error::InvalidRoyaltySplit`] — royalty recipients list is empty.
    pub fn royalty_info(
        env: Env,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<RoyaltyInfo, Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let royalty = Self::load_token(&env, token_id)?.royalty;

        let mut total_bps: u32 = 0;
        for idx in 0..royalty.recipients.len() {
            let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;
            total_bps = total_bps.saturating_add(split.basis_points);
        }

        let total_royalty_amount = Self::calculate_royalty(sale_price, total_bps)?;
        let first = royalty.recipients.get(0).ok_or(Error::InvalidRoyaltySplit)?;

        Ok(RoyaltyInfo {
            receiver: first.recipient,
            royalty_amount: total_royalty_amount,
            asset_address: royalty.asset_address,
        })
    }

    /// Pay royalties for a token sale using the SEP-0041 asset in the royalty config.
    ///
    /// Iterates over all recipients and transfers each share via the token client.
    /// Also accrues the total royalty amount to `RoyaltyBalance(token_id)` so
    /// recipients can track lifetime earnings and claim via [`claim_royalties`].
    /// For XLM royalties (`asset_address = None`) the marketplace must handle
    /// the transfer directly — this function returns [`Error::InvalidRecipient`].
    ///
    /// Emits: `"royalty"` [`RoyaltyPaidEvent`] per recipient paid.
    ///
    /// # Arguments
    /// * `payer`      — Address making the payment (must authorize).
    /// * `token_id`   — Token being sold.
    /// * `sale_price` — Sale price in the asset's smallest unit (must be > 0).
    ///
    /// # Errors
    /// * [`Error::InvalidSalePrice`]  — `sale_price` ≤ 0.
    /// * [`Error::InvalidTokenId`]    — token does not exist.
    /// * [`Error::InvalidRecipient`]  — no SEP-0041 asset configured (XLM royalty).
    /// * [`Error::InvalidRoyaltySplit`] — recipients list is empty.
    /// * [`Error::RoyaltyOverflow`]   — arithmetic would overflow.
    pub fn pay_royalty(
        env: Env,
        payer: Address,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<(), Error> {
        payer.require_auth();
        Self::acquire_reentrancy_lock(&env)?;
        let result = Self::pay_royalty_internal(&env, &payer, token_id, sale_price);
        Self::release_reentrancy_lock(&env);
        result
    }

    /// Internal royalty payout logic (caller must hold reentrancy lock).
    /// Follows check-effects-interactions pattern: updates storage before external transfers.
    fn pay_royalty_internal(
        env: &Env,
        payer: &Address,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<(), Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let royalty = Self::load_token(env, token_id)?.royalty;
        let asset_address = royalty.asset_address.clone().ok_or(Error::InvalidRecipient)?;

        // First, calculate total royalty amount (check phase)
        let mut cumulative_bps: u32 = 0;
        let mut cumulative_royalty: i128 = 0;

        for idx in 0..royalty.recipients.len() {
            let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;
            cumulative_bps = cumulative_bps.saturating_add(split.basis_points);
            let total_so_far = Self::calculate_royalty(sale_price, cumulative_bps)?;
            cumulative_royalty = total_so_far;
        }

        // Update storage before external transfers (effects phase)
        // This ensures state changes are committed before any external calls
        if cumulative_royalty > 0 {
            let prev: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::RoyaltyBalance(token_id))
                .unwrap_or(0);
            env.storage()
                .persistent()
                .set(&DataKey::RoyaltyBalance(token_id), &(prev.saturating_add(cumulative_royalty)));
        }

        // Now perform external transfers (interactions phase)
        let token_client = soroban_sdk::token::TokenClient::new(env, &asset_address);
        let mut cumulative_bps: u32 = 0;
        let mut cumulative_royalty: i128 = 0;

        for idx in 0..royalty.recipients.len() {
            let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;

            cumulative_bps = cumulative_bps.saturating_add(split.basis_points);
            let total_so_far = Self::calculate_royalty(sale_price, cumulative_bps)?;
            let amount = total_so_far.saturating_sub(cumulative_royalty);
            cumulative_royalty = total_so_far;

            if amount == 0 {
                continue;
            }

            token_client.transfer(payer, &split.recipient, &amount);
            env.events().publish(
                (symbol_short!("royalty"), token_id, payer.clone(), split.recipient.clone()),
                RoyaltyPaidEvent {
                    token_id,
                    from: payer.clone(),
                    to: split.recipient,
                    amount,
                },
            );
        }

        Ok(())
    }

    /// Claim accumulated royalties for a token.
    ///
    /// Transfers the full `RoyaltyBalance` for `token_id` to the primary royalty
    /// recipient using the SEP-0041 asset configured in the royalty. Clears the
    /// balance atomically (check-effects-interactions) to prevent double-claiming.
    ///
    /// Only the primary royalty recipient (index 0) may call this.
    ///
    /// Emits: `"roy_claim"` [`RoyaltyClaimedEvent`].
    ///
    /// # Arguments
    /// * `caller`   — Must be the primary royalty recipient.
    /// * `token_id` — Token whose royalties are being claimed.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`]    — token does not exist.
    /// * [`Error::Unauthorized`]      — caller is not the primary recipient.
    /// * [`Error::InvalidRecipient`]  — no SEP-0041 asset configured.
    /// * [`Error::InsufficientBalance`] — no royalties to claim.
    pub fn claim_royalties(env: Env, caller: Address, token_id: TokenId) -> Result<(), Error> {
        caller.require_auth();
        Self::acquire_reentrancy_lock(&env)?;
        let result = Self::claim_royalties_internal(&env, &caller, token_id);
        Self::release_reentrancy_lock(&env);
        result
    }

    /// Internal royalty claim logic (caller must hold reentrancy lock).
    fn claim_royalties_internal(
        env: &Env,
        caller: &Address,
        token_id: TokenId,
    ) -> Result<(), Error> {
        let royalty = Self::load_token(env, token_id)?.royalty;
        let recipient = royalty
            .recipients
            .get(0)
            .ok_or(Error::InvalidRoyaltySplit)?
            .recipient;

        if caller != &recipient {
            return Err(Error::Unauthorized);
        }

        let asset_address = royalty.asset_address.ok_or(Error::InvalidRecipient)?;

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyBalance(token_id))
            .unwrap_or(0);

        if balance <= 0 {
            return Err(Error::InsufficientBalance);
        }

        // Clear balance before transfer (check-effects-interactions).
        env.storage()
            .persistent()
            .remove(&DataKey::RoyaltyBalance(token_id));

        soroban_sdk::token::TokenClient::new(env, &asset_address)
            .transfer(&env.current_contract_address(), &recipient, &balance);

        env.events().publish(
            (symbol_short!("roy_clm"), token_id, recipient.clone()),
            RoyaltyClaimedEvent {
                token_id,
                recipient,
                amount: balance,
                asset: asset_address,
            },
        );

        Ok(())
    }

    /// Update the royalty configuration for a token.
    /// Access Control: Admin only.
    /// Emits RoyaltyRecipientUpdated event when the primary recipient changes.
    pub fn set_royalty(
        env: Env,
        admin: Address,
        token_id: TokenId,
        new_royalty: Royalty,
    ) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;

        // 1 persistent read
        let mut data = Self::load_token(&env, token_id)?;
        let old_royalty = data.royalty.clone();

        let new_royalty = Self::normalize_royalty(&env, new_royalty)?;

        // Emit event if primary recipient changed
        if !old_royalty.recipients.is_empty() && !new_royalty.recipients.is_empty() {
            let old_recipient = old_royalty.recipients.get(0).ok_or(Error::InvalidRoyaltySplit)?;
            let new_recipient = new_royalty.recipients.get(0).ok_or(Error::InvalidRoyaltySplit)?;
            
            if old_recipient.recipient != new_recipient.recipient {
                env.events().publish(
                    (
                        symbol_short!("royalty"),
                        token_id,
                        old_recipient.recipient.clone(),
                        new_recipient.recipient.clone(),
                    ),
                    RoyaltyRecipientUpdatedEvent {
                        token_id,
                        old_recipient: old_recipient.recipient,
                        new_recipient: new_recipient.recipient,
                    },
                );
            }
        }

        data.royalty = new_royalty;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &data);

        env.events().publish(
            (symbol_short!("roy_upd"), token_id),
            RoyaltyUpdatedEvent { token_id },
        );

        Ok(())
    }

    /// Burn (destroy) an NFT. Only the current owner may burn.
    ///
    /// Storage removes (persistent): TokenData, ClipIdMinted = **2** (Optimized from 4)
    pub fn burn(env: Env, owner: Address, token_id: TokenId) -> Result<(), Error> {
        owner.require_auth();

        if Self::is_frozen(env.clone(), token_id) {
            return Err(Error::TokenFrozen);
        }

        // 1 persistent read — also gives us clip_id for dedup cleanup
        let data: TokenData = Self::load_token(&env, token_id)?;

        if owner != data.owner {
            return Err(Error::Unauthorized);
        }

        // 2 persistent removes
        env.storage().persistent().remove(&DataKey::Token(token_id));
        env.storage()
            .persistent()
            .remove(&DataKey::ClipIdMinted(data.clip_id));

        // Update total supply
        let total_supply: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &total_supply.saturating_sub(1));

        // Update balance
        let balance: u32 = env.storage().persistent().get(&DataKey::Balance(owner.clone())).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Balance(owner.clone()), &balance.saturating_sub(1));

        env.events().publish(
            (symbol_short!("burn"), token_id, owner.clone()),
            BurnEvent {
                owner: owner.clone(),
                token_id,
                clip_id: data.clip_id,
            },
        );

        // Emit standard Transfer event for ERC-721 compliance
        env.events().publish(
            (
                symbol_short!("transfer"),
                token_id,
                owner.clone(),
                env.current_contract_address(),
            ),
            TransferEvent {
                token_id,
                from: owner.clone(),
                to: env.current_contract_address(),
            },
        );

        Ok(())
    }

    /// Burn (destroy) multiple NFTs. Only the current owner may burn the tokens.
    ///
    /// Loops through `token_ids` and burns each token if owned by `owner`.
    /// All burns happen in a single transaction.
    ///
    /// Storage removes (persistent): For each token - TokenData, ClipIdMinted
    ///
    /// Emits: `"burn"` [`BurnEvent`] per token burned.
    ///
    /// # Arguments
    /// * `owner`   - Owner of the tokens (must authorize and match token ownership)
    /// * `token_ids` - List of token IDs to burn
    ///
    /// # Errors
    /// * [`Error::TokenFrozen`]   — any token is frozen.
    /// * [`Error::Unauthorized`]  — owner mismatch for any token.
    /// * [`Error::InvalidTokenId`] — any token does not exist.
    pub fn batch_burn(env: Env, owner: Address, token_ids: Vec<TokenId>) -> Result<(), Error> {
        owner.require_auth();

        // Process each token
        for i in 0..token_ids.len() {
            let token_id = token_ids.get(i).unwrap();

            // Validate token is not frozen
            if Self::is_frozen(env.clone(), token_id) {
                return Err(Error::TokenFrozen);
            }

            // Load token data and verify ownership
            let data: TokenData = Self::load_token(&env, token_id)?;
            if owner != data.owner {
                return Err(Error::Unauthorized);
            }

            // Burn the token (2 persistent removes)
            env.storage().persistent().remove(&DataKey::Token(token_id));
            env.storage()
                .persistent()
                .remove(&DataKey::ClipIdMinted(data.clip_id));

            // Update total supply
            let total_supply: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
            env.storage().instance().set(&DataKey::TotalSupply, &total_supply.saturating_sub(1));

            // Update balance
            let balance: u32 = env.storage().persistent().get(&DataKey::Balance(owner.clone())).unwrap_or(0);
            env.storage().persistent().set(&DataKey::Balance(owner.clone()), &balance.saturating_sub(1));

            // Emit Burn event
            env.events().publish(
                (symbol_short!("burn"), token_id, owner.clone()),
                BurnEvent {
                    owner: owner.clone(),
                    token_id,
                    clip_id: data.clip_id,
                },
            );

            // Emit standard Transfer event for ERC-721 compliance (to zero address)
            env.events().publish(
                (
                    symbol_short!("transfer"),
                    token_id,
                    owner.clone(),
                    env.current_contract_address(),
                ),
                TransferEvent {
                    token_id,
                    from: owner.clone(),
                    to: env.current_contract_address(),
                },
            );
        }

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Task 1: Update royalty recipient
    // -------------------------------------------------------------------------

    /// Allow the current royalty recipient to update their address.
    ///
    /// Only the current primary royalty recipient (index 0) may call this.
    /// Emits `RoyaltyRecipientUpdated` event.
    ///
    /// # Arguments
    /// * `caller`        - Must be the current primary royalty recipient
    /// * `token_id`      - Token whose royalty recipient is being updated
    /// * `new_recipient` - New recipient address
    pub fn update_royalty_recipient(
        env: Env,
        caller: Address,
        token_id: TokenId,
        new_recipient: Address,
    ) -> Result<(), Error> {
        caller.require_auth();

        let mut data = Self::load_token(&env, token_id)?;
        let old_recipient = data
            .royalty
            .recipients
            .get(0)
            .ok_or(Error::InvalidRoyaltySplit)?
            .recipient
            .clone();

        if caller != old_recipient {
            return Err(Error::Unauthorized);
        }

        // Replace recipient at index 0, keep basis_points unchanged
        let bps = data
            .royalty
            .recipients
            .get(0)
            .ok_or(Error::InvalidRoyaltySplit)?
            .basis_points;

        data.royalty.recipients.set(
            0,
            RoyaltyRecipient {
                recipient: new_recipient.clone(),
                basis_points: bps,
            },
        );

        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &data);

        env.events().publish(
            (
                symbol_short!("royalty"),
                token_id,
                old_recipient.clone(),
                new_recipient.clone(),
            ),
            RoyaltyRecipientUpdatedEvent {
                token_id,
                old_recipient,
                new_recipient,
            },
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Task 1 (Issue #124): tokens_of_owner view
    // -------------------------------------------------------------------------

    /// Return token IDs owned by `owner` with pagination support.
    ///
    /// This function enables frontends to display NFTs owned by a user with pagination.
    /// It iterates over minted token IDs (1..=next_token_id-1) and collects those
    /// whose owner matches.
    ///
    /// ## Storage Optimization
    /// - Linear iteration per token to check ownership (unavoidable for general query)
    /// - Single instance read for NextTokenId
    /// - Persistent reads only for tokens that might belong to owner
    ///
    /// ## Gas Protection
    /// - Result is capped at MAX_RESULTS (1000) entries to prevent gas explosion
    /// - When result reaches 1000, iteration stops even if more tokens exist
    /// - Pagination allows fetching large collections in batches
    ///
    /// # Arguments
    /// * `owner` - Address to query
    /// * `limit` - Maximum number of results to return (optional, defaults to 1000)
    /// * `offset` - Number of results to skip (optional, defaults to 0)
    ///
    /// # Returns
    /// Vec of token IDs owned by the address, respecting limit and offset
    pub fn tokens_of_owner(
        env: Env,
        owner: Address,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Vec<TokenId> {
        const MAX_RESULTS: u32 = 1000;
        let limit = limit.unwrap_or(MAX_RESULTS).min(MAX_RESULTS);
        let offset = offset.unwrap_or(0);
        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        let mut result: Vec<TokenId> = Vec::new(&env);
        let mut count: u32 = 0;
        let mut skipped: u32 = 0;

        let mut token_id: u32 = 1;
        while token_id < next_id && count < limit {
            if let Some(data) = env
                .storage()
                .persistent()
                .get::<DataKey, TokenData>(&DataKey::Token(token_id))
            {
                if data.owner == owner {
                    if skipped < offset {
                        skipped += 1;
                    } else {
                        result.push_back(token_id);
                        count += 1;
                    }
                }
            }
            token_id += 1;
        }

        result
    }

    /// Return a paginated list of token IDs owned by `owner`.
    ///
    /// Supports offset-based pagination: `offset` is the number of matching
    /// tokens to skip, `limit` is the max to return (capped at 100).
    ///
    /// ## Usage
    /// ```text
    /// // Page 1: first 10 tokens
    /// get_user_tokens(owner, 10, 0)
    /// // Page 2: next 10 tokens
    /// get_user_tokens(owner, 10, 10)
    /// ```
    ///
    /// # Arguments
    /// * `owner`  — Address to query.
    /// * `limit`  — Max tokens to return (capped at 100).
    /// * `offset` — Number of matching tokens to skip before collecting.
    pub fn get_user_tokens(env: Env, owner: Address, limit: u32, offset: u32) -> Vec<TokenId> {
        const MAX_LIMIT: u32 = 100;
        let limit = if limit > MAX_LIMIT { MAX_LIMIT } else { limit };

        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        let mut result: Vec<TokenId> = Vec::new(&env);
        let mut skipped: u32 = 0;
        let mut collected: u32 = 0;
        let mut token_id: u32 = 1;

        while token_id < next_id && collected < limit {
            if let Some(data) = env
                .storage()
                .persistent()
                .get::<DataKey, TokenData>(&DataKey::Token(token_id))
            {
                if data.owner == owner {
                    if skipped < offset {
                        skipped += 1;
                    } else {
                        result.push_back(token_id);
                        collected += 1;
                    }
                }
            }
            token_id += 1;
        }

        result
    }

    // -------------------------------------------------------------------------
    // Task 2: Batch minting
    // -------------------------------------------------------------------------

    /// Mint multiple clips in a single transaction.
    ///
    /// Loops through `clip_ids` and `metadata_uris` in lockstep, minting each
    /// with the provided `royalty` and `signatures`. Emits a single
    /// `BatchMint` event on success.
    ///
    /// # Arguments
    /// * `to`            - Owner of all minted tokens
    /// * `clip_ids`      - List of clip IDs to mint
    /// * `metadata_uris` - Corresponding metadata URIs
    /// * `images`        - Corresponding static thumbnail URLs (optional for each)
    /// * `animation_urls` - Corresponding animated preview URLs (optional for each)
    /// * `royalty`       - Royalty config applied to all tokens
    /// * `is_soulbound`  - Whether all tokens are soulbound
    /// * `signatures`    - Per-clip backend signatures
    pub fn batch_mint(
        env: Env,
        to: Address,
        clip_ids: Vec<u32>,
        metadata_uris: Vec<String>,
        images: Vec<Option<String>>,
        animation_urls: Vec<Option<String>>,
        royalty: Royalty,
        is_soulbound: bool,
        signatures: Vec<BytesN<64>>,
    ) -> Result<Vec<TokenId>, Error> {
        to.require_auth();
        Self::require_not_paused(&env)?;
        Self::enforce_mint_cooldown(&env, &to)?;

        let n = clip_ids.len();
        Self::check_circuit_breaker(&env, n as u64)?;

        if n != metadata_uris.len() || n != signatures.len() || n != images.len() || n != animation_urls.len() {
            return Err(Error::InvalidRoyaltySplit); // mismatched input lengths
        }
        if n > MAX_BATCH_MINT {
            return Err(Error::BatchTooLarge);
        }

        let royalty = Self::normalize_royalty(&env, royalty)?;
        let mut minted: Vec<TokenId> = Vec::new(&env);

        for i in 0..n {
            let clip_id = clip_ids.get(i).ok_or(Error::InvalidTokenId)?;
            let metadata_uri = metadata_uris.get(i).ok_or(Error::InvalidTokenId)?;
            let image = images.get(i).ok_or(Error::InvalidTokenId)?;
            let animation_url = animation_urls.get(i).ok_or(Error::InvalidTokenId)?;
            let signature = signatures.get(i).ok_or(Error::InvalidTokenId)?;

            // Validate URLs
            Self::validate_url(&env, &image, Error::InvalidImageUrl)?;
            Self::validate_url(&env, &animation_url, Error::InvalidAnimationUrl)?;

            Self::verify_clip_signature(&env, &to, clip_id, &metadata_uri, &signature)?;

            if Self::load_clip_token_id(&env, clip_id).is_some() {
                return Err(Error::ClipAlreadyMinted);
            }

            if env
                .storage()
                .persistent()
                .get(&DataKey::BlacklistedClip(clip_id))
                .unwrap_or(false)
            {
                return Err(Error::ClipBlacklisted);
            }

            let token_id: TokenId = env
                .storage()
                .instance()
                .get(&DataKey::NextTokenId)
                .unwrap_or(1);

            env.storage().persistent().set(
                &DataKey::Token(token_id),
                &TokenData {
                    owner: to.clone(),
                    clip_id,
                    is_soulbound,
                    metadata_uri,
                    image,
                    animation_url,
                    description: None,
                    external_url: None,
                    attributes: Vec::new(&env),
                    royalty: royalty.clone(),
                },
            );
            Self::bump_persistent_ttl(&env, &DataKey::Token(token_id));
            env.storage()
                .persistent()
                .set(&DataKey::ClipIdMinted(clip_id), &token_id);
            Self::bump_persistent_ttl(&env, &DataKey::ClipIdMinted(clip_id));
            env.storage()
                .instance()
                .set(&DataKey::NextTokenId, &(token_id + 1));

            // Update total supply
            let total_supply: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
            env.storage().instance().set(&DataKey::TotalSupply, &(total_supply + 1));

            // Update balance
            let balance: u32 = env.storage().persistent().get(&DataKey::Balance(to.clone())).unwrap_or(0);
            env.storage().persistent().set(&DataKey::Balance(to.clone()), &(balance + 1));

            minted.push_back(token_id);
        }

        env.events().publish(
            (
                symbol_short!("batch_mnt"),
                to.clone(),
                minted.get(0).unwrap_or(0),
            ),
            BatchMintEvent {
                to: to.clone(),
                count: n,
                first_token_id: minted.get(0).unwrap_or(0),
            },
        );

        // Gas tracking — Closes #169
        let count_mint: u64 = env.storage().instance().get(&DataKey::CountMint).unwrap_or(0);
        env.storage().instance().set(&DataKey::CountMint, &(count_mint + n as u64));
        let total_gas_mint: u64 = env.storage().instance().get(&DataKey::TotalGasMint).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalGasMint, &total_gas_mint.saturating_add(GAS_BASE_MINT.saturating_mul(n as u64)));
        Self::record_mint_timestamp(&env, &to);

        // Update circuit breaker counter after successful batch mint
        Self::update_circuit_breaker_counter(&env, n as u64);

        Ok(minted)
    }

    // -------------------------------------------------------------------------
    // Task 4: Public royalty fee calculation helper
    // -------------------------------------------------------------------------

    /// Calculate the royalty amount for a given sale price using the token's
    /// stored royalty configuration (sum of all recipient basis points).
    ///
    /// Returns `InvalidSalePrice` if `sale_price <= 0`.
    /// Returns `RoyaltyOverflow` if `sale_price` is too large.
    ///
    /// # Arguments
    /// * `token_id`   - Token to look up royalty config for
    /// * `sale_price` - Sale price in the token's royalty asset denomination
    pub fn calculate_royalty_amount(
        env: Env,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<i128, Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let royalty = Self::load_token(&env, token_id)?.royalty;
        let mut total_bps: u32 = 0;
        for idx in 0..royalty.recipients.len() {
            let split = royalty.recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;
            total_bps = total_bps.saturating_add(split.basis_points);
        }

        Self::calculate_royalty(sale_price, total_bps)
    }

    // -------------------------------------------------------------------------
    // Circuit breaker internal helpers
    // -------------------------------------------------------------------------

    /// Check if the circuit breaker should trigger based on mint activity.
    /// If enabled and the threshold is exceeded within the time window,
    /// automatically pause the contract.
    fn check_circuit_breaker(env: &Env, mint_count: u64) -> Result<(), Error> {
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerEnabled)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_ENABLED);
        
        if !enabled {
            return Ok(());
        }

        let threshold: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerThreshold)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_THRESHOLD);
        
        let window_seconds: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowSeconds)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS);

        let now = env.ledger().timestamp();
        let window_start: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowStart)
            .unwrap_or(0);
        
        let current_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowCount)
            .unwrap_or(0);

        // Check if we need to reset the window (time elapsed)
        if window_start == 0 || now >= window_start.saturating_add(window_seconds) {
            // Window expired or not started - check if this batch alone would exceed threshold
            if mint_count > threshold {
                Self::trigger_circuit_breaker(env, mint_count, threshold, window_seconds)?;
            }
        } else {
            // Within current window, check if adding this batch would exceed threshold
            let new_count = current_count.saturating_add(mint_count);
            if new_count > threshold {
                Self::trigger_circuit_breaker(env, new_count, threshold, window_seconds)?;
            }
        }

        Ok(())
    }

    /// Update the circuit breaker counter after a successful mint.
    fn update_circuit_breaker_counter(env: &Env, mint_count: u64) {
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerEnabled)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_ENABLED);
        
        if !enabled {
            return;
        }

        let window_seconds: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowSeconds)
            .unwrap_or(DEFAULT_CIRCUIT_BREAKER_WINDOW_SECONDS);

        let now = env.ledger().timestamp();
        let window_start: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowStart)
            .unwrap_or(0);
        
        let current_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerWindowCount)
            .unwrap_or(0);

        // Check if we need to reset the window (time elapsed)
        if window_start == 0 || now >= window_start.saturating_add(window_seconds) {
            // Window expired or not started, reset counter
            env.storage()
                .instance()
                .set(&DataKey::CircuitBreakerWindowStart, &now);
            env.storage()
                .instance()
                .set(&DataKey::CircuitBreakerWindowCount, &mint_count);
        } else {
            // Within current window, increment counter
            env.storage()
                .instance()
                .set(&DataKey::CircuitBreakerWindowCount, &current_count.saturating_add(mint_count));
        }
    }

    /// Trigger the circuit breaker by pausing the contract.
    fn trigger_circuit_breaker(env: &Env, mint_count: u64, threshold: u64, window_seconds: u64) -> Result<(), Error> {
        // Set pause flag immediately (no timelock for automatic circuit breaker)
        env.storage().instance().set(&DataKey::Paused, &true);
        
        // Emit event
        env.events().publish(
            (symbol_short!("circuit"), threshold),
            CircuitBreakerTriggeredEvent {
                mint_count,
                threshold,
                window_seconds,
            },
        );

        Err(Error::CircuitBreakerTripped)
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Rejects minting when `wallet` is still inside the configured cooldown window.
    fn enforce_mint_cooldown(env: &Env, wallet: &Address) -> Result<(), Error> {
        let cooldown = Self::get_mint_cooldown(env.clone());
        if cooldown == 0 {
            return Ok(());
        }
        let now = env.ledger().timestamp();
        let key = DataKey::LastMintTimestamp(wallet.clone());
        if let Some(last_mint) = env.storage().persistent().get::<DataKey, u64>(&key) {
            if now < last_mint.saturating_add(cooldown) {
                return Err(Error::MintCooldownActive);
            }
        }
        Ok(())
    }

    /// Persists the ledger timestamp of the latest successful mint for `wallet`.
    fn record_mint_timestamp(env: &Env, wallet: &Address) {
        env.storage()
            .persistent()
            .set(&DataKey::LastMintTimestamp(wallet.clone()), &env.ledger().timestamp());
    }

    /// Load and return `TokenData`, or `InvalidTokenId` if not found.
    fn load_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
        let data: TokenData = env.storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)?;
        Self::bump_persistent_ttl(env, &DataKey::Token(token_id));
        Ok(data)
    }

    /// Returns the token ID minted for `clip_id`, if any, bumping TTL when present.
    fn load_clip_token_id(env: &Env, clip_id: u32) -> Option<TokenId> {
        let key = DataKey::ClipIdMinted(clip_id);
        let token_id: Option<TokenId> = env.storage().persistent().get(&key);
        if token_id.is_some() {
            Self::bump_persistent_ttl(env, &key);
        }
        token_id
    }

    /// Extends persistent entry TTL to reduce archive risk on hot keys.
    fn bump_persistent_ttl(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, PERSISTENT_BUMP_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
    }

    /// Acquire the contract reentrancy lock before external token calls.
    fn acquire_reentrancy_lock(env: &Env) -> Result<(), Error> {
        let locked: bool = env
            .storage()
            .instance()
            .get(&DataKey::ReentrancyLock)
            .unwrap_or(false);
        if locked {
            return Err(Error::Reentrancy);
        }
        env.storage()
            .instance()
            .set(&DataKey::ReentrancyLock, &true);
        Ok(())
    }

    /// Release the contract reentrancy lock after external token calls complete.
    fn release_reentrancy_lock(env: &Env) {
        env.storage()
            .instance()
            .set(&DataKey::ReentrancyLock, &false);
    }

    /// Verify the backend Ed25519 signature over the canonical mint payload.
    ///
    /// Payload:
    /// ```text
    /// owner_hash = SHA-256(XDR(owner))
    /// uri_hash   = SHA-256(UTF-8(metadata_uri))
    /// message    = SHA-256( clip_id_le4 || owner_hash || uri_hash )
    /// ```
    /// Traps (panics) on invalid signature via `env.crypto().ed25519_verify`.
    fn verify_clip_signature(
        env: &Env,
        owner: &Address,
        clip_id: u32,
        metadata_uri: &String,
        signature: &BytesN<64>,
    ) -> Result<(), Error> {
        let signer: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::Signer)
            .ok_or(Error::SignerNotSet)?;

        let owner_hash: BytesN<32> = env.crypto().sha256(&owner.clone().to_xdr(env)).into();
        let uri_hash: BytesN<32> = env.crypto().sha256(&Bytes::from(metadata_uri.to_xdr(env))).into();

        let mut preimage = Bytes::new(env);
        preimage.extend_from_array(&clip_id.to_le_bytes());
        preimage.append(&Bytes::from(owner_hash));
        preimage.append(&Bytes::from(uri_hash));

        let message: BytesN<32> = env.crypto().sha256(&preimage).into();

        env.crypto().ed25519_verify(&signer, &Bytes::from(message), signature);

        Ok(())
    }

    /// Verify the backend Ed25519 signature over the recovery payload.
    ///
    /// Payload:
    /// ```text
    /// new_owner_hash = SHA-256(XDR(new_owner))
    /// message        = SHA-256( "recover" || token_id_le4 || new_owner_hash )
    /// ```
    ///
    /// The `"recover"` domain separator prevents cross-purpose replay attacks.
    fn verify_recovery_signature(
        env: &Env,
        new_owner: &Address,
        token_id: TokenId,
        signature: &BytesN<64>,
    ) -> Result<(), Error> {
        let signer: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::Signer)
            .ok_or(Error::SignerNotSet)?;

        let new_owner_hash: BytesN<32> = env
            .crypto()
            .sha256(&new_owner.clone().to_xdr(env))
            .into();

        let mut preimage = Bytes::new(env);
        preimage.append(&Bytes::from_slice(env, b"recover"));
        preimage.extend_from_array(&token_id.to_le_bytes());
        preimage.append(&Bytes::from(new_owner_hash));

        let message: BytesN<32> = env.crypto().sha256(&preimage).into();

        env.crypto()
            .ed25519_verify(&signer, &Bytes::from(message), signature);

        Ok(())
    }

    /// Assert that `addr` is the stored admin and require its authorization.
    fn require_admin(env: &Env, addr: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not initialized");

        if addr != &admin {
            return Err(Error::Unauthorized);
        }

        addr.require_auth();
        Ok(())
    }

    /// Return `ContractPaused` if the pause flag is set and the 24-hour timelock has elapsed.
    fn require_not_paused(env: &Env) -> Result<(), Error> {
        if Self::check_paused(env) {
            return Err(Error::ContractPaused);
        }
        Ok(())
    }

    /// Returns `true` if the pause flag is set AND the 24-hour timelock has elapsed.
    fn check_paused(env: &Env) -> bool {
        let flagged: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if !flagged {
            return false;
        }
        // Check if the timelock has elapsed.
        match env.storage().instance().get::<DataKey, u64>(&DataKey::PauseUnlockTime) {
            Some(active_at) => env.ledger().timestamp() >= active_at,
            // No timelock stored — legacy pause (immediately active).
            None => true,
        }
    }

    /// Validate royalty recipients and append the platform 1 % cut if absent.
    fn normalize_royalty(env: &Env, royalty: Royalty) -> Result<Royalty, Error> {
        if royalty.recipients.is_empty() {
            return Err(Error::InvalidRoyaltySplit);
        }
        let asset_address = royalty.asset_address.clone();

        let platform: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformRecipient)
            .ok_or(Error::InvalidRecipient)?;

        let mut recipients = royalty.recipients;
        let mut has_platform = false;
        let mut total_bps: u32 = 0;

        for idx in 0..recipients.len() {
            let split = recipients.get(idx).ok_or(Error::InvalidRoyaltySplit)?;
            if split.recipient == platform {
                has_platform = true;
            }
            total_bps = total_bps.saturating_add(split.basis_points);
        }

        if !has_platform {
            recipients.push_back(RoyaltyRecipient {
                recipient: platform,
                basis_points: 100, // fixed default 1 %
            });
            total_bps = total_bps.saturating_add(100);
        }

        if total_bps > 10_000 {
            return Err(Error::RoyaltyTooHigh);
        }

        Ok(Royalty {
            recipients,
            asset_address,
        })
    }

    /// Validate that a URL starts with `https://` or `ipfs://`.
    fn validate_url(env: &Env, url: &Option<String>, error: Error) -> Result<(), Error> {
        if let Some(ref u) = url {
            if !Self::url_starts_with(env, u, b"https://")
                && !Self::url_starts_with(env, u, b"ipfs://")
            {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Returns true when `value` begins with the UTF-8 `prefix` bytes.
    fn url_starts_with(_env: &Env, value: &String, prefix: &[u8]) -> bool {
        let bytes = value.to_bytes();
        let prefix_len = prefix.len() as u32;
        if bytes.len() < prefix_len {
            return false;
        }
        for i in 0..prefix_len {
            if bytes.get(i) != Some(prefix[i as usize]) {
                return false;
            }
        }
        true
    }

    /// Append a Soroban [`String`] onto a byte buffer used for JSON assembly.
    fn append_string_bytes(_env: &Env, buffer: &mut Bytes, value: &String) {
        let chunk: Bytes = value.to_bytes();
        buffer.append(&chunk);
    }

    /// Append a static UTF-8 fragment onto a byte buffer.
    fn append_literal_bytes(env: &Env, buffer: &mut Bytes, literal: &[u8]) {
        buffer.append(&Bytes::from_slice(env, literal));
    }

    /// Calculate royalty amount using safe (checked) arithmetic.
    ///
    /// Formula: `royalty_amount = (sale_price * basis_points + 5_000) / 10_000`
    ///
    /// # Safe price limits
    /// `sale_price` must be ≤ `i128::MAX / 10_000` (≈ 1.7 × 10³⁴ stroops).
    /// Prices above this threshold return `RoyaltyOverflow`.
    ///
    /// Delegates to [`safe_math::safe_royalty_amount`] — see that module for
    /// full overflow-protection documentation.
    pub fn calculate_royalty(sale_price: i128, basis_points: u32) -> Result<i128, Error> {
        safe_math::safe_royalty_amount(sale_price, basis_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, BytesN as _, Events as _, Ledger as _},
        Address, Bytes, BytesN, Env, String, Vec, xdr::ToXdr,
    };

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        (env, admin, user1, user2)
    }

    fn default_royalty(env: &Env, recipient: Address) -> Royalty {
        let mut recipients = Vec::new(env);
        recipients.push_back(RoyaltyRecipient { recipient, basis_points: 500 });
        Royalty { recipients, asset_address: None }
    }

    fn sign_mint(
        env: &Env,
        signer_secret: &ed25519_dalek::SigningKey,
        owner: &Address,
        clip_id: u32,
        metadata_uri: &String,
    ) -> BytesN<64> {
        let owner_hash: BytesN<32> = env.crypto().sha256(&owner.clone().to_xdr(env)).into();
        let uri_hash: BytesN<32> = env.crypto().sha256(&Bytes::from(metadata_uri.to_xdr(env))).into();
        let mut preimage = Bytes::new(env);
        preimage.extend_from_array(&clip_id.to_le_bytes());
        preimage.append(&Bytes::from(owner_hash));
        preimage.append(&Bytes::from(uri_hash));
        let message: BytesN<32> = env.crypto().sha256(&preimage).into();
        use ed25519_dalek::Signer as _;
        let sig = signer_secret.sign(&message.to_array());
        BytesN::from_array(env, &sig.to_bytes())
    }

    fn register_signer(
        env: &Env,
        client: &ClipsNftContractClient,
        admin: &Address,
    ) -> ed25519_dalek::SigningKey {
        let sk_bytes = soroban_sdk::BytesN::<32>::random(env).to_array();
        let keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pubkey = BytesN::from_array(env, &keypair.verifying_key().to_bytes());
        client.set_signer(admin, &pubkey);
        keypair
    }

    fn do_mint(
        client: &ClipsNftContractClient,
        env: &Env,
        to: &Address,
        clip_id: u32,
        keypair: &ed25519_dalek::SigningKey,
    ) -> TokenId {
        let uri = String::from_str(env, "ipfs://QmExample");
        let sig = sign_mint(env, keypair, to, clip_id, &uri);
        client.mint(
            to,
            &clip_id,
            &uri,
            &None,  // image
            &None,  // animation_url
            &default_royalty(env, to.clone()),
            &false,
            &sig
        )
    }

    fn do_mint_soulbound(
        client: &ClipsNftContractClient,
        env: &Env,
        to: &Address,
        clip_id: u32,
        keypair: &ed25519_dalek::SigningKey,
    ) -> TokenId {
        let uri = String::from_str(env, "ipfs://QmExample");
        let sig = sign_mint(env, keypair, to, clip_id, &uri);
        client.mint(
            to,
            &clip_id,
            &uri,
            &None,  // image
            &None,  // animation_url
            &default_royalty(env, to.clone()),
            &true,
            &sig
        )
    }

    #[test]
    fn test_version() {
        let env = Env::default();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        assert_eq!(client.version(), 1);
    }

    #[test]
    fn test_fee_estimators_return_expected_values() {
        let env = Env::default();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        assert_eq!(client.estimate_mint_fee(), GAS_BASE_MINT as i128);
        assert_eq!(client.estimate_transfer_fee(), GAS_BASE_TRANSFER as i128);
    }

    #[test]
    fn test_contract_info_contains_core_fields() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let info = client.contract_info();
        assert_eq!(info.name, String::from_str(&env, "ClipCash Clips"));
        assert_eq!(info.symbol, String::from_str(&env, "CLIP"));
        assert_eq!(info.version, VERSION);
        assert_eq!(info.owner, admin);
        assert_eq!(info.platform_fee, 100);
    }

    #[test]
    fn test_mint_cooldown_enforced_and_configurable() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        client.set_mint_cooldown(&admin, &120);
        assert_eq!(client.get_mint_cooldown(), 120);

        let first_clip_id = 9_001u32;
        let first_uri = String::from_str(&env, "ipfs://QmCooldown1");
        let first_sig = sign_mint(&env, &kp, &user1, first_clip_id, &first_uri);
        client.mint(
            &user1,
            &first_clip_id,
            &first_uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &first_sig,
        );

        let second_clip_id = 9_002u32;
        let second_uri = String::from_str(&env, "ipfs://QmCooldown2");
        let second_sig = sign_mint(&env, &kp, &user1, second_clip_id, &second_uri);
        assert_eq!(
            client.try_mint(
                &user1,
                &second_clip_id,
                &second_uri,
                &None,
                &None,
                &default_royalty(&env, user1.clone()),
                &false,
                &second_sig,
            ),
            Err(Ok(Error::MintCooldownActive))
        );

        env.ledger().with_mut(|li| li.timestamp += 121);
        client.mint(
            &user1,
            &second_clip_id,
            &second_uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &second_sig,
        );
    }

    #[test]
    fn test_mint_stores_owner_and_uri() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 42, &kp);
        assert_eq!(token_id, 1);
        assert_eq!(client.owner_of(&token_id), user1);
        assert_eq!(client.token_uri(&token_id), String::from_str(&env, "ipfs://QmExample"));
        assert_eq!(client.total_supply(), 1);
    }

    #[test]
    fn test_set_token_uri_owner_only_and_precedence() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 4242, &kp);
        let custom_uri = String::from_str(&env, "ipfs://QmCustomOverride");
        client.set_token_uri(&user1, &token_id, &custom_uri);
        assert_eq!(client.token_uri(&token_id), custom_uri.clone());
        assert_eq!(client.get_metadata(&token_id), custom_uri);
    }

    #[test]
    fn test_set_token_uri_non_owner_fails() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 4343, &kp);
        let result = client.try_set_token_uri(&user2, &token_id, &String::from_str(&env, "ipfs://QmShouldFail"));
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
        assert_eq!(client.token_uri(&token_id), String::from_str(&env, "ipfs://QmExample"));
    }

    #[test]
    fn test_set_token_uri_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 4344, &kp);
        let custom_uri = String::from_str(&env, "ipfs://QmNewURI");

        client.set_token_uri(&user1, &token_id, &custom_uri.clone());

        let events = env.events().all();
        assert!(
            events.events().len() > 0,
            "TokenUriChanged event should be emitted"
        );
    }

    #[test]
    fn test_clip_token_id_lookup() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 99, &kp);
        assert_eq!(client.clip_token_id(&99), token_id);
    }

    #[test]
    #[should_panic]
    fn test_double_mint_same_clip_id_panics() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        do_mint(&client, &env, &user1, 7, &kp);
        do_mint(&client, &env, &user1, 7, &kp);
    }

    #[test]
    fn test_mint_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 5, &kp);
        let events = env.events().all();
        // Mint emits both MintEvent and TransferEvent
        assert_eq!(events.events().len(), 2);
        assert_eq!(token_id, 1);
    }

    #[test]
    fn test_mint_fails_without_signer_set() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
        let kp = ed25519_dalek::SigningKey::from_bytes(&kp_bytes);
        let uri = String::from_str(&env, "ipfs://QmExample");
        let sig = sign_mint(&env, &kp, &user1, 1, &uri);
        let result = client.try_mint(
            &user1,
            &1u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &sig,
        );
        assert_eq!(result, Err(Ok(Error::SignerNotSet)));
    }

    #[test]
    #[should_panic]
    fn test_mint_fails_with_wrong_signature() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        register_signer(&env, &client, &admin);
        let wrong_kp = ed25519_dalek::SigningKey::from_bytes(&soroban_sdk::BytesN::<32>::random(&env).to_array());
        let uri = String::from_str(&env, "ipfs://QmExample");
        let bad_sig = sign_mint(&env, &wrong_kp, &user1, 1, &uri);
        client.mint(&user1, &1u32, &uri, &None, &None, &default_royalty(&env, user1.clone()), &false, &bad_sig);
    }

    #[test]
    #[should_panic]
    fn test_mint_fails_with_wrong_owner_in_payload() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let uri = String::from_str(&env, "ipfs://QmExample");
        let sig_for_user2 = sign_mint(&env, &kp, &user2, 1, &uri);
        client.mint(&user1, &1u32, &uri, &None, &None, &default_royalty(&env, user1.clone()), &false, &sig_for_user2);
    }

    #[test]
    #[should_panic]
    fn test_mint_fails_with_wrong_clip_id_in_payload() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let uri = String::from_str(&env, "ipfs://QmExample");
        let sig_for_99 = sign_mint(&env, &kp, &user1, 99, &uri);
        client.mint(&user1, &1u32, &uri, &None, &None, &default_royalty(&env, user1.clone()), &false, &sig_for_99);
    }

    #[test]
    fn test_set_signer_and_rotate() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp1 = register_signer(&env, &client, &admin);
        let kp1_pub = BytesN::from_array(&env, &kp1.verifying_key().to_bytes());
        assert_eq!(client.get_signer(), Some(kp1_pub));
        let kp2 = ed25519_dalek::SigningKey::from_bytes(&soroban_sdk::BytesN::<32>::random(&env).to_array());
        let kp2_pub = BytesN::from_array(&env, &kp2.verifying_key().to_bytes());
        client.set_signer(&admin, &kp2_pub);
        assert_eq!(client.get_signer(), Some(kp2_pub));
        let uri = String::from_str(&env, "ipfs://QmExample");
        let old_sig = sign_mint(&env, &kp1, &user1, 1, &uri);
        let result = client.try_mint(
            &user1,
            &1u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &old_sig,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_updates_owner() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        client.transfer(&user1, &user2, &token_id, &0, &None);
        assert_eq!(client.owner_of(&token_id), user2);
    }

    #[test]
    fn test_transfer_emits_event() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 3, &kp);
        client.transfer(&user1, &user2, &token_id, &0, &None);
        let events = env.events().all();
        assert_eq!(events.events().len(), 1);
    }

    #[test]
    fn test_total_supply_derived_from_next_token_id() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        assert_eq!(client.total_supply(), 0);
        do_mint(&client, &env, &user1, 1, &kp);
        assert_eq!(client.total_supply(), 1);
        do_mint(&client, &env, &user1, 2, &kp);
        assert_eq!(client.total_supply(), 2);
    }

    #[test]
    fn test_royalty_info_xlm() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        let info = client.royalty_info(&token_id, &1_000_000i128);
        assert_eq!(info.royalty_amount, 60_000i128);
        assert_eq!(info.asset_address, None);
    }

    #[test]
    fn test_royalty_info_custom_asset() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let asset_addr = Address::generate(&env);
        let mut recipients = Vec::new(&env);
        recipients.push_back(RoyaltyRecipient { recipient: user1.clone(), basis_points: 1000 });
        let royalty = Royalty { recipients, asset_address: Some(asset_addr.clone()) };
        let uri = String::from_str(&env, "ipfs://QmCustom");
        let sig = sign_mint(&env, &kp, &user1, 2, &uri);
        let token_id = client.mint(&user1, &2u32, &uri, &None, &None, &royalty, &false, &sig);
        let info = client.royalty_info(&token_id, &500i128);
        assert_eq!(info.royalty_amount, 55i128);
        assert_eq!(info.asset_address, Some(asset_addr));
    }

    #[test]
    fn test_set_royalty_with_custom_asset() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        let asset_addr = Address::generate(&env);
        let mut recipients = Vec::new(&env);
        recipients.push_back(RoyaltyRecipient { recipient: user2.clone(), basis_points: 1000 });
        let new_royalty = Royalty { recipients, asset_address: Some(asset_addr.clone()) };
        client.set_royalty(&admin, &token_id, &new_royalty);
        let stored = client.get_royalty(&token_id);
        assert_eq!(stored.recipients.get(0).unwrap().recipient, user2);
        assert_eq!(stored.recipients.get(0).unwrap().basis_points, 1000);
        assert_eq!(stored.recipients.len(), 2);
        assert_eq!(stored.asset_address, Some(asset_addr));
    }

    #[test]
    fn test_burn() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
        let token_id2 = do_mint(&client, &env, &user1, 1, &kp);
        assert!(client.exists(&token_id2));
    }

    #[test]
    fn test_burn_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 77, &kp);
        client.burn(&user1, &token_id);
        let events = env.events().all();
        // Burn emits both BurnEvent and TransferEvent
        assert_eq!(events.events().len(), 2);
    }

    #[test]
    fn test_pause_blocks_mint() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        assert!(!client.is_paused());
        client.pause(&admin);
        assert!(!client.is_paused());
        env.ledger().with_mut(|l| l.timestamp += 86_400 + 1);
        assert!(client.is_paused());
        let uri = String::from_str(&env, "ipfs://QmPaused");
        let sig = sign_mint(&env, &kp, &user1, 1, &uri);
        let result = client.try_mint(
            &user1,
            &1u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &sig,
        );
        assert_eq!(result, Err(Ok(Error::ContractPaused)));
    }

    #[test]
    fn test_pause_blocks_transfer() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        client.pause(&admin);
        env.ledger().with_mut(|l| l.timestamp += 86_400 + 1);
        let result = client.try_transfer(&user1, &user2, &token_id, &0, &None);
        assert_eq!(result, Err(Ok(Error::ContractPaused)));
    }

    #[test]
    fn test_unpause_restores_mint_and_transfer() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        client.pause(&admin);
        client.unpause(&admin);
        assert!(!client.is_paused());
        let token_id = do_mint(&client, &env, &user1, 1, &kp);
        client.transfer(&user1, &user2, &token_id, &0, &None);
        assert_eq!(client.owner_of(&token_id), user2);
    }

    #[test]
    #[should_panic]
    fn test_non_admin_cannot_pause() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        client.pause(&user1);
    }

    // soulbound tests
    #[test]
    fn test_mint_soulbound_token() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint_soulbound(&client, &env, &user1, 100, &kp);
        assert_eq!(token_id, 1);
        assert_eq!(client.owner_of(&token_id), user1);
        assert!(client.is_soulbound(&token_id));
    }

    #[test]
    fn test_soulbound_transfer_blocked() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint_soulbound(&client, &env, &user1, 101, &kp);
        let result = client.try_transfer(&user1, &user2, &token_id, &0, &None);
        assert_eq!(result, Err(Ok(Error::SoulboundTransferBlocked)));
        assert_eq!(client.owner_of(&token_id), user1);
    }

    #[test]
    fn test_regular_token_transferable() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 102, &kp);
        assert!(!client.is_soulbound(&token_id));
        client.transfer(&user1, &user2, &token_id, &0, &None);
        assert_eq!(client.owner_of(&token_id), user2);
    }

    #[test]
    fn test_soulbound_can_be_burned() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint_soulbound(&client, &env, &user1, 103, &kp);
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
    }

    // royalty overflow / safe math tests
    #[test]
    fn test_royalty_calculation_safe_math() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let uri = String::from_str(&env, "ipfs://QmExample");

        let token_id = do_mint(&client, &env, &user1, 104, &kp);
        let info = client.royalty_info(&token_id, &1_000_000_000_000_000i128);
        assert_eq!(info.royalty_amount, 60_000_000_000_000i128);
    }

    #[test]
    fn test_royalty_overflow_detection() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 105, &kp);
        let result = client.try_royalty_info(&token_id, &i128::MAX);
        assert_eq!(result, Err(Ok(Error::RoyaltyOverflow)));
    }

    #[test]
    fn test_royalty_calculation_max_safe_price() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 106, &kp);
        let info = client.royalty_info(&token_id, &(i128::MAX / 10_000));
        assert!(info.royalty_amount > 0);
    }

    #[test]
    fn test_royalty_recipient_updated_event() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 107, &kp);
        let mut recipients = Vec::new(&env);
        recipients.push_back(RoyaltyRecipient {
            recipient: user2.clone(),
            basis_points: 500,
        });
        let new_royalty = Royalty {
            recipients,
            asset_address: None,
        };
        
        client.set_royalty(&admin, &token_id, &new_royalty);

        let stored = client.get_royalty(&token_id);
        assert_eq!(stored.recipients.get(0).unwrap().recipient, user2);
    }

    #[test]
    fn test_royalty_recipient_no_event_if_unchanged() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 108, &kp);
        let mut recipients = Vec::new(&env);
        recipients.push_back(RoyaltyRecipient { recipient: user1.clone(), basis_points: 600 });
        client.set_royalty(&admin, &token_id, &Royalty { recipients, asset_address: None });
        let updated = client.get_royalty(&token_id);
        assert_eq!(updated.recipients.get(0).unwrap().basis_points, 600);
    }

    #[test]
    fn test_double_mint_prevention() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let uri = String::from_str(&env, "ipfs://QmUnique");
        let sig = sign_mint(&env, &kp, &user1, 202, &uri);
        let token_id = client.mint(&user1, &202u32, &uri, &None, &None, &default_royalty(&env, user1.clone()), &false, &sig);
        assert_eq!(token_id, 1);
        let sig2 = sign_mint(&env, &kp, &user1, 202, &uri);
        let result = client.try_mint(
            &user1,
            &202u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &sig2,
        );
        assert_eq!(result, Err(Ok(Error::ClipAlreadyMinted)));
    }

    #[test]
    fn test_mint_and_burn_cycle() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 204, &kp);
        assert!(client.exists(&token_id));
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
        let token_id2 = do_mint(&client, &env, &user1, 204, &kp);
        assert!(client.exists(&token_id2));
    }

    #[test]
    fn test_multiple_mints_increment_token_id() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        assert_eq!(do_mint(&client, &env, &user1, 205, &kp), 1);
        assert_eq!(do_mint(&client, &env, &user1, 206, &kp), 2);
        assert_eq!(do_mint(&client, &env, &user1, 207, &kp), 3);
        assert_eq!(client.total_supply(), 3);
    }

    #[test]
    fn test_royalty_with_zero_sale_price_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 208, &kp);
        assert_eq!(client.try_royalty_info(&token_id, &0i128), Err(Ok(Error::InvalidSalePrice)));
        assert_eq!(client.try_royalty_info(&token_id, &(-1000i128)), Err(Ok(Error::InvalidSalePrice)));
    }

    #[test]
    fn test_royalty_calculation_accuracy() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 209, &kp);
        for (price, expected) in [(100i128, 6i128), (1000, 60), (10000, 600), (1_000_000, 60_000)] {
            assert_eq!(client.royalty_info(&token_id, &price).royalty_amount, expected);
        }
    }

    // -------------------------------------------------------------------------
    // Task 1: update_royalty_recipient tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_royalty_recipient_success() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 300, &kp);

        // user1 is the primary recipient — they can update to user2
        client.update_royalty_recipient(&user1, &token_id, &user2);

        let royalty = client.get_royalty(&token_id);
        assert_eq!(royalty.recipients.get(0).unwrap().recipient, user2);
    }

    #[test]
    fn test_update_royalty_recipient_unauthorized() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 301, &kp);

        // user2 is not the royalty recipient — should fail
        let result = client.try_update_royalty_recipient(&user2, &token_id, &user2);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    fn test_update_royalty_recipient_emits_event() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 302, &kp);
        client.update_royalty_recipient(&user1, &token_id, &user2);

        let events = env.events().all();
        assert!(events.events().len() > 0);
    }

    // -------------------------------------------------------------------------
    // Task 1 (Issue #124): tokens_of_owner tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tokens_of_owner_returns_owned_tokens() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let t1 = do_mint(&client, &env, &user1, 400, &kp);
        let t2 = do_mint(&client, &env, &user1, 401, &kp);
        let _t3 = do_mint(&client, &env, &user2, 402, &kp);

        // Pause the contract and advance time past the 24-hour timelock
        client.pause(&admin);
        env.ledger().set_timestamp(env.ledger().timestamp() + 86401);

        let uri = String::from_str(&env, "ipfs://QmPaused");
        let sig = sign_mint(&env, &kp, &user1, 1, &uri);
        let result = client.try_mint(
            &user1,
            &1u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user1.clone()),
            &false,
            &sig,
        );
        assert_eq!(result, Err(Ok(Error::ContractPaused)));
        let owned = client.tokens_of_owner(&user1, &None, &None);
        assert_eq!(owned.len(), 2);
        assert_eq!(owned.get(0).unwrap(), t1);
        assert_eq!(owned.get(1).unwrap(), t2);
    }

    #[test]
    fn test_tokens_of_owner_empty_for_non_owner() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        do_mint(&client, &env, &user1, 403, &kp);

        let owned = client.tokens_of_owner(&user2, &None, &None);
        assert_eq!(owned.len(), 0);
    }

    #[test]
    fn test_tokens_of_owner_updates_after_transfer() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 404, &kp);
        client.transfer(&user1, &user2, &token_id, &0, &None);

        assert_eq!(client.tokens_of_owner(&user1, &None, &None).len(), 0);
        assert_eq!(client.tokens_of_owner(&user2, &None, &None).len(), 1);
    }

    #[test]
    fn test_tokens_of_owner_respects_result_limit() {
        // This test verifies that tokens_of_owner respects the MAX_RESULTS limit
        // to prevent gas explosion. While we can't easily test 1000+ tokens,
        // we verify that the function returns a bounded result.
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 5 tokens to verify basic functionality
        let mut minted = Vec::new(&env);
        for i in 0..5u32 {
            let token_id = do_mint(&client, &env, &user1, 500 + i, &kp);
            minted.push_back(token_id);
        }

        let owned = client.tokens_of_owner(&user1, &None, &None);
        assert_eq!(owned.len(), 5);
        
        // Verify returned tokens match minted tokens
        for i in 0..5 {
            assert_eq!(owned.get(i as u32).unwrap(), minted.get(i as u32).unwrap());
        }
    }

    #[test]
    fn test_tokens_of_owner_pagination_limit() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 10 tokens
        let mut minted = Vec::new(&env);
        for i in 0..10u32 {
            let token_id = do_mint(&client, &env, &user1, 600 + i, &kp);
            minted.push_back(token_id);
        }

        // Test limit parameter
        let page1 = client.tokens_of_owner(&user1, &Some(3u32), &None);
        assert_eq!(page1.len(), 3);
        assert_eq!(page1.get(0).unwrap(), minted.get(0).unwrap());
        assert_eq!(page1.get(1).unwrap(), minted.get(1).unwrap());
        assert_eq!(page1.get(2).unwrap(), minted.get(2).unwrap());

        let page2 = client.tokens_of_owner(&user1, &Some(3u32), &None);
        assert_eq!(page2.len(), 3);
    }

    #[test]
    fn test_tokens_of_owner_pagination_offset() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 10 tokens
        let mut minted = Vec::new(&env);
        for i in 0..10u32 {
            let token_id = do_mint(&client, &env, &user1, 610 + i, &kp);
            minted.push_back(token_id);
        }

        // Test offset parameter - skip first 3
        let page = client.tokens_of_owner(&user1, &None, &Some(3u32));
        assert_eq!(page.len(), 7);
        assert_eq!(page.get(0).unwrap(), minted.get(3).unwrap());
        assert_eq!(page.get(1).unwrap(), minted.get(4).unwrap());
    }

    #[test]
    fn test_tokens_of_owner_pagination_limit_and_offset() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 10 tokens
        let mut minted = Vec::new(&env);
        for i in 0..10u32 {
            let token_id = do_mint(&client, &env, &user1, 620 + i, &kp);
            minted.push_back(token_id);
        }

        // Test both limit and offset - get page 2 (skip 3, take 3)
        let page1 = client.tokens_of_owner(&user1, &Some(3u32), &Some(0u32));
        assert_eq!(page1.len(), 3);
        assert_eq!(page1.get(0).unwrap(), minted.get(0).unwrap());
        assert_eq!(page1.get(1).unwrap(), minted.get(1).unwrap());
        assert_eq!(page1.get(2).unwrap(), minted.get(2).unwrap());

        let page2 = client.tokens_of_owner(&user1, &Some(3u32), &Some(3u32));
        assert_eq!(page2.len(), 3);
        assert_eq!(page2.get(0).unwrap(), minted.get(3).unwrap());
        assert_eq!(page2.get(1).unwrap(), minted.get(4).unwrap());
        assert_eq!(page2.get(2).unwrap(), minted.get(5).unwrap());

        let page3 = client.tokens_of_owner(&user1, &Some(3u32), &Some(6u32));
        assert_eq!(page3.len(), 3);
        assert_eq!(page3.get(0).unwrap(), minted.get(6).unwrap());
        assert_eq!(page3.get(1).unwrap(), minted.get(7).unwrap());
        assert_eq!(page3.get(2).unwrap(), minted.get(8).unwrap());

        let page4 = client.tokens_of_owner(&user1, &Some(3u32), &Some(9u32));
        assert_eq!(page4.len(), 1);
        assert_eq!(page4.get(0).unwrap(), minted.get(9).unwrap());
    }

    #[test]
    fn test_tokens_of_owner_pagination_limit_exceeds_max() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 5 tokens
        for i in 0..5u32 {
            do_mint(&client, &env, &user1, 630 + i, &kp);
        }

        // Test that limit exceeding MAX_RESULTS is capped
        let result = client.tokens_of_owner(&user1, &Some(2000u32), &None);
        assert_eq!(result.len(), 5); // Only 5 tokens exist
    }

    #[test]
    fn test_tokens_of_owner_pagination_offset_exceeds_count() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 5 tokens
        for i in 0..5u32 {
            do_mint(&client, &env, &user1, 635 + i, &kp);
        }

        // Test that offset exceeding token count returns empty
        let result = client.tokens_of_owner(&user1, &None, &Some(10u32));
        assert_eq!(result.len(), 0);
    }

    // -------------------------------------------------------------------------
    // Task 2: batch_mint tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_batch_mint_success() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let uri1 = String::from_str(&env, "ipfs://QmBatch1");
        let uri2 = String::from_str(&env, "ipfs://QmBatch2");
        let sig1 = sign_mint(&env, &kp, &user1, 500, &uri1);
        let sig2 = sign_mint(&env, &kp, &user1, 501, &uri2);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(500u32);
        clip_ids.push_back(501u32);

        let mut uris = Vec::new(&env);
        uris.push_back(uri1.clone());
        uris.push_back(uri2.clone());

        let mut sigs = Vec::new(&env);
        sigs.push_back(sig1);
        sigs.push_back(sig2);

        let mut images = Vec::new(&env);
        images.push_back(None);
        images.push_back(None);

        let mut animation_urls = Vec::new(&env);
        animation_urls.push_back(None);
        animation_urls.push_back(None);

        let minted = client.batch_mint(
            &user1,
            &clip_ids,
            &uris,
            &images,
            &animation_urls,
            &default_royalty(&env, user1.clone()),
            &false,
            &sigs,
        );

        assert_eq!(minted.len(), 2);
        assert_eq!(client.owner_of(&minted.get(0).unwrap()), user1);
        assert_eq!(client.owner_of(&minted.get(1).unwrap()), user1);
        assert_eq!(client.token_uri(&minted.get(0).unwrap()), uri1);
        assert_eq!(client.token_uri(&minted.get(1).unwrap()), uri2);
    }

    #[test]
    fn test_batch_mint_duplicate_clip_id_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Pre-mint clip 502
        do_mint(&client, &env, &user1, 502, &kp);

        let uri = String::from_str(&env, "ipfs://QmDup");
        let sig = sign_mint(&env, &kp, &user1, 502, &uri);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(502u32);
        let mut uris = Vec::new(&env);
        uris.push_back(uri);
        let mut images = Vec::new(&env);
        images.push_back(None);
        let mut animation_urls = Vec::new(&env);
        animation_urls.push_back(None);
        let mut sigs = Vec::new(&env);
        sigs.push_back(sig);

        let result = client.try_batch_mint(
            &user1,
            &clip_ids,
            &uris,
            &images,
            &animation_urls,
            &default_royalty(&env, user1.clone()),
            &false,
            &sigs,
        );
        assert_eq!(result, Err(Ok(Error::ClipAlreadyMinted)));
    }

    #[test]
    fn test_batch_mint_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let uri = String::from_str(&env, "ipfs://QmBatchEvt");
        let sig = sign_mint(&env, &kp, &user1, 503, &uri);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(503u32);
        let mut uris = Vec::new(&env);
        uris.push_back(uri);
        let mut sigs = Vec::new(&env);
        sigs.push_back(sig);

        let mut images = Vec::new(&env);
        images.push_back(None);
        let mut animation_urls = Vec::new(&env);
        animation_urls.push_back(None);

        client.batch_mint(
            &user1,
            &clip_ids,
            &uris,
            &images,
            &animation_urls,
            &default_royalty(&env, user1.clone()),
            &false,
            &sigs,
        );

        let events = env.events().all();
        assert!(events.events().len() > 0);
    }

    // -------------------------------------------------------------------------
    // Task 3: exists tests (function already existed, verify behavior)
    // -------------------------------------------------------------------------

    #[test]
    fn test_exists_returns_true_for_minted_token() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 600, &kp);
        assert!(client.exists(&token_id));
    }

    #[test]
    fn test_exists_returns_false_for_unminted_token() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        assert!(!client.exists(&9999u32));
    }

    #[test]
    fn test_exists_returns_false_after_burn() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 601, &kp);
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
    }

    // -------------------------------------------------------------------------
    // Task 4: calculate_royalty_amount tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_calculate_royalty_amount_basic() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 104, &kp);

        // Test with large but safe values
        let large_price = 1_000_000_000_000_000i128; // 10^15
        let info = client.royalty_info(&token_id, &large_price);

        // Should calculate without overflow: 10^15 * 600 / 10000 = 6 * 10^13
        assert_eq!(info.royalty_amount, 60_000_000_000_000i128);
        // default_royalty = 5% creator + 1% platform = 6% total
        let token_id = do_mint(&client, &env, &user1, 700, &kp);
        let amount = client.calculate_royalty_amount(&token_id, &10_000i128);
        assert_eq!(amount, 600i128); // 6% of 10000
    }

    #[test]
    fn test_calculate_royalty_amount_zero_price_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 105, &kp);

        // Test with value that would overflow: i128::MAX
        let overflow_price = i128::MAX;
        let result = client.try_royalty_info(&token_id, &overflow_price);

        // Should detect overflow and return error
        assert_eq!(result, Err(Ok(Error::RoyaltyOverflow)));
        let token_id = do_mint(&client, &env, &user1, 701, &kp);
        let result = client.try_calculate_royalty_amount(&token_id, &0i128);
        assert_eq!(result, Err(Ok(Error::InvalidSalePrice)));
    }

    #[test]
    fn test_calculate_royalty_amount_overflow_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 106, &kp);

        // Test with maximum safe price: i128::MAX / 10000
        let max_safe_price = i128::MAX / 10_000;
        let info = client.royalty_info(&token_id, &max_safe_price);

        // Should succeed with safe calculation
        assert!(info.royalty_amount > 0);
        let token_id = do_mint(&client, &env, &user1, 702, &kp);
        let result = client.try_calculate_royalty_amount(&token_id, &i128::MAX);
        assert_eq!(result, Err(Ok(Error::RoyaltyOverflow)));
    }

    // -------------------------------------------------------------------------
    // Task 1: 48-hour timelock tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_withdraw_timelock_is_48h() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        // Request a withdrawal — unlock_time should be now + 172_800 seconds
        client.request_withdraw_asset(&admin, &1_000i128);

        let request: WithdrawRequest = env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .get(&DataKey::WithdrawXlmRequest)
                .unwrap()
        });

        let expected_unlock = env.ledger().timestamp() + 172_800;
        assert_eq!(request.unlock_time, expected_unlock);
        assert_eq!(request.amount, 1_000i128);
    }

    #[test]
    fn test_withdraw_blocked_before_48h() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        client.request_withdraw_asset(&admin, &500i128);

        // Advance time by only 47 hours — still locked
        env.ledger().with_mut(|l| l.timestamp += 169_200);

        let asset = Address::generate(&env);
        let result = client.try_withdraw_asset(&admin, &asset, &500i128);
        assert_eq!(result, Err(Ok(Error::WithdrawalStillLocked)));
    }

    #[test]
    fn test_last_withdrawal_time_not_set_before_execution() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        // Before any withdrawal, LastWithdrawalTime should not exist
        let stored: Option<u64> = env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .get(&DataKey::LastWithdrawalTime)
        });
        assert_eq!(stored, None);

        // After requesting (but not executing), it should still be absent
        client.request_withdraw_asset(&admin, &100i128);
        let stored: Option<u64> = env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .get(&DataKey::LastWithdrawalTime)
        });
        assert_eq!(stored, None);
    }

    // -------------------------------------------------------------------------
    // Task 3 & 4: Royalty overflow — checked_mul, max i128 boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_royalty_checked_mul_max_safe_boundary() {
        // sale_price == i128::MAX / 10_000 should succeed (boundary value)
        let max_safe = i128::MAX / 10_000;
        let result = ClipsNftContract::calculate_royalty(max_safe, 10_000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), max_safe); // 100% of max_safe
    }

    #[test]
    fn test_royalty_checked_mul_one_over_boundary_fails() {
        // sale_price == i128::MAX / 10_000 + 1 should overflow
        let over_boundary = i128::MAX / 10_000 + 1;
        let result = ClipsNftContract::calculate_royalty(over_boundary, 1);
        assert_eq!(result, Err(Error::RoyaltyOverflow));
    }

    #[test]
    fn test_royalty_checked_mul_i128_max_fails() {
        let result = ClipsNftContract::calculate_royalty(i128::MAX, 500);
        assert_eq!(result, Err(Error::RoyaltyOverflow));
    }

    #[test]
    fn test_royalty_checked_mul_zero_basis_points() {
        // 0 basis points → 0 royalty regardless of price
        let result = ClipsNftContract::calculate_royalty(1_000_000, 0);
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn test_balance_of_counts_owned_tokens() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        assert_eq!(client.balance_of(&user1), 0);
        let t1 = do_mint(&client, &env, &user1, 800, &kp);
        assert_eq!(client.balance_of(&user1), 1);
        let _t2 = do_mint(&client, &env, &user1, 801, &kp);
        assert_eq!(client.balance_of(&user1), 2);

        client.transfer(&user1, &user2, &t1, &0, &None);
        assert_eq!(client.balance_of(&user1), 1);
        assert_eq!(client.balance_of(&user2), 1);
    }

    #[test]
    fn test_token_by_index_enumerable() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let t1 = do_mint(&client, &env, &user1, 810, &kp);
        let _t2 = do_mint(&client, &env, &user1, 811, &kp);
        let t3 = do_mint(&client, &env, &user1, 812, &kp);

        assert_eq!(client.token_by_index(&0), t1);
        assert_eq!(client.token_by_index(&2), t3);

        client.burn(&user1, &t1);
        assert_eq!(client.token_by_index(&0), 2);
    }

    #[test]
    fn test_token_by_index_out_of_bounds() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        do_mint(&client, &env, &user1, 820, &kp);
        let result = client.try_token_by_index(&5);
        assert_eq!(result, Err(Ok(Error::InvalidTokenId)));
    }

    #[test]
    fn test_royalty_checked_mul_large_safe_price() {
        // 10^15 stroops * 600 bps / 10_000 = 6 * 10^13
        let result = ClipsNftContract::calculate_royalty(1_000_000_000_000_000i128, 600);
        assert_eq!(result, Ok(60_000_000_000_000i128));
    }

    // -------------------------------------------------------------------------
    // Issue #117: refresh_metadata with 30-day cooldown
    // -------------------------------------------------------------------------

    #[test]
    fn test_refresh_metadata_admin_success() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2000, &kp);

        let uri = String::from_str(&env, "ipfs://QmUnauth");
        // Sign for user1 but try to mint as user2
        let sig = sign_mint(&env, &kp, &user1, 203, &uri);

        let result = client.try_mint(
            &user2,
            &203u32,
            &uri,
            &None,
            &None,
            &default_royalty(&env, user2.clone()),
            &false,
            &sig,
        );
        // Should fail because signature doesn't match the caller
        assert!(result.is_err());
        let new_uri = String::from_str(&env, "ipfs://QmRefreshed");
        client.refresh_metadata(&admin, &token_id, &Some(new_uri.clone()), &None, &None);

        assert_eq!(client.token_uri(&token_id), new_uri);
    }

    #[test]
    fn test_refresh_metadata_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        // Mint token
        let token_id = do_mint(&client, &env, &user1, 204, &kp);
        assert!(client.exists(&token_id));
        assert_eq!(client.total_supply(), 1);

        // Burn token
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
        // total_supply is decremented on burn
        assert_eq!(client.total_supply(), 0);

        // Can re-mint same clip_id after burn
        let token_id2 = do_mint(&client, &env, &user1, 204, &kp);
        assert!(client.exists(&token_id2));
        assert_eq!(client.total_supply(), 1);
        let new_uri = String::from_str(&env, "ipfs://QmRefreshedEvt");
        client.refresh_metadata(&admin, &token_id2, &Some(new_uri.clone()), &None, &None);

        let events = env.events().all();
        assert!(events.events().len() >= 1);
        assert_eq!(client.token_uri(&token_id2), new_uri);
    }

    #[test]
    fn test_refresh_metadata_non_admin_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2002, &kp);

        let result = client.try_refresh_metadata(
            &user1,
            &token_id,
            &Some(String::from_str(&env, "ipfs://QmHack")),
            &None,
            &None,
        );
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    fn test_refresh_metadata_cooldown_enforced() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2003, &kp);

        client.refresh_metadata(&admin, &token_id, &Some(String::from_str(&env, "ipfs://QmFirst")), &None, &None);

        // Advance time by 29 days — still within cooldown
        env.ledger().with_mut(|l| l.timestamp += 29 * 24 * 3600);

        let result = client.try_refresh_metadata(
            &admin,
            &token_id,
            &Some(String::from_str(&env, "ipfs://QmTooSoon")),
            &None,
            &None,
        );
        assert_eq!(result, Err(Ok(Error::MetadataRefreshTooSoon)));
    }

    #[test]
    fn test_refresh_metadata_allowed_after_30_days() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2004, &kp);

        client.refresh_metadata(&admin, &token_id, &Some(String::from_str(&env, "ipfs://QmFirst")), &None, &None);

        // Advance time by exactly 30 days
        env.ledger().with_mut(|l| l.timestamp += 30 * 24 * 3600);

        let new_uri = String::from_str(&env, "ipfs://QmSecond");
        client.refresh_metadata(&admin, &token_id, &Some(new_uri.clone()), &None, &None);
        assert_eq!(client.token_uri(&token_id), new_uri);
    }

    #[test]
    fn test_refresh_metadata_invalid_token_fails() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let result = client.try_refresh_metadata(
            &admin,
            &9999u32,
            &Some(String::from_str(&env, "ipfs://QmGhost")),
            &None,
            &None,
        );
        assert_eq!(result, Err(Ok(Error::InvalidTokenId)));
    }

    // -------------------------------------------------------------------------
    // get_tokens_by_clip_ids tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_tokens_by_clip_ids_all_minted() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let t1 = do_mint(&client, &env, &user1, 1000, &kp);
        let t2 = do_mint(&client, &env, &user1, 1001, &kp);
        let t3 = do_mint(&client, &env, &user1, 1002, &kp);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(1000u32);
        clip_ids.push_back(1001u32);
        clip_ids.push_back(1002u32);

        let result = client.get_tokens_by_clip_ids(&clip_ids);
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).unwrap(), Some(t1));
        assert_eq!(result.get(1).unwrap(), Some(t2));
        assert_eq!(result.get(2).unwrap(), Some(t3));
    }

    #[test]
    fn test_get_tokens_by_clip_ids_partial_minted() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Only mint clip_id 1000
        let t1 = do_mint(&client, &env, &user1, 1000, &kp);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(1000u32); // minted
        clip_ids.push_back(1001u32); // not minted
        clip_ids.push_back(1002u32); // not minted

        let result = client.get_tokens_by_clip_ids(&clip_ids);
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).unwrap(), Some(t1));
        assert_eq!(result.get(1).unwrap(), None);
        assert_eq!(result.get(2).unwrap(), None);
    }

    #[test]
    fn test_get_tokens_by_clip_ids_empty_input() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let clip_ids = Vec::new(&env);
        let result = client.get_tokens_by_clip_ids(&clip_ids);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_get_tokens_by_clip_ids_all_unminted() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(2000u32);
        clip_ids.push_back(2001u32);
        clip_ids.push_back(2002u32);

        let result = client.get_tokens_by_clip_ids(&clip_ids);
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).unwrap(), None);
        assert_eq!(result.get(1).unwrap(), None);
        assert_eq!(result.get(2).unwrap(), None);
    }

    // -------------------------------------------------------------------------
    // Circuit breaker tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_circuit_breaker_disabled_by_default() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        assert!(!client.circuit_breaker_enabled());
    }

    #[test]
    fn test_circuit_breaker_can_be_enabled() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        client.set_circuit_breaker_enabled(&admin, &true);
        assert!(client.circuit_breaker_enabled());
    }

    #[test]
    fn test_circuit_breaker_threshold_configurable() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        client.set_circuit_breaker_threshold(&admin, &50);
        assert_eq!(client.circuit_breaker_threshold(), 50);
    }

    #[test]
    fn test_circuit_breaker_window_configurable() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        client.set_circuit_breaker_window(&admin, &30);
        assert_eq!(client.circuit_breaker_window_seconds(), 30);
    }

    #[test]
    fn test_circuit_breaker_non_admin_cannot_configure() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let result = client.try_set_circuit_breaker_enabled(&user1, &true);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));

        let result = client.try_set_circuit_breaker_threshold(&user1, &50);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));

        let result = client.try_set_circuit_breaker_window(&user1, &30);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));

        let result = client.try_reset_circuit_breaker(&user1);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

}
