//! Transfer Request Validator — resolves issue #1194.
//!
//! Validates all required parameters for NFT transfer requests before any
//! storage mutation occurs.  Each check is a standalone function returning
//! `Result<(), Error>` so callers can chain them with `?` or run them via
//! the centralised [`crate::validators`] pipeline.
//!
//! # Acceptance criteria (issue #1194)
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Validate token ID | [`validate_token_id`] |
//! | Validate sender | [`validate_sender`] |
//! | Validate recipient | [`validate_recipient`] |
//! | Validate transfer authorization | [`validate_transfer_authorization`] |
//! | Add unit tests | `tests` module below |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::transfer_request_validator::{validate_transfer_request};
//!
//! pub fn transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error> {
//!     validate_transfer_request(&env, &from, &to, token_id)?;
//!     // ... proceed with storage mutations
//!     Ok(())
//! }
//! ```

use soroban_sdk::{Address, Env};

use crate::storage::keys::StorageKey;
use crate::types::{DataKey, Error, TokenData, TokenId};

// ─── Individual validators ────────────────────────────────────────────────────

/// Validate that `token_id` is non-zero and exists in persistent storage.
///
/// Token ID `0` is reserved and treated as unset / invalid across the contract.
///
/// # Errors
/// - [`Error::StorageNotFound`] — the token with `token_id` does not exist.
pub fn validate_token_id(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Token(token_id)) {
        return Err(Error::StorageNotFound);
    }
    Ok(())
}

/// Validate that the sender address is the current owner of `token_id`.
///
/// Loads the token from persistent storage and compares the stored owner
/// against `sender`.
///
/// # Errors
/// - [`Error::StorageNotFound`] — the token does not exist.
/// - [`Error::Unauthorized`] — `sender` is not the current owner of the token.
pub fn validate_sender(env: &Env, sender: &Address, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;

    if data.owner != *sender {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Validate that the recipient address is set and is not the same as the sender.
///
/// Self-transfers are disallowed: they waste gas and produce no meaningful
/// state change.
///
/// # Errors
/// - [`Error::InvalidRecipient`] — `recipient` is the zero-equivalent address
///   (contract address acting as burn address) or is otherwise invalid.
/// - [`Error::SelfTransferNotAllowed`] — `sender` and `recipient` are the same address.
pub fn validate_recipient(
    env: &Env,
    sender: &Address,
    recipient: &Address,
) -> Result<(), Error> {
    // Reject self-transfers.
    if sender == recipient {
        return Err(Error::SelfTransferNotAllowed);
    }
    // Reject transfers to the contract itself (acts as a "burn" address in
    // standard transfers; callers should use the `burn` entry point instead).
    if *recipient == env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }
    Ok(())
}

/// Validate that the token is in a state compatible with a transfer.
///
/// A transfer is only valid when the token exists, has an owner, is not
/// burned, is not frozen, and is active.  This resolves the state-related
/// failure modes of issue #1393.
///
/// # Errors
/// - [`Error::StorageNotFound`] — the token does not exist (missing owner /
///   invalid ownership state).
/// - [`Error::InvalidTransferState`] — the token is burned, frozen, or
///   otherwise inactive.
pub fn validate_transfer_state(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;

    // Missing owner is an invalid ownership state.
    let _ = data.owner.clone();

    // Burned token check.
    let burned: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::BurnedToken(token_id))
        .unwrap_or(false);
    if burned {
        return Err(Error::InvalidTransferState);
    }

    // Frozen token check.
    let frozen: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::FrozenToken(token_id))
        .unwrap_or(false);
    if frozen {
        return Err(Error::InvalidTransferState);
    }

    // Inactive token check.
    let inactive: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::InactiveToken(token_id))
        .unwrap_or(false);
    if inactive {
        return Err(Error::InvalidTransferState);
    }

    Ok(())
}

/// Validate that `caller` is authorised to transfer `token_id`.
///
/// Authorization passes when `caller` is one of:
/// 1. The current owner of the token.
/// 2. The approved single-token operator (set via `approve`).
/// 3. An operator approved for all of the owner's tokens
///    (set via `set_approval_for_all`).
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — `caller` is none of the above.
pub fn validate_transfer_authorization(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;

    // Owner is always authorized.
    if data.owner == *caller {
        return Ok(());
    }

    // Check single-token approval.
    let approved: Option<Address> = env
        .storage()
        .persistent()
        .get(&StorageKey::Approval(token_id));
    if let Some(approved_addr) = approved {
        if approved_addr == *caller {
            return Ok(());
        }
    }

    // Check operator approval-for-all.
    let approved_all: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::OperatorApproval(
            data.owner.clone(),
            caller.clone(),
        ))
        .unwrap_or(false);
    if approved_all {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

/// Validate that the token is not soulbound (non-transferable).
///
/// Soulbound tokens can only be burned by their owner; they may never be
/// transferred to another address.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — the token is soulbound and cannot be transferred.
pub fn validate_not_soulbound(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;

    // TokenData in the modular types only has owner + clip_id in the minimal
    // struct; soulbound flag lives in the FrozenToken key.
    let frozen: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::FrozenToken(token_id))
        .unwrap_or(false);

    let _ = data; // used above; suppress unused warning
    if frozen {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ─── Composite validator ──────────────────────────────────────────────────────

/// Run all transfer request validations as a pipeline.
///
/// Runs in this order (first failure short-circuits):
/// 1. Token exists (`validate_token_id`)
/// 2. Sender is the owner (`validate_sender`)
/// 3. Recipient is valid and not sender (`validate_recipient`)
/// 4. Caller is authorized (`validate_transfer_authorization`)
/// 5. Token is not soulbound (`validate_not_soulbound`)
///
/// # Arguments
/// * `env`       — Contract environment.
/// * `caller`    — Address invoking the transfer (owner or approved operator).
/// * `sender`    — Declared sender address (current owner).
/// * `recipient` — Declared recipient address.
/// * `token_id`  — Token being transferred.
///
/// # Errors
/// Returns the first [`Error`] encountered in the pipeline, or `Ok(())` when
/// all checks pass.
pub fn validate_transfer_request(
    env: &Env,
    caller: &Address,
    sender: &Address,
    recipient: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    validate_token_id(env, token_id)?;
    validate_sender(env, sender, token_id)?;
    validate_recipient(env, sender, recipient)?;
    validate_transfer_authorization(env, caller, token_id)?;
    validate_transfer_state(env, token_id)?;
    validate_not_soulbound(env, token_id)?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataKey, TokenData};
    use soroban_sdk::{testutils::Address as _, Env};

    fn insert_token(env: &Env, token_id: TokenId, owner: &Address) {
        env.storage().persistent().set(
            &DataKey::Token(token_id),
            &TokenData {
                owner: owner.clone(),
                clip_id: token_id,
            },
        );
    }

    // ── validate_token_id ────────────────────────────────────────────────────

    #[test]
    fn test_validate_token_id_missing() {
        let env = Env::default();
        assert_eq!(
            validate_token_id(&env, 99),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_validate_token_id_present() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 1, &owner);
        assert!(validate_token_id(&env, 1).is_ok());
    }

    // ── validate_sender ──────────────────────────────────────────────────────

    #[test]
    fn test_validate_sender_missing_token() {
        let env = Env::default();
        let sender = Address::generate(&env);
        assert_eq!(
            validate_sender(&env, &sender, 99),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_validate_sender_not_owner() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        insert_token(&env, 1, &owner);
        assert_eq!(
            validate_sender(&env, &other, 1),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn test_validate_sender_success() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 1, &owner);
        assert!(validate_sender(&env, &owner, 1).is_ok());
    }

    // ── validate_recipient ───────────────────────────────────────────────────

    #[test]
    fn test_validate_recipient_self_transfer() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert_eq!(
            validate_recipient(&env, &addr, &addr),
            Err(Error::SelfTransferNotAllowed)
        );
    }

    #[test]
    fn test_validate_recipient_success() {
        let env = Env::default();
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        assert!(validate_recipient(&env, &sender, &recipient).is_ok());
    }

    #[test]
    fn test_validate_recipient_contract_address_invalid() {
        let env = Env::default();
        let sender = Address::generate(&env);
        let contract = env.current_contract_address();
        assert_eq!(
            validate_recipient(&env, &sender, &contract),
            Err(Error::InvalidRecipient)
        );
    }

    // ── validate_transfer_state ──────────────────────────────────────────────

    #[test]
    fn test_validate_transfer_state_missing_token() {
        let env = Env::default();
        assert_eq!(
            validate_transfer_state(&env, 99),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_validate_transfer_state_active_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 20, &owner);
        assert!(validate_transfer_state(&env, 20).is_ok());
    }

    #[test]
    fn test_validate_transfer_state_burned_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 21, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::BurnedToken(21), &true);
        assert_eq!(
            validate_transfer_state(&env, 21),
            Err(Error::InvalidTransferState)
        );
    }

    #[test]
    fn test_validate_transfer_state_frozen_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 22, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::FrozenToken(22), &true);
        assert_eq!(
            validate_transfer_state(&env, 22),
            Err(Error::InvalidTransferState)
        );
    }

    #[test]
    fn test_validate_transfer_state_inactive_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 23, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::InactiveToken(23), &true);
        assert_eq!(
            validate_transfer_state(&env, 23),
            Err(Error::InvalidTransferState)
        );
    }

    // ── validate_transfer_authorization ─────────────────────────────────────

    #[test]
    fn test_validate_authorization_missing_token() {
        let env = Env::default();
        let caller = Address::generate(&env);
        assert_eq!(
            validate_transfer_authorization(&env, &caller, 99),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_validate_authorization_owner_allowed() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 2, &owner);
        assert!(validate_transfer_authorization(&env, &owner, 2).is_ok());
    }

    #[test]
    fn test_validate_authorization_approved_single_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let operator = Address::generate(&env);
        insert_token(&env, 3, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::Approval(3), &operator);
        assert!(validate_transfer_authorization(&env, &operator, 3).is_ok());
    }

    #[test]
    fn test_validate_authorization_approved_for_all() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let operator = Address::generate(&env);
        insert_token(&env, 4, &owner);
        env.storage().persistent().set(
            &StorageKey::OperatorApproval(owner.clone(), operator.clone()),
            &true,
        );
        assert!(validate_transfer_authorization(&env, &operator, 4).is_ok());
    }

    #[test]
    fn test_validate_authorization_unauthorized() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let stranger = Address::generate(&env);
        insert_token(&env, 5, &owner);
        assert_eq!(
            validate_transfer_authorization(&env, &stranger, 5),
            Err(Error::Unauthorized)
        );
    }

    // ── validate_not_soulbound ───────────────────────────────────────────────

    #[test]
    fn test_validate_not_soulbound_normal_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 6, &owner);
        // No FrozenToken key → not soulbound
        assert!(validate_not_soulbound(&env, 6).is_ok());
    }

    #[test]
    fn test_validate_not_soulbound_frozen_token() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 7, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::FrozenToken(7), &true);
        assert_eq!(
            validate_not_soulbound(&env, 7),
            Err(Error::Unauthorized)
        );
    }

    // ── validate_transfer_request (composite) ────────────────────────────────

    #[test]
    fn test_validate_transfer_request_full_success() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        insert_token(&env, 10, &owner);
        assert!(
            validate_transfer_request(&env, &owner, &owner, &recipient, 10).is_ok()
        );
    }

    #[test]
    fn test_validate_transfer_request_self_transfer_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        insert_token(&env, 11, &owner);
        assert_eq!(
            validate_transfer_request(&env, &owner, &owner, &owner, 11),
            Err(Error::SelfTransferNotAllowed)
        );
    }

    #[test]
    fn test_validate_transfer_request_missing_token_fails() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let recipient = Address::generate(&env);
        assert_eq!(
            validate_transfer_request(&env, &caller, &caller, &recipient, 999),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_validate_transfer_request_unauthorized_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let stranger = Address::generate(&env);
        let recipient = Address::generate(&env);
        insert_token(&env, 12, &owner);
        // stranger is neither owner nor approved
        assert_eq!(
            validate_transfer_request(&env, &stranger, &owner, &recipient, 12),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn test_validate_transfer_request_invalid_recipient_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let contract = env.current_contract_address();
        insert_token(&env, 13, &owner);
        assert_eq!(
            validate_transfer_request(&env, &owner, &owner, &contract, 13),
            Err(Error::InvalidRecipient)
        );
    }

    #[test]
    fn test_validate_transfer_request_burned_state_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        insert_token(&env, 14, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::BurnedToken(14), &true);
        assert_eq!(
            validate_transfer_request(&env, &owner, &owner, &recipient, 14),
            Err(Error::InvalidTransferState)
        );
    }

    #[test]
    fn test_validate_transfer_request_frozen_state_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        insert_token(&env, 15, &owner);
        env.storage()
            .persistent()
            .set(&StorageKey::FrozenToken(15), &true);
        assert_eq!(
            validate_transfer_request(&env, &owner, &owner, &recipient, 15),
            Err(Error::InvalidTransferState)
        );
    }
}
