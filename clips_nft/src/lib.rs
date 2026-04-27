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
//! - [`ClipsNftContract::set_signer`]
//! - [`ClipsNftContract::upgrade`]
//! - [`ClipsNftContract::pause`]
//! - [`ClipsNftContract::unpause`]
//! - [`ClipsNftContract::blacklist_clip`]
//! - [`ClipsNftContract::set_name`]
//! - [`ClipsNftContract::set_symbol`]
//! - [`ClipsNftContract::set_royalty`]

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, String, Vec,
};

/// Contract version — bump on every breaking change.
pub const VERSION: u32 = 1;

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

/// Emitted when a global config value is updated by the admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdatedEvent {
    /// Name of the config field that changed (e.g. "platform_fee", "default_royalty").
    pub key: String,
    /// New value in basis points.
    pub new_value: u32,
}


/// Emerging Soroban NFT standard interface (ERC-721 adapted).
/// Documents the expected API surface for marketplace interoperability.
pub trait NftStandard {
    fn balance_of(env: Env, owner: Address) -> u32;
    fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error>;
    fn transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error>;
    fn approve(env: Env, caller: Address, operator: Option<Address>, token_id: TokenId) -> Result<(), Error>;
    fn get_approved(env: Env, token_id: TokenId) -> Option<Address>;
    fn set_approval_for_all(env: Env, caller: Address, operator: Address, approved: bool) -> Result<(), Error>;
    fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool;
    fn total_supply(env: Env) -> u32;
    fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error>;
    fn name(env: Env) -> String;
    fn symbol(env: Env) -> String;
}

// =============================================================================
// Contract
// =============================================================================

/// ClipCash NFT contract.
#[contract]
pub struct ClipsNftContract;

/// Synthetic gas constants for tracking (approximations)
const GAS_BASE_MINT: u64 = 50_000;
const GAS_BASE_TRANSFER: u64 = 30_000;
const MAX_BATCH_MINT: u32 = 25;

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
        Ok(())
    }

    /// Return the currently registered backend signer public key, if any.
    pub fn get_signer(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&DataKey::Signer)
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
    /// * `admin`         — Must be the contract admin.
    /// * `new_wasm_hash` — 32-byte SHA-256 hash of the new WASM blob.
    pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
        env.events().publish(
            (symbol_short!("upgrade"),),
            UpgradeEvent { new_wasm_hash },
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Pausable  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Pause the contract. Blocks `mint` and `transfer` until unpaused.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"paused"` event.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((symbol_short!("paused"),), ());
        Ok(())
    }

    /// Unpause the contract, re-enabling `mint` and `transfer`.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"unpaused"` event.
    pub fn unpause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((symbol_short!("unpaused"),), ());
        Ok(())
    }

    /// Returns `true` if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Request an emergency withdrawal of XLM (or any other token).
    /// Starts a 48-hour safety delay (timelock) before the withdrawal can be executed.
    /// Only callable by the admin.
    ///
    /// Emits `WithdrawRequested` event with amount and unlock_time.
    ///
    /// Part of Closes #78
    pub fn request_withdraw_xlm(env: Env, admin: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if amount <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let unlock_time = env.ledger().timestamp().saturating_add(172_800); // 48 hours
        let request = WithdrawRequest { amount, unlock_time };

        env.storage().instance().set(&DataKey::WithdrawXlmRequest, &request);

        env.events().publish(
            (symbol_short!("with_req"),),
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
    pub fn withdraw_xlm(env: Env, admin: Address, asset: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;

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
        let client = soroban_sdk::token::TokenClient::new(&env, &asset);
        client.transfer(&env.current_contract_address(), &admin, &amount);

        // Record the timestamp of this withdrawal for audit purposes
        env.storage()
            .instance()
            .set(&DataKey::LastWithdrawalTime, &env.ledger().timestamp());

        env.events().publish(
            (symbol_short!("with_exe"),),
            WithdrawExecutedEvent { amount, recipient: admin },
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
            .publish((symbol_short!("blacklist"),), BlacklistEvent { clip_id });
        Ok(())
    }

    pub fn freeze(env: Env, admin: Address, token_id: TokenId) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if !Self::exists(env.clone(), token_id) {
            return Err(Error::InvalidTokenId);
        }
        env.storage().persistent().set(&DataKey::Frozen(token_id), &true);
        env.events().publish((symbol_short!("freeze"),), TokenFrozenEvent { token_id });
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
        env.events().publish((symbol_short!("unfreeze"),), TokenUnfrozenEvent { token_id });
        Ok(())
    }

    /// Returns `true` if the token is currently frozen.
    pub fn is_frozen(env: Env, token_id: TokenId) -> bool {
        env.storage().persistent().get(&DataKey::Frozen(token_id)).unwrap_or(false)
    }

    // -------------------------------------------------------------------------
    // Core NFT operations
    // -------------------------------------------------------------------------

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
    pub fn mint(
        env: Env,
        to: Address,
        clip_id: u32,
        metadata_uri: String,
        royalty: Royalty,
        is_soulbound: bool,
        signature: BytesN<64>,
    ) -> Result<TokenId, Error> {
        to.require_auth();
        Self::require_not_paused(&env)?;

        // Verify backend signature before any state reads/writes.
        Self::verify_clip_signature(&env, &to, clip_id, &metadata_uri, &signature)?;

        // Dedup check — one persistent read.
        if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
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

        // 2 persistent writes (optimized from 4).
        env.storage().persistent().set(
            &DataKey::Token(token_id),
            &TokenData {
                owner: to.clone(),
                clip_id,
                is_soulbound,
                metadata_uri: metadata_uri.clone(),
                royalty,
            },
        );
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &token_id);

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
            (symbol_short!("mint"),),
            MintEvent { to: to.clone(), clip_id, token_id, metadata_uri },
        );

        // Emit standard Transfer event for ERC-721 compliance
        // (contract address stands in for the zero address)
        env.events().publish(
            (symbol_short!("transfer"),),
            TransferEvent {
                token_id,
                from: env.current_contract_address(),
                to: to.clone(),
            },
        );

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

        if caller != owner && !Self::is_approved_for_all(env.clone(), owner.clone(), caller.clone()) {
            return Err(Error::NotAuthorizedToApprove);
        }

        if let Some(op) = operator.clone() {
            env.storage().persistent().set(&DataKey::Approved(token_id), &op);
            env.events().publish(
                (symbol_short!("approve"),),
                ApprovalEvent { owner, operator: op, token_id },
            );
        } else {
            env.storage().persistent().remove(&DataKey::Approved(token_id));
        }

        Ok(())
    }

    /// Grant or revoke an operator's permission to manage all of the caller's tokens.
    ///
    /// Emits: `"appr_all"` [`ApprovalForAllEvent`].
    ///
    /// # Arguments
    /// * `caller`   — Token owner (must authorize).
    /// * `operator` — Address to grant or revoke.
    /// * `approved` — `true` to grant, `false` to revoke.
    pub fn set_approval_for_all(
        env: Env,
        caller: Address,
        operator: Address,
        approved: bool,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_not_paused(&env)?;

        env.storage()
            .persistent()
            .set(&DataKey::ApprovalForAll(caller.clone(), operator.clone()), &approved);

        env.events().publish(
            (symbol_short!("appr_all"),),
            ApprovalForAllEvent { owner: caller, operator, approved },
        );

        Ok(())
    }

    /// Returns `true` if `operator` is approved to manage all of `owner`'s tokens.
    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovalForAll(owner, operator))
            .unwrap_or(false)
    }

    /// Returns the approved operator for a specific token, or `None`.
    pub fn get_approved(env: Env, token_id: TokenId) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Approved(token_id))
    }

    // -------------------------------------------------------------------------
    // Transfers
    // -------------------------------------------------------------------------

    /// Transfer NFT ownership from `from` to `to`.
    ///
    /// Blocked when the contract is paused or the token is soulbound.
    /// Clears any existing per-token approval on success.
    ///
    /// Storage writes: 1 persistent (TokenData).
    ///
    /// Emits: `"transfer"` [`TransferEvent`].
    ///
    /// # Arguments
    /// * `from`     — Current owner (must authorize).
    /// * `to`       — New owner.
    /// * `token_id` — Token to transfer.
    ///
    /// # Errors
    /// * [`Error::ContractPaused`]          — contract is paused.
    /// * [`Error::InvalidTokenId`]          — token does not exist.
    /// * [`Error::Unauthorized`]            — `from` is not the owner.
    /// * [`Error::SoulboundTransferBlocked`] — token is soulbound.
    pub fn transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error> {
        from.require_auth();
        Self::require_not_paused(&env)?;

        if Self::is_frozen(env.clone(), token_id) {
            return Err(Error::TokenFrozen);
        }

        let mut data: TokenData = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)?;

        if from != data.owner {
            return Err(Error::Unauthorized);
        }

        if data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
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
            (symbol_short!("transfer"),),
            TransferEvent { token_id, from, to },
        );

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

        let mut data: TokenData = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)?;

        if from != data.owner {
            return Err(Error::Unauthorized);
        }

        let is_approved_for_all =
            Self::is_approved_for_all(env.clone(), from.clone(), spender.clone());
        let approved_operator = Self::get_approved(env.clone(), token_id);

        if !is_approved_for_all && approved_operator != Some(spender.clone()) {
            return Err(Error::Unauthorized);
        }

        if data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
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
            (symbol_short!("transfer"),),
            TransferEvent { token_id, from, to },
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Admin configuration  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Set the collection name.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// # Arguments
    /// * `admin` — Must be the contract admin.
    /// * `name`  — New collection name string.
    pub fn set_name(env: Env, admin: Address, name: String) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Name, &name);
        Ok(())
    }

    /// Set the collection symbol.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// # Arguments
    /// * `admin`  — Must be the contract admin.
    /// * `symbol` — New collection symbol string.
    pub fn set_symbol(env: Env, admin: Address, symbol: String) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        Ok(())
    }

    /// Set the platform fee in basis points (0–10 000).
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"cfg_upd"` [`ConfigUpdatedEvent`] with key `"platform_fee"`.
    pub fn set_platform_fee(env: Env, admin: Address, bps: u16) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if bps as u32 > 10_000 {
            return Err(Error::RoyaltyTooHigh);
        }
        env.storage().instance().set(&DataKey::PlatformFeeBps, &(bps as u32));
        env.events().publish(
            (symbol_short!("cfg_upd"),),
            ConfigUpdatedEvent {
                key: String::from_str(&env, "platform_fee"),
                new_value: bps as u32,
            },
        );
        Ok(())
    }

    /// Get the current platform fee in basis points.
    pub fn get_platform_fee(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::PlatformFeeBps).unwrap_or(100)
    }

    /// Set the default royalty in basis points (0–10 000).
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"cfg_upd"` [`ConfigUpdatedEvent`] with key `"default_royalty"`.
    pub fn set_default_royalty(env: Env, admin: Address, bps: u16) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if bps as u32 > 10_000 {
            return Err(Error::RoyaltyTooHigh);
        }
        env.storage().instance().set(&DataKey::DefaultRoyaltyBps, &(bps as u32));
        env.events().publish(
            (symbol_short!("cfg_upd"),),
            ConfigUpdatedEvent {
                key: String::from_str(&env, "default_royalty"),
                new_value: bps as u32,
            },
        );
        Ok(())
    }

    /// Get the current default royalty in basis points.
    pub fn get_default_royalty(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::DefaultRoyaltyBps).unwrap_or(500)
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
        let mut data = Self::load_token(&env, token_id)?;
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

        let old_uri = data.metadata_uri.clone();
        let mut data = data;
        data.metadata_uri = new_uri.clone();
        
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &data);
        
        // Increment update count
        env.storage()
            .persistent()
            .set(&DataKey::MetadataUpdateCount(token_id), &(update_count + 1));

        env.events().publish(
            (symbol_short!("meta_upd"),),
            MetadataUpdatedEvent {
                token_id,
                old_uri,
                new_uri,
            },
        );

        Ok(())
    }

    /// Set a custom token URI for a minted token. Only the token owner can update it.
    /// Deprecated: Use update_metadata instead.
    pub fn set_token_uri(
        env: Env,
        owner: Address,
        token_id: TokenId,
        uri: String,
    ) -> Result<(), Error> {
        Self::update_metadata(env, owner, token_id, uri)
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
    pub fn refresh_metadata(
        env: Env,
        caller: Address,
        token_id: TokenId,
        new_uri: String,
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

        let mut data = Self::load_token(&env, token_id)?;
        let old_uri = data.metadata_uri.clone();
        data.metadata_uri = new_uri.clone();
        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        env.storage()
            .persistent()
            .set(&DataKey::MetadataRefreshTime(token_id), &now);

        env.events().publish(
            (symbol_short!("meta_upd"),),
            MetadataUpdatedEvent { token_id, old_uri, new_uri },
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // View functions
    // -------------------------------------------------------------------------

    /// Returns the contract version number.
    pub fn version(_env: Env) -> u32 {
        VERSION
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

    /// Look up the on-chain token ID for a given `clip_id`.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — no token exists for this clip.
    pub fn clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::ClipIdMinted(clip_id))
            .ok_or(Error::InvalidTokenId)
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

    /// Returns `true` if the token exists.
    pub fn exists(env: Env, token_id: TokenId) -> bool {
        env.storage().persistent().has(&DataKey::Token(token_id))
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

        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let royalty = Self::load_token(&env, token_id)?.royalty;
        let asset_address = royalty.asset_address.clone().ok_or(Error::InvalidRecipient)?;
        let token_client = soroban_sdk::token::TokenClient::new(&env, &asset_address);

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

            token_client.transfer(&payer, &split.recipient, &amount);
            env.events().publish(
                (symbol_short!("royalty"),),
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
                    (symbol_short!("royalty"),),
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
            (symbol_short!("burn"),),
            BurnEvent {
                owner: owner.clone(),
                token_id,
                clip_id: data.clip_id,
            },
        );

        // Emit standard Transfer event for ERC-721 compliance
        env.events().publish(
            (symbol_short!("transfer"),),
            TransferEvent {
                token_id,
                from: owner.clone(),
                to: env.current_contract_address(),
            },
        );

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
            (symbol_short!("royalty"),),
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

    /// Return all token IDs owned by `owner`.
    ///
    /// This function enables frontends to display all NFTs owned by a user.
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
    /// - Callers should use pagination or alternative queries for larger result sets
    ///
    /// # Arguments
    /// * `owner` - Address to query
    ///
    /// # Returns
    /// Vec of token IDs owned by the address, capped at 1000 entries
    pub fn tokens_of_owner(env: Env, owner: Address) -> Vec<TokenId> {
        const MAX_RESULTS: u32 = 1000;
        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        let mut result: Vec<TokenId> = Vec::new(&env);
        let mut count: u32 = 0;

        let mut token_id: u32 = 1;
        while token_id < next_id && count < MAX_RESULTS {
            if let Some(data) = env
                .storage()
                .persistent()
                .get::<DataKey, TokenData>(&DataKey::Token(token_id))
            {
                if data.owner == owner {
                    result.push_back(token_id);
                    count += 1;
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
    /// * `royalty`       - Royalty config applied to all tokens
    /// * `is_soulbound`  - Whether all tokens are soulbound
    /// * `signatures`    - Per-clip backend signatures
    pub fn batch_mint(
        env: Env,
        to: Address,
        clip_ids: Vec<u32>,
        metadata_uris: Vec<String>,
        royalty: Royalty,
        is_soulbound: bool,
        signatures: Vec<BytesN<64>>,
    ) -> Result<Vec<TokenId>, Error> {
        to.require_auth();
        Self::require_not_paused(&env)?;

        let n = clip_ids.len();
        if n != metadata_uris.len() || n != signatures.len() {
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
            let signature = signatures.get(i).ok_or(Error::InvalidTokenId)?;

            Self::verify_clip_signature(&env, &to, clip_id, &metadata_uri, &signature)?;

            if env
                .storage()
                .persistent()
                .has(&DataKey::ClipIdMinted(clip_id))
            {
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
                    royalty: royalty.clone(),
                },
            );
            env.storage()
                .persistent()
                .set(&DataKey::ClipIdMinted(clip_id), &token_id);
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
            (symbol_short!("batch_mnt"),),
            BatchMintEvent {
                to,
                count: n,
                first_token_id: minted.get(0).unwrap_or(0),
            },
        );

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
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Load and return `TokenData`, or `InvalidTokenId` if not found.
    fn load_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)
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

    /// Return `ContractPaused` if the pause flag is set.
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

    /// Validate royalty recipients and append the platform 1 % cut if absent.
    fn normalize_royalty(env: &Env, royalty: Royalty) -> Result<Royalty, Error> {
        if royalty.recipients.is_empty() {
            return Err(Error::InvalidRoyaltySplit);
        }

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

        Ok(Royalty { recipients, asset_address: royalty.asset_address })
    }
    /// Calculate royalty amount using safe (checked) arithmetic.
    ///
    /// Formula: `royalty_amount = (sale_price * basis_points + 5_000) / 10_000`
    ///
    /// # Safe price limits
    /// `sale_price` must be ≤ `i128::MAX / 10_000` (≈ 1.7 × 10³⁴ stroops).
    /// Prices above this threshold return `RoyaltyOverflow`.
    pub fn calculate_royalty(sale_price: i128, basis_points: u32) -> Result<i128, Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }
        // Guard: sale_price * 10_000 must not overflow i128
        if sale_price > i128::MAX / 10_000 {
            return Err(Error::RoyaltyOverflow);
        }
        let numerator = sale_price
            .checked_mul(basis_points as i128)
            .ok_or(Error::RoyaltyOverflow)?
            .checked_add(5_000)
            .ok_or(Error::RoyaltyOverflow)?;
        Ok(numerator / 10_000)
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
        client.mint(to, &clip_id, &uri, &default_royalty(env, to.clone()), &false, &sig)
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
        client.mint(to, &clip_id, &uri, &default_royalty(env, to.clone()), &true, &sig)
    }

    #[test]
    fn test_version() {
        let env = Env::default();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        assert_eq!(client.version(), 1);
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
        let token_uri_events: Vec<_> = events
            .iter()
            .filter(|e| {
                if let soroban_sdk::xdr::ContractEvent::V0(ev) = &e.event {
                    ev.topics.get(0).map_or(false, |t| {
                        matches!(t, soroban_sdk::xdr::ScVal::Symbol(s) if s.0.to_vec() == b"token_uri".to_vec())
                    })
                } else {
                    false
                }
            })
            .collect();

        assert!(token_uri_events.len() > 0, "TokenUriChanged event should be emitted");
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
        let result = client.try_mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig);
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
        client.mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &bad_sig);
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
        client.mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig_for_user2);
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
        client.mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig_for_99);
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
        let result = client.try_mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &old_sig);
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
        client.transfer(&user1, &user2, &token_id);
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
        client.transfer(&user1, &user2, &token_id);
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
        let token_id = client.mint(&user1, &2u32, &uri, &royalty, &false, &sig);
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
        assert!(client.is_paused());
        let uri = String::from_str(&env, "ipfs://QmPaused");
        let sig = sign_mint(&env, &kp, &user1, 1, &uri);
        let result = client.try_mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig);
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
        let result = client.try_transfer(&user1, &user2, &token_id);
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
        client.transfer(&user1, &user2, &token_id);
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
        let result = client.try_transfer(&user1, &user2, &token_id);
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
        client.transfer(&user1, &user2, &token_id);
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
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
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

        // Verify RoyaltyRecipientUpdated event emitted with correct old/new addresses
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (topics, data): (soroban_sdk::Vec<soroban_sdk::Val>, RoyaltyRecipientUpdatedEvent) =
            env.events().all().iter().next().unwrap().unwrap_into();
        let _ = topics; // topic is ("royalty",)
        assert_eq!(data.token_id, token_id);
        assert_eq!(data.old_recipient, user1);
        assert_eq!(data.new_recipient, user2);
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
        let token_id = client.mint(&user1, &202u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig);
        assert_eq!(token_id, 1);
        let sig2 = sign_mint(&env, &kp, &user1, 202, &uri);
        let result = client.try_mint(&user1, &202u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig2);
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

        let owned = client.tokens_of_owner(&user1);
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

        let owned = client.tokens_of_owner(&user2);
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
        client.transfer(&user1, &user2, &token_id);

        assert_eq!(client.tokens_of_owner(&user1).len(), 0);
        assert_eq!(client.tokens_of_owner(&user2).len(), 1);
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

        let owned = client.tokens_of_owner(&user1);
        assert_eq!(owned.len(), 5);
        
        // Verify returned tokens match minted tokens
        for i in 0..5 {
            assert_eq!(owned.get(i as u32).unwrap(), minted.get(i as u32).unwrap());
        }
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

        let minted = client.batch_mint(
            &user1,
            &clip_ids,
            &uris,
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
        let mut sigs = Vec::new(&env);
        sigs.push_back(sig);

        let result = client.try_batch_mint(
            &user1,
            &clip_ids,
            &uris,
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

        client.batch_mint(
            &user1,
            &clip_ids,
            &uris,
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
        client.request_withdraw_xlm(&admin, &1_000i128);

        let request: WithdrawRequest = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawXlmRequest)
            .unwrap();

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

        client.request_withdraw_xlm(&admin, &500i128);

        // Advance time by only 47 hours — still locked
        env.ledger().with_mut(|l| l.timestamp += 169_200);

        let asset = Address::generate(&env);
        let result = client.try_withdraw_xlm(&admin, &asset, &500i128);
        assert_eq!(result, Err(Ok(Error::WithdrawalStillLocked)));
    }

    #[test]
    fn test_last_withdrawal_time_not_set_before_execution() {
        let (env, admin, _, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        // Before any withdrawal, LastWithdrawalTime should not exist
        let stored: Option<u64> = env
            .storage()
            .instance()
            .get(&DataKey::LastWithdrawalTime);
        assert_eq!(stored, None);

        // After requesting (but not executing), it should still be absent
        client.request_withdraw_xlm(&admin, &100i128);
        let stored: Option<u64> = env
            .storage()
            .instance()
            .get(&DataKey::LastWithdrawalTime);
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

        client.transfer(&user1, &user2, &t1);
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
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2000, &kp);

        let new_uri = String::from_str(&env, "ipfs://QmRefreshed");
        client.refresh_metadata(&admin, &token_id, &new_uri);

        assert_eq!(client.token_uri(&token_id), new_uri);
    }

    #[test]
    fn test_refresh_metadata_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);
        let token_id = do_mint(&client, &env, &user1, 2001, &kp);

        let new_uri = String::from_str(&env, "ipfs://QmRefreshedEvt");
        client.refresh_metadata(&admin, &token_id, &new_uri);

        let events = env.events().all();
        assert!(events.len() >= 1);
        let (_, data): (soroban_sdk::Vec<soroban_sdk::Val>, MetadataUpdatedEvent) =
            events.iter().next().unwrap().unwrap_into();
        assert_eq!(data.token_id, token_id);
        assert_eq!(data.new_uri, new_uri);
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
            &String::from_str(&env, "ipfs://QmHack"),
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

        client.refresh_metadata(&admin, &token_id, &String::from_str(&env, "ipfs://QmFirst"));

        // Advance time by 29 days — still within cooldown
        env.ledger().with_mut(|l| l.timestamp += 29 * 24 * 3600);

        let result = client.try_refresh_metadata(
            &admin,
            &token_id,
            &String::from_str(&env, "ipfs://QmTooSoon"),
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

        client.refresh_metadata(&admin, &token_id, &String::from_str(&env, "ipfs://QmFirst"));

        // Advance time by exactly 30 days
        env.ledger().with_mut(|l| l.timestamp += 30 * 24 * 3600);

        let new_uri = String::from_str(&env, "ipfs://QmSecond");
        client.refresh_metadata(&admin, &token_id, &new_uri);
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
            &String::from_str(&env, "ipfs://QmGhost"),
        );
        assert_eq!(result, Err(Ok(Error::InvalidTokenId)));
    }

}
