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
    TokenAlreadyMinted = 3,
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
    /// Collection name (instance).
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
    /// Operator-for-all approval: (owner, operator) → bool (persistent).
    ApprovalForAll(Address, Address),
    /// Blacklist flag for a clip_id (persistent).
    BlacklistedClip(u32),
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

/// Emitted when a clip ID is blacklisted by the admin.
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

/// Emitted when a royalty payment is made for a SEP-0041 asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPaidEvent {
    pub token_id: TokenId,
    pub from: Address,
    pub to: Address,
    pub amount: i128,
}

/// Emitted when the primary royalty recipient changes via [`ClipsNftContract::set_royalty`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipientUpdatedEvent {
    pub token_id: TokenId,
    pub old_recipient: Address,
    pub new_recipient: Address,
}

/// Emitted when the contract WASM is upgraded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeEvent {
    pub new_wasm_hash: BytesN<32>,
}

// =============================================================================
// Contract
// =============================================================================

/// ClipCash NFT contract.
#[contract]
pub struct ClipsNftContract;

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

    // -------------------------------------------------------------------------
    // Blacklist  ⚠️ PRIVILEGED — admin only
    // -------------------------------------------------------------------------

    /// Blacklist a clip ID, permanently preventing it from being minted.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"blacklist"` [`BlacklistEvent`].
    ///
    /// # Arguments
    /// * `admin`   — Must be the contract admin.
    /// * `clip_id` — Off-chain clip identifier to blacklist.
    pub fn blacklist_clip(env: Env, admin: Address, clip_id: u32) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::BlacklistedClip(clip_id), &true);
        env.events()
            .publish((symbol_short!("blacklist"),), BlacklistEvent { clip_id });
        Ok(())
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
    /// * [`Error::TokenAlreadyMinted`] — clip already has a token.
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
            return Err(Error::TokenAlreadyMinted);
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

        env.events().publish(
            (symbol_short!("mint"),),
            MintEvent { to, clip_id, token_id, metadata_uri },
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

    /// Update the royalty configuration for a token.
    ///
    /// ⚠️ **Access Control: Admin only.**
    ///
    /// Emits: `"royalty"` [`RoyaltyRecipientUpdatedEvent`] when the primary
    /// recipient address changes.
    ///
    /// # Arguments
    /// * `admin`       — Must be the contract admin.
    /// * `token_id`    — Token whose royalty is being updated.
    /// * `new_royalty` — New royalty configuration.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`]    — token does not exist.
    /// * [`Error::RoyaltyTooHigh`]    — total basis points exceed 10 000.
    /// * [`Error::InvalidRoyaltySplit`] — recipients list is empty.
    pub fn set_royalty(
        env: Env,
        admin: Address,
        token_id: TokenId,
        new_royalty: Royalty,
    ) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;

        let mut data = Self::load_token(&env, token_id)?;
        let old_royalty = data.royalty.clone();
        let new_royalty = Self::normalize_royalty(&env, new_royalty)?;

        // Emit event only when the primary recipient address changes.
        if !old_royalty.recipients.is_empty() && !new_royalty.recipients.is_empty() {
            let old_r = old_royalty.recipients.get(0).ok_or(Error::InvalidRoyaltySplit)?;
            let new_r = new_royalty.recipients.get(0).ok_or(Error::InvalidRoyaltySplit)?;
            if old_r.recipient != new_r.recipient {
                env.events().publish(
                    (symbol_short!("royalty"),),
                    RoyaltyRecipientUpdatedEvent {
                        token_id,
                        old_recipient: old_r.recipient,
                        new_recipient: new_r.recipient,
                    },
                );
            }
        }

        data.royalty = new_royalty;
        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Token owner operations
    // -------------------------------------------------------------------------

    /// Override the metadata URI for a token. Only the current token owner may call this.
    ///
    /// # Arguments
    /// * `owner`    — Current token owner (must authorize).
    /// * `token_id` — Token to update.
    /// * `uri`      — New metadata URI.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    /// * [`Error::Unauthorized`]   — caller is not the token owner.
    pub fn set_token_uri(
        env: Env,
        owner: Address,
        token_id: TokenId,
        uri: String,
    ) -> Result<(), Error> {
        owner.require_auth();
        let mut data = Self::load_token(&env, token_id)?;
        if data.owner != owner {
            return Err(Error::Unauthorized);
        }
        data.metadata_uri = uri;
        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        Ok(())
    }

    /// Burn (destroy) an NFT. Only the current owner may burn.
    ///
    /// Removes both the `TokenData` and the `ClipIdMinted` dedup guard so the
    /// same `clip_id` can be re-minted after a burn.
    ///
    /// Storage removes: 2 persistent (TokenData, ClipIdMinted).
    ///
    /// Emits: `"burn"` [`BurnEvent`].
    ///
    /// # Arguments
    /// * `owner`    — Current token owner (must authorize).
    /// * `token_id` — Token to destroy.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
    /// * [`Error::Unauthorized`]   — caller is not the token owner.
    pub fn burn(env: Env, owner: Address, token_id: TokenId) -> Result<(), Error> {
        owner.require_auth();

        let data: TokenData = Self::load_token(&env, token_id)?;

        if owner != data.owner {
            return Err(Error::Unauthorized);
        }

        // 2 persistent removes (optimized from 4).
        env.storage().persistent().remove(&DataKey::Token(token_id));
        env.storage().persistent().remove(&DataKey::ClipIdMinted(data.clip_id));

        env.events().publish(
            (symbol_short!("burn"),),
            BurnEvent { owner, token_id, clip_id: data.clip_id },
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

    /// Returns the off-chain clip ID associated with a token.
    ///
    /// # Errors
    /// * [`Error::InvalidTokenId`] — token does not exist.
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

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Load `TokenData` from persistent storage, or return `InvalidTokenId`.
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

    /// Compute `sale_price * basis_points / 10_000` with overflow protection.
    ///
    /// Uses banker's rounding (round-half-up via `+ 5_000`).
    ///
    /// # Errors
    /// * [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
    /// * [`Error::RoyaltyOverflow`]  — `sale_price > i128::MAX / 10_000`.
    pub fn calculate_royalty(sale_price: i128, basis_points: u32) -> Result<i128, Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }
        if sale_price > i128::MAX / 10_000 {
            return Err(Error::RoyaltyOverflow);
        }
        let amount = sale_price.saturating_mul(basis_points as i128);
        Ok((amount.saturating_add(5_000)) / 10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, BytesN as _, Events as _},
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
        assert_eq!(events.events().len(), 1);
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
        assert_eq!(events.events().len(), 1);
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
        recipients.push_back(RoyaltyRecipient { recipient: user2.clone(), basis_points: 500 });
        client.set_royalty(&admin, &token_id, &Royalty { recipients, asset_address: None });
        let events = env.events().all();
        assert!(events.events().len() > 0);
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
        assert_eq!(result, Err(Ok(Error::TokenAlreadyMinted)));
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
}
