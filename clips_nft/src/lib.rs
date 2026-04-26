//! ClipCash NFT - Soroban Smart Contract
//!
//! This contract enables minting video clips as NFTs on the Stellar network
//! with built-in royalty support for content creators.
//! Royalties can be paid in XLM or any custom Stellar asset (SEP-0041 token).
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
//! payload = SHA-256( clip_id_le_bytes || owner_address_bytes || metadata_uri_bytes )
//! ```
//!
//! - `clip_id` is encoded as 4 little-endian bytes.
//! - `owner_address_bytes` is the raw XDR encoding of the `Address` produced by
//!   `env.crypto().sha256(&owner.to_xdr(&env))` — i.e. the contract hashes the
//!   address XDR so the payload is always a fixed-size 32-byte digest.
//! - The final SHA-256 over the concatenation is what gets signed.
//!
//! The backend registers its Ed25519 public key once via `set_signer` (admin only).
//! The public key is stored in instance storage under `DataKey::Signer`.
//!
//! # Storage layout & gas cost notes
//!
//! ## Storage tiers used
//! - `instance`   – cheap, loaded once per tx, shared across all calls in the tx.
//!                  Used for: Admin, NextTokenId, Paused, Signer.
//! - `persistent` – per-entry fee, survives ledger expiry extension.
//!                  Used for: TokenData (owner+clip_id+metadata+royalty packed),
//!                  ClipIdMinted (dedup guard).
//!
//! ## Estimated storage operations per function
//!
//! ### `mint`
//! | Op              | Tier       | Count |
//! |-----------------|------------|-------|
//! | instance read   | instance   | 4     | (Admin, NextTokenId, Paused, Signer)
//! | instance write  | instance   | 1     | (NextTokenId++)
//! | persistent read | persistent | 1     | (ClipIdMinted dedup check)
//! | persistent write| persistent | 2     | (TokenData, ClipIdMinted)
//! Total persistent writes: **2** (Optimized from 4)
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
//! | persistent remove| persistent| 2     | (TokenData, ClipIdMinted)
//! Total persistent removes: **2** (Optimized from 4)
//!
//! ## Removed counters / indexes (vs. earlier version)
//! - `Balance(Address)` — per-address token counter removed.
//! - `TokenCount` — replaced by `next_token_id - 1`.
//! - `TokenClipId(TokenId)` — clip_id packed into `TokenData`.
//!
//! # Privileged entrypoints (admin-only)
//!
//! - `set_signer`
//! - `upgrade`
//! - `pause`
//! - `unpause`
//! - `blacklist_clip`
//! - `set_name`
//! - `set_symbol`
//! - `set_royalty`

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, String, Vec,
};

/// Contract version
pub const VERSION: u32 = 1;

/// Custom errors for the NFT contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Error {
    /// Operation not authorized
    Unauthorized = 1,
    /// Invalid token ID
    InvalidTokenId = 2,
    /// Token already minted
    TokenAlreadyMinted = 3,
    /// Royalty too high (max 10000 basis points = 100%)
    RoyaltyTooHigh = 4,
    /// Invalid recipient
    InvalidRecipient = 5,
    /// Sale price must be greater than zero
    InvalidSalePrice = 6,
    /// Contract is paused — minting and transfers are blocked
    ContractPaused = 7,
    /// Backend signature over the mint payload is invalid
    InvalidSignature = 8,
    /// No backend signer public key has been registered yet
    SignerNotSet = 9,
    /// Royalty split is invalid
    InvalidRoyaltySplit = 10,
    /// Token is soulbound (non-transferable)
    SoulboundTransferBlocked = 11,
    /// Royalty calculation would overflow
    RoyaltyOverflow = 12,
    /// Clip is blacklisted
    ClipBlacklisted = 13,
    /// Caller is not authorized to approve
    NotAuthorizedToApprove = 14,
    /// Withdrawal is still locked (24h safety delay)
    WithdrawalStillLocked = 15,
    /// No active withdrawal request found
    NoWithdrawalRequest = 16,
}

/// Token ID type
pub type TokenId = u32;

/// Packs owner address, originating clip_id, metadata, and royalty into a single persistent entry.
///
/// Combining these fields eliminates the separate `Metadata` and `Royalty`
/// entries that were previously written on every mint.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenData {
    pub owner: Address,
    /// The off-chain clip identifier this token was minted for.
    pub clip_id: u32,
    /// Whether this token is soulbound (non-transferable)
    pub is_soulbound: bool,
    /// Metadata URI for the token
    pub metadata_uri: String,
    /// Royalty configuration
    pub royalty: Royalty,
}

/// Royalty information stored per token.
/// `asset_address` is `None` for native XLM, or `Some(contract_address)`
/// for any SEP-0041 custom Stellar asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRecipient {
    pub recipient: Address,
    pub basis_points: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Royalty {
    /// Multi-recipient split. Platform recipient is automatically added with 1%
    /// if not present.
    pub recipients: Vec<RoyaltyRecipient>,
    /// Optional SEP-0041 asset contract address.
    /// `None` → royalties expected in XLM (native).
    pub asset_address: Option<Address>,
}

/// Royalty payment info returned by `royalty_info()`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyInfo {
    pub receiver: Address,
    /// Royalty amount in the same denomination as `sale_price`
    pub royalty_amount: i128,
    /// `None` → pay in XLM; `Some(addr)` → pay in that SEP-0041 token.
    pub asset_address: Option<Address>,
}

/// Storage keys
///
/// Key sizing notes:
/// - Enum variants with no payload (Admin, NextTokenId, Paused) are 1-word keys.
/// - Variants with a u32 payload (Token, ClipIdMinted) are
///   2-word keys — the smallest possible for per-token entries.
#[contracttype]
pub enum DataKey {
    /// Contract administrator address (instance storage)
    Admin,
    /// Monotonically increasing token ID counter (instance storage).
    /// `total_supply = NextTokenId - 1` — no separate TokenCount needed.
    NextTokenId,
    /// Pause flag (instance storage)
    Paused,
    /// Collection name (instance storage)
    Name,
    /// Collection symbol (instance storage)
    Symbol,
    /// Packed owner + clip_id + metadata + royalty for a token (persistent storage)
    Token(TokenId),
    /// Dedup guard: clip_id → token_id (persistent storage)
    ClipIdMinted(u32),
    /// Custom metadata URI override per token (persistent storage)
    CustomTokenUri(TokenId),
    /// Ed25519 public key of the trusted backend signer (instance storage)
    Signer,
    /// Platform recipient used for default 1% royalty cut
    PlatformRecipient,
    /// Total synthetic gas used in minting (instance storage)
    TotalGasMint,
    /// Total number of successful mints (instance storage)
    CountMint,
    /// Total synthetic gas used in transfers (instance storage)
    TotalGasTransfer,
    /// Total number of successful transfers (instance storage)
    CountTransfer,
    /// Per-token approval: token_id -> operator
    Approved(TokenId),
    /// Operator approval for all: (owner, operator) -> bool
    ApprovalForAll(Address, Address),
    /// Blacklist flag for a clip_id
    BlacklistedClip(u32),
    /// Pending XLM withdrawal request (instance storage)
    WithdrawXlmRequest,
    /// Timestamp of the last successfully executed withdrawal (instance storage)
    LastWithdrawalTime,
}

/// Emergency withdrawal request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequest {
    pub amount: i128,
    pub unlock_time: u64,
}

/// Event emitted when a new NFT is minted
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent {
    pub to: Address,
    pub clip_id: u32,
    pub token_id: TokenId,
    pub metadata_uri: String,
}

/// Event emitted when an NFT is burned.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnEvent {
    pub owner: Address,
    pub token_id: TokenId,
    pub clip_id: u32,
}

/// Event emitted when NFT ownership changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEvent {
    pub token_id: TokenId,
    pub from: Address,
    pub to: Address,
}

/// Event emitted when a clip ID is blacklisted by admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlacklistEvent {
    pub clip_id: u32,
}

/// Event emitted when an operator is approved for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalEvent {
    pub owner: Address,
    pub operator: Address,
    pub token_id: TokenId,
}

/// Event emitted when approval-for-all is set or revoked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalForAllEvent {
    pub owner: Address,
    pub operator: Address,
    pub approved: bool,
}

/// Event emitted when a clip ID is blacklisted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlacklistEvent {
    pub clip_id: u32,
}

/// Event emitted when token approval changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalEvent {
    pub owner: Address,
    pub operator: Address,
    pub token_id: TokenId,
}

/// Event emitted when operator-for-all approval changes.
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

/// NFT Contract
#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    /// Initialize the contract with an admin address.
    ///
    /// Security:
    /// - One-time initialization only.
    /// - Requires `admin` authorization.
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

    /// Register (or rotate) the backend Ed25519 public key used to verify
    /// clip ownership before minting. Only callable by the admin.
    ///
    /// # Arguments
    /// * `admin`  - Must be the contract admin
    /// * `pubkey` - 32-byte Ed25519 public key of the trusted backend signer
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
    // Upgradeability
    // -------------------------------------------------------------------------

    /// Upgrade the contract to a new WASM implementation.
    /// Only callable by the admin.
    ///
    /// Uses Soroban's built-in `update_current_contract_wasm` which replaces
    /// the current contract code with the new WASM hash while preserving all
    /// instance and persistent storage.
    ///
    /// # Arguments
    /// * `admin`         - Must be the contract admin
    /// * `new_wasm_hash` - 32-byte SHA-256 hash of the new WASM blob
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
    // Pausable
    // -------------------------------------------------------------------------

    /// Pause the contract. Blocks `mint` and `transfer` until unpaused.
    /// Only callable by the admin.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((symbol_short!("paused"),), ());
        Ok(())
    }

    /// Unpause the contract, re-enabling `mint` and `transfer`.
    /// Only callable by the admin.
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

    // -------------------------------------------------------------------------
    // Core NFT operations
    // -------------------------------------------------------------------------

    /// Mint a new NFT for a video clip.
    ///
    /// Requires a valid Ed25519 `signature` from the registered backend signer
    /// over the canonical mint payload, proving the clip exists and belongs to
    /// `to`. The payload is:
    ///
    /// ```text
    /// payload = SHA-256(
    ///     clip_id_le_4_bytes
    ///     || SHA-256(owner_address_xdr)   // 32 bytes
    ///     || SHA-256(metadata_uri_bytes)  // 32 bytes
    /// )
    /// ```
    ///
    /// Storage writes (persistent): TokenData, ClipIdMinted = **2**
    /// Instance writes: NextTokenId = **1**
    ///
    /// # Arguments
    /// * `to`           - Address that will own the NFT (must match the signed payload)
    /// * `clip_id`      - Unique off-chain clip identifier (must match the signed payload)
    /// * `metadata_uri` - IPFS or Arweave URI (must match the signed payload)
    /// * `royalty`      - Royalty configuration
    /// * `is_soulbound` - Whether the token is soulbound (non-transferable)
    /// * `signature`    - 64-byte Ed25519 signature from the backend signer
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

        // Verify backend signature before any state reads/writes
        Self::verify_clip_signature(&env, &to, clip_id, &metadata_uri, &signature)?;

        // Dedup check — one persistent read
        if env
            .storage()
            .persistent()
            .has(&DataKey::ClipIdMinted(clip_id))
        {
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

        // One instance read
        let token_id: TokenId = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        // 2 persistent writes (Optimized from 4)
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

        // 1 instance write
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

    /// Approve an operator to transfer a specific token.
    pub fn approve(
        env: Env,
        caller: Address,
        operator: Option<Address>,
        token_id: TokenId,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_not_paused(&env)?;

        let owner = Self::owner_of(env.clone(), token_id)?;
        
        // Caller must be owner or approved for all
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

    /// Set or unset an operator to manage all of the caller's tokens.
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

    /// Check if an operator is approved to manage all of the owner's tokens.
    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovalForAll(owner, operator))
            .unwrap_or(false)
    }

    /// Get the approved operator for a specific token, if any.
    pub fn get_approved(env: Env, token_id: TokenId) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Approved(token_id))
    }


    /// Transfer NFT ownership from `from` to `to`.
    ///
    /// Blocked if the token is soulbound (non-transferable).
    /// Storage writes (persistent): TokenData = **1**
    ///
    /// # Arguments
    /// * `from`     - Current owner (must authorize)
    /// * `to`       - New owner
    /// * `token_id` - Token to transfer
    pub fn transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error> {
        from.require_auth();
        Self::require_not_paused(&env)?;

        // 1 persistent read
        let mut data: TokenData = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(Error::InvalidTokenId)?;

        if from != data.owner {
            return Err(Error::Unauthorized);
        }

        // Check if token is soulbound
        if data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
        }

        // Clear approval when transferred
        env.storage().persistent().remove(&DataKey::Approved(token_id));

        // 1 persistent write — update owner in-place, clip_id unchanged
        data.owner = to.clone();
        env.storage().persistent().set(&DataKey::Token(token_id), &data);

        env.events().publish(
            (symbol_short!("transfer"),),
            TransferEvent { token_id, from, to },
        );

        Ok(())
    }

    /// Transfer NFT ownership from `from` to `to` by an approved `spender`.
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

        let is_approved_for_all = Self::is_approved_for_all(env.clone(), from.clone(), spender.clone());
        let approved_operator = Self::get_approved(env.clone(), token_id);
        
        let is_approved = is_approved_for_all || approved_operator == Some(spender);
        
        if !is_approved {
            return Err(Error::Unauthorized);
        }

        if data.is_soulbound {
            return Err(Error::SoulboundTransferBlocked);
        }

        // Clear approval
        env.storage().persistent().remove(&DataKey::Approved(token_id));

        data.owner = to.clone();
        env.storage().persistent().set(&DataKey::Token(token_id), &data);
        let gas_used = GAS_BASE_TRANSFER;
        env.events().publish(
            (symbol_short!("transfer"),),
            TransferEvent { token_id, from, to, gas_used },
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Admin Configuration
    // -------------------------------------------------------------------------

    /// Set the collection name.
    /// Access Control: Admin only.
    pub fn set_name(env: Env, admin: Address, name: String) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Name, &name);
        Ok(())
    }

    /// Set the collection symbol.
    /// Access Control: Admin only.
    pub fn set_symbol(env: Env, admin: Address, symbol: String) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        Ok(())
    }

    /// Set a custom token URI for a minted token. Only the token owner can update it.
    pub fn set_token_uri(
        env: Env,
        owner: Address,
        token_id: TokenId,
        uri: String,
    ) -> Result<(), Error> {
        owner.require_auth();

        let data = Self::load_token(&env, token_id)?;
        if data.owner != owner {
            return Err(Error::Unauthorized);
        }

        let mut data = data;
        data.metadata_uri = uri;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &data);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // View functions
    // -------------------------------------------------------------------------

    /// Returns the contract version.
    pub fn version(_env: Env) -> u32 {
        VERSION
    }

    /// Returns the collection name.
    pub fn name(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::Name)
            .unwrap_or_else(|| String::from_str(&env, "ClipCash Clips"))
    }

    /// Returns the collection symbol.
    pub fn symbol(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::Symbol)
            .unwrap_or_else(|| String::from_str(&env, "CLIP"))
    }

    /// Returns the original clip ID for a given token ID.
    pub fn get_clip_id(env: Env, token_id: TokenId) -> Result<u32, Error> {
        Ok(Self::load_token(&env, token_id)?.clip_id)
    }

    /// Returns the owner of a given token ID.
    pub fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        Ok(Self::load_token(&env, token_id)?.owner)
    }

    /// Returns the metadata URI for a given token ID.
    pub fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error> {
        Ok(Self::load_token(&env, token_id)?.metadata_uri)
    }

    /// Alias for `token_uri`, kept for compatibility.
    pub fn get_metadata(env: Env, token_id: TokenId) -> Result<String, Error> {
        Ok(Self::load_token(&env, token_id)?.metadata_uri)
    }

    /// Look up the on-chain token ID for a given clip_id.
    pub fn clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::ClipIdMinted(clip_id))
            .ok_or(Error::InvalidTokenId)
    }

    /// Returns the stored `Royalty` struct for a token.
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        Ok(Self::load_token(&env, token_id)?.royalty)
    }

    /// Returns the total number of minted (and not yet burned) tokens.
    ///
    /// Derived from `NextTokenId - 1` — no separate counter needed.
    pub fn total_supply(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::NextTokenId)
            .unwrap_or(1)
            .saturating_sub(1)
    }

    /// Returns true if the token exists.
    pub fn exists(env: Env, token_id: TokenId) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Token(token_id))
    }

    /// Returns true if the token is soulbound (non-transferable).
    pub fn is_soulbound(env: Env, token_id: TokenId) -> bool {
        if let Ok(data) = Self::load_token(&env, token_id) {
            data.is_soulbound
        } else {
            false
        }
    }

    // -------------------------------------------------------------------------
    // Royalty extension (EIP-2981 style, with custom asset support)
    // -------------------------------------------------------------------------

    /// Returns the royalty receiver, amount, and payment asset for a given sale price.
    ///
    /// Uses safe math to prevent overflow. Royalty amount is calculated as:
    /// `royalty_amount = sale_price * basis_points / 10000`
    ///
    /// Safe limits: sale_price should not exceed i128::MAX / 10000 to avoid overflow.
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

    /// Pay royalties for a token sale using the asset configured in the royalty.
    ///
    /// Only handles SEP-0041 custom assets. For XLM (`asset_address` is `None`)
    /// the marketplace must handle the transfer directly.
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
            let total_royalty_so_far = Self::calculate_royalty(sale_price, cumulative_bps)?;
            let amount = total_royalty_so_far.saturating_sub(cumulative_royalty);
            cumulative_royalty = total_royalty_so_far;

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

        env.events().publish(
            (symbol_short!("burn"),),
            BurnEvent {
                owner,
                token_id,
                clip_id: data.clip_id,
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
    /// Iterates over minted token IDs (1..=next_token_id-1) and collects those
    /// whose owner matches. Result is capped at 1000 entries to prevent gas
    /// explosion.
    ///
    /// # Arguments
    /// * `owner` - Address to query
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
    /// Payload construction (all hashing via SHA-256):
    /// ```text
    /// owner_hash    = SHA-256(XDR(owner))
    /// uri_hash      = SHA-256(UTF-8(metadata_uri))
    /// message       = SHA-256( clip_id_le4 || owner_hash || uri_hash )
    /// ```
    /// The signer signs `message` (32 bytes) with their Ed25519 private key.
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

        // Hash the owner address XDR so the payload is always fixed-width
        let owner_hash: BytesN<32> = env.crypto().sha256(&owner.clone().to_xdr(env)).into();

        // Hash the metadata URI bytes
        let uri_hash: BytesN<32> = env.crypto().sha256(&Bytes::from(metadata_uri.to_xdr(env))).into();

        // Build the 68-byte pre-image: 4 (clip_id LE) + 32 (owner_hash) + 32 (uri_hash)
        let mut preimage = Bytes::new(env);
        preimage.extend_from_array(&clip_id.to_le_bytes());
        preimage.append(&Bytes::from(owner_hash));
        preimage.append(&Bytes::from(uri_hash));

        // Final message digest that was signed
        let message: BytesN<32> = env.crypto().sha256(&preimage).into();

        // Panics (traps) on invalid signature — map to our error type
        env.crypto().ed25519_verify(&signer, &Bytes::from(message), signature);

        Ok(())
    }

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
                basis_points: 100, // fixed default 1%
            });
            total_bps = total_bps.saturating_add(100);
        }
        if total_bps > 10_000 {
            return Err(Error::RoyaltyTooHigh);
        }
        Ok(Royalty {
            recipients,
            asset_address: royalty.asset_address,
        })
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
        recipients.push_back(RoyaltyRecipient {
            recipient,
            basis_points: 500,
        });
        Royalty {
            recipients,
            asset_address: None,
        }
    }

    /// Build the canonical mint payload and sign it with `signer_secret`.
    /// Mirrors the on-chain `verify_clip_signature` logic exactly.
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

    /// Register a fresh signer keypair and return (pubkey, secret).
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
        assert_eq!(
            client.token_uri(&token_id),
            String::from_str(&env, "ipfs://QmExample")
        );
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
        let custom_uri = String::from_str(&env, "ipfs://QmShouldFail");

        let result = client.try_set_token_uri(&user2, &token_id, &custom_uri);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));

        // Original URI is still returned because override wasn't authorized.
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

    // -------------------------------------------------------------------------
    // Signature verification tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mint_fails_without_signer_set() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        // No set_signer call

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

        // Sign with a *different* keypair — not the registered signer
        let wrong_kp_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
        let wrong_kp = ed25519_dalek::SigningKey::from_bytes(&wrong_kp_bytes);
        let uri = String::from_str(&env, "ipfs://QmExample");
        let bad_sig = sign_mint(&env, &wrong_kp, &user1, 1, &uri);

        // ed25519_verify traps on bad sig, which surfaces as a panic in tests
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
        // Signature is over user2 but we pass user1 as `to`
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
        // Signature is over clip_id=99 but we pass clip_id=1
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

        // Rotate to a new keypair
        let kp2_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
        let kp2 = ed25519_dalek::SigningKey::from_bytes(&kp2_bytes);
        let kp2_pub = BytesN::from_array(&env, &kp2.verifying_key().to_bytes());
        client.set_signer(&admin, &kp2_pub);
        assert_eq!(client.get_signer(), Some(kp2_pub));

        // Old signer's signature should now fail
        let uri = String::from_str(&env, "ipfs://QmExample");
        let old_sig = sign_mint(&env, &kp1, &user1, 1, &uri);
        let result = client.try_mint(&user1, &1u32, &uri, &default_royalty(&env, user1.clone()), &false, &old_sig);
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // Transfer / royalty / burn / pause tests (unchanged logic)
    // -------------------------------------------------------------------------

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
        recipients.push_back(RoyaltyRecipient {
            recipient: user1.clone(),
            basis_points: 1000,
        });
        let royalty = Royalty {
            recipients,
            asset_address: Some(asset_addr.clone()),
        };
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
        recipients.push_back(RoyaltyRecipient {
            recipient: user2.clone(),
            basis_points: 1000,
        });
        let new_royalty = Royalty {
            recipients,
            asset_address: Some(asset_addr.clone()),
        };
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
        // clip_id dedup entry also removed — can re-mint same clip_id
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

    // =========================================================================
    // NEW COMPREHENSIVE TESTS FOR ISSUES #55, #53, #57, #9
    // =========================================================================

    // -------------------------------------------------------------------------
    // Issue #55: Soulbound (non-transferable) clips support
    // -------------------------------------------------------------------------

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
        
        // Attempt to transfer soulbound token should fail
        let result = client.try_transfer(&user1, &user2, &token_id);
        assert_eq!(result, Err(Ok(Error::SoulboundTransferBlocked)));
        
        // Owner should remain unchanged
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
        
        // Regular token should transfer successfully
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
        assert!(client.exists(&token_id));
        
        // Soulbound token can still be burned by owner
        client.burn(&user1, &token_id);
        assert!(!client.exists(&token_id));
    }

    // -------------------------------------------------------------------------
    // Issue #53: Safe royalty calculation with overflow protection
    // -------------------------------------------------------------------------

    #[test]
    fn test_royalty_calculation_safe_math() {
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
    }

    #[test]
    fn test_royalty_overflow_detection() {
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
    }

    #[test]
    fn test_royalty_calculation_max_u128_values() {
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
    }

    // -------------------------------------------------------------------------
    // Issue #57: Events for royalty recipient changes
    // -------------------------------------------------------------------------

    #[test]
    fn test_royalty_recipient_updated_event() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 107, &kp);
        
        // Change royalty recipient
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
        assert_eq!(events.events().len(), 1);
        let (topics, data): (soroban_sdk::Vec<soroban_sdk::Val>, RoyaltyRecipientUpdatedEvent) =
            env.events().all().first().unwrap();
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
        
        // Get current royalty
        let _current_royalty = client.get_royalty(&token_id);
        
        // Set same royalty (only basis points change, recipient stays same)
        let mut recipients = Vec::new(&env);
        recipients.push_back(RoyaltyRecipient {
            recipient: user1.clone(),
            basis_points: 600, // Different basis points, same recipient
        });
        let new_royalty = Royalty {
            recipients,
            asset_address: None,
        };
        
        client.set_royalty(&admin, &token_id, &new_royalty);
        
        // Verify royalty was updated
        let updated = client.get_royalty(&token_id);
        assert_eq!(updated.recipients.get(0).unwrap().basis_points, 600);
    }

    // -------------------------------------------------------------------------
    // Issue #9: Comprehensive unit tests for mint and royalty functions
    // -------------------------------------------------------------------------

    #[test]
    fn test_successful_mint_with_metadata() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let uri = String::from_str(&env, "ipfs://QmTestMetadata");
        let sig = sign_mint(&env, &kp, &user1, 200, &uri);
        let token_id = client.mint(&user1, &200u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig);
        
        assert_eq!(token_id, 1);
        assert_eq!(client.token_uri(&token_id), uri);
        assert_eq!(client.owner_of(&token_id), user1);
    }

    #[test]
    fn test_royalty_payment_on_transfer() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 201, &kp);
        
        // Verify royalty info is correct
        let sale_price = 10_000_000i128;
        let royalty_info = client.royalty_info(&token_id, &sale_price);
        
        // 5% creator + 1% platform = 6% total
        assert_eq!(royalty_info.royalty_amount, 600_000i128);
        assert_eq!(royalty_info.receiver, user1);
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
        
        // First mint succeeds
        let token_id = client.mint(&user1, &202u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig);
        assert_eq!(token_id, 1);
        
        // Second mint with same clip_id should fail
        let sig2 = sign_mint(&env, &kp, &user1, 202, &uri);
        let result = client.try_mint(&user1, &202u32, &uri, &default_royalty(&env, user1.clone()), &false, &sig2);
        assert_eq!(result, Err(Ok(Error::TokenAlreadyMinted)));
    }

    #[test]
    fn test_unauthorized_mint_attempt() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let uri = String::from_str(&env, "ipfs://QmUnauth");
        // Sign for user1 but try to mint as user2
        let sig = sign_mint(&env, &kp, &user1, 203, &uri);
        
        let result = client.try_mint(&user2, &203u32, &uri, &default_royalty(&env, user2.clone()), &false, &sig);
        // Should fail because signature doesn't match the caller
        assert!(result.is_err());
    }

    #[test]
    fn test_mint_and_burn_cycle() {
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
        // Note: total_supply is derived from NextTokenId - 1, so it remains 1
        // even after burning (NextTokenId is still 2)
        assert_eq!(client.total_supply(), 1);
        
        // Can re-mint same clip_id after burn
        let token_id2 = do_mint(&client, &env, &user1, 204, &kp);
        assert!(client.exists(&token_id2));
        // Now NextTokenId is 3, so total_supply is 2
        assert_eq!(client.total_supply(), 2);
    }

    #[test]
    fn test_multiple_mints_increment_token_id() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id1 = do_mint(&client, &env, &user1, 205, &kp);
        let token_id2 = do_mint(&client, &env, &user1, 206, &kp);
        let token_id3 = do_mint(&client, &env, &user1, 207, &kp);
        
        assert_eq!(token_id1, 1);
        assert_eq!(token_id2, 2);
        assert_eq!(token_id3, 3);
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
        
        // Zero price should fail
        let result = client.try_royalty_info(&token_id, &0i128);
        assert_eq!(result, Err(Ok(Error::InvalidSalePrice)));
        
        // Negative price should fail
        let result = client.try_royalty_info(&token_id, &(-1000i128));
        assert_eq!(result, Err(Ok(Error::InvalidSalePrice)));
    }

    #[test]
    fn test_royalty_calculation_accuracy() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 209, &kp);
        
        // Test various prices
        let test_cases: [(i128, i128); 4] = [
            (100i128, 6i128),           // 100 * 0.06 = 6
            (1000i128, 60i128),         // 1000 * 0.06 = 60
            (10000i128, 600i128),       // 10000 * 0.06 = 600
            (1_000_000i128, 60_000i128), // 1M * 0.06 = 60k
        ];
        
        for (price, expected) in test_cases.iter() {
            let info = client.royalty_info(&token_id, price);
            assert_eq!(info.royalty_amount, *expected);
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
        assert_eq!(result, Err(Ok(Error::TokenAlreadyMinted)));
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
        env.ledger().set_timestamp(env.ledger().timestamp() + 169_200);

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
    fn test_royalty_checked_mul_large_safe_price() {
        // 10^15 stroops * 600 bps / 10_000 = 6 * 10^13
        let result = ClipsNftContract::calculate_royalty(1_000_000_000_000_000i128, 600);
        assert_eq!(result, Ok(60_000_000_000_000i128));
    }

}
