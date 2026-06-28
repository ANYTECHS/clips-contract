//! Token metadata storage — save, retrieve, and update NFT metadata URIs.
//!
//! Validation pipeline applied on every write:
//!   1. URI format check     (`metadata_uri_validator`, Issue #561)
//!   2. URI size check       (`metadata_size`,          Issue #560)
//!   3. Update-policy check  (`metadata_update_policy`, Issue #562)
//!
//! A `MetadataUpdatedEvent` is emitted after every successful update (#563).
//!
//! # Storage
//! Key: `DataKey::Metadata(token_id)` (persistent storage)

use soroban_sdk::{Address, Env, String};

use crate::metadata_size::validate_metadata_size;
use crate::metadata_update_policy::{check_update_allowed, mark_update_used};
use crate::metadata_uri_validator::validate_metadata_uri;
use crate::types::{DataKey, Error, MetadataUpdatedEvent, TokenId};

/// Persist the metadata URI for `token_id` during minting.
///
/// Applies format (#561) and size (#560) validation. Does **not** enforce
/// the update policy (minting is an initial write, not an update).
pub fn save_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_metadata_uri(uri)?;
    validate_metadata_size(uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    Ok(())
}

/// Load the metadata URI for `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Update the metadata URI for `token_id`.
///
/// Full validation pipeline:
///   1. Token must already exist (metadata previously set).
///   2. URI format validated (#561).
///   3. URI size validated (#560).
///   4. Update policy checked: one-time update for non-admins, admin override (#562).
///   5. `MetadataUpdatedEvent` emitted with previous URI, new URI, and updater (#563).
pub fn update_metadata(
    env: &Env,
    token_id: TokenId,
    new_uri: &String,
    caller: &Address,
) -> Result<(), Error> {
    let previous_uri: String = env
        .storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)?;

    validate_metadata_uri(new_uri)?;
    validate_metadata_size(new_uri)?;
    check_update_allowed(env, token_id, caller)?;

    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), new_uri);

    // Mark one-time slot as used (no-op for admin; admin bypass is checked inside policy).
    mark_update_used(env, token_id);

    // Emit event (#563).
    env.events().publish(
        (soroban_sdk::symbol_short!("meta_upd"),),
        MetadataUpdatedEvent {
            token_id,
            previous_uri,
            new_uri: new_uri.clone(),
            updater: caller.clone(),
        },
    );

    Ok(())
}
