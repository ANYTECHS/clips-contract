//! ClipCashNFT — Soroban smart contract entry point.
//!
//! This module is the single gateway for all public contract methods.
//! It re-exports types and registers the contract implementation
//! via the `#[contract]` / `#[contractimpl]` macros.

#![no_std]

mod blacklist;
mod config;
mod config_guard;
mod config_validator;
mod contract_version;
mod default_royalty;
mod migration;
mod operator_approval;
mod pause_state;
mod payment_currency;
mod platform_fee;
mod storage_constants;
mod storage_deserializer;
mod token_approval;
mod token_storage;
mod types;

pub use blacklist::{add_wallet, is_blacklisted, remove_wallet};
pub use config::{get_config, set_config, Config};
pub use contract_version::{
    get_migration_version, get_upgrade_timestamp, get_version_record, record_upgrade,
    set_version_record, ContractVersionRecord,
};
pub use migration::{is_fully_migrated, migrate_to_current, run_migrations, MigrationEvent};
pub use storage_constants::{
    BASIS_POINTS_SCALE, CONTRACT_VERSION, CURRENT_MIGRATION_VERSION,
    DEFAULT_NEXT_TOKEN_ID, DEFAULT_PAUSED, DEFAULT_PLATFORM_FEE_BPS, DEFAULT_ROYALTY_BPS,
    DEFAULT_TOTAL_SUPPLY, DEFAULT_UPGRADE_TIMESTAMP, INITIAL_MIGRATION_VERSION,
    MAX_BATCH_MINT_SIZE, MAX_BATCH_MINT_SIZE_LIMIT, MAX_COLLECTION_LIMIT, MAX_COLLECTION_SIZE,
    MAX_PLATFORM_FEE_BPS, MAX_ROYALTY_BPS, MIN_BATCH_MINT_SIZE, MIN_COLLECTION_SIZE,
};
pub use storage_deserializer::{
    deserialize_metadata, deserialize_royalty, deserialize_token, read_instance,
    read_persistent,
};

/// Compile-time contract version (alias used by upgrade/migration tests).
pub const VERSION: u32 = CONTRACT_VERSION;
pub use default_royalty::{get_default_royalty_bps, set_default_royalty_bps};
pub use operator_approval::{is_operator, remove_operator, save_operator};
pub use pause_state::{get_pause_state, save_pause_state};
pub use platform_fee::{get_platform_fee, set_platform_fee};
pub use config_guard::require_config_admin;
pub use config_validator::{
    validate_collection_limit, validate_config, validate_fee, validate_royalty_bps, validate_uri,
};
pub use payment_currency::{add_currency, get_currencies, is_supported, remove_currency};
pub use types::{
    BurnEvent, DataKey, Error, MintEvent, Royalty, RoyaltyInfo, RoyaltyPaidEvent, TokenData,
    TokenId, TransferEvent,
};

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
        config::set_config(&env, cfg, admin)
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
