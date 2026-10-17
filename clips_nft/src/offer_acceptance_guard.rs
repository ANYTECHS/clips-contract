//! Offer Acceptance Authorization Guard (Issue #1266).
//!
//! Verifies whether a caller is authorized to accept an NFT offer.
//! Ensures only the NFT owner or an authorized representative can accept offers.
//!
//! # Acceptance criteria
//!
//! | Criterion | Guard function |
//! |-----------|----------------|
//! | Verify offer existence and state | [`verify_offer_exists_and_active`] |
//! | Validate accepting account's authority | [`require_offer_acceptance_auth`] |
//! | Reject unauthorized acceptance | enforced by [`require_offer_acceptance_auth`] |
//! | Add integration tests | see test module below |
//!
//! # Authorization hierarchy
//!
//! A caller may accept an offer only when **all** of the following conditions hold:
//!
//! 1. **Offer exists** — an active offer exists for the token.
//! 2. **Offer is active** — not already accepted, cancelled, or expired.
//! 3. **Caller is authorized** — the token owner or an authorized representative.
//! 4. **Caller is not the buyer** — prevents self-dealing (buyer cannot accept own offer).
//!
//! # Authorization for offer acceptance
//!
//! Authorized accounts:
//! - **Token owner** — naturally, can accept any offer on their token
//! - **Operator** — approved for all tokens by the owner
//!
//! Note: Unlike listing cancellation, admins **cannot** accept offers.
//! Offer acceptance involves token transfer and requires owner involvement.
//!
//! # Usage
//!
//! ```rust,ignore
//! offer_acceptance_guard::require_offer_acceptance_auth(&env, &caller, token_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::marketplace::offer_storage;
use crate::marketplace::types::OfferStatus;
use crate::operator_approval;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary entry point ──────────────────────────────────────────────────────

/// Validate that `caller` is authorized to accept an offer for `token_id`.
///
/// Performs all offer acceptance pre-condition checks in order:
/// 1. Offer exists — returns [`Error::OfferNotFound`] or marketplace error if missing.
/// 2. Offer is active — returns [`Error::OfferExpired`] if not in Active state.
/// 3. Caller is authorized — returns [`Error::Unauthorized`] if not owner/operator.
/// 4. Caller is not buyer — returns [`Error::SelfTransferNotAllowed`] if self-dealing.
///
/// `caller.require_auth()` is called before any storage reads so that the
/// Soroban host always validates the cryptographic authorization envelope.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — Address attempting to accept the offer.
/// * `token_id` — Token whose offer is being accepted.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `OfferNotFound` or marketplace error | No active offer exists for `token_id`. |
/// | `OfferExpired` | Offer is not in `Active` state or has expired. |
/// | `TokenNotFound` | Token doesn't exist. |
/// | `Unauthorized`  | `caller` is not the owner or an operator. |
/// | `SelfTransferNotAllowed`  | `caller` is the offer buyer (self-dealing). |
pub fn require_offer_acceptance_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Demand a signed authorization envelope from the caller first.
    caller.require_auth();

    // 1. Offer must exist and be retrievable.
    let offer = offer_storage::get_offer(env, token_id)?;

    // 2. Offer must be in Active state (not expired, not already accepted/cancelled).
    verify_offer_is_active(env, &offer)?;

    // 3. Token must exist; retrieve its owner.
    let owner = token_owner_storage::get_owner(env, token_id)?;

    // 4. Caller must have authority over this token.
    check_acceptance_authority(env, caller, &owner)?;

    // 5. Caller must not be the buyer (prevent self-dealing).
    if caller == &offer.buyer {
        return Err(Error::SelfTransferNotAllowed);
    }

    Ok(())
}

// ─── Offer state validation ───────────────────────────────────────────────────

/// Verify that an offer exists and is in a state that permits acceptance.
///
/// Checks both:
/// - Offer status is Active (not Accepted, Cancelled, or other terminal state)
/// - Offer has not expired based on ledger timestamp
///
/// # Errors
/// Returns [`Error::OfferExpired`] if offer is not in Active state or is expired.
pub fn verify_offer_is_active(env: &Env, offer: &crate::marketplace::types::Offer) -> Result<(), Error> {
    if offer.status != OfferStatus::Active {
        return Err(Error::OfferExpired);
    }

    let now = env.ledger().timestamp();
    if offer.expires_at != 0 && offer.expires_at <= now {
        return Err(Error::OfferExpired);
    }

    Ok(())
}

/// Check if an offer exists for a token.
///
/// # Errors
/// Returns marketplace-specific error if offer doesn't exist.
pub fn verify_offer_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let _ = offer_storage::get_offer(env, token_id)?;
    Ok(())
}

// ─── Authority validation ────────────────────────────────────────────────────

/// Check whether `caller` is authorized to accept an offer on behalf of `owner`.
///
/// Authorization is granted if:
/// - Caller is the owner
/// - Caller is an operator approved by the owner for all tokens
///
/// Note: Unlike cancellation, admins cannot accept offers.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if none of the above conditions is met.
pub fn check_acceptance_authority(
    env: &Env,
    caller: &Address,
    owner: &Address,
) -> Result<(), Error> {
    // 1. Caller is the owner
    if caller == owner {
        return Ok(());
    }

    // 2. Caller is an operator of the owner
    if operator_approval::is_operator(env, owner, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Retrieve the offer for a token without full authorization checks.
/// Use this for read-only operations or when authorization is already verified.
///
/// # Errors
/// Returns marketplace-specific error if offer doesn't exist.
pub fn get_offer_info(env: &Env, token_id: TokenId) -> Result<crate::marketplace::types::Offer, Error> {
    offer_storage::get_offer(env, token_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::offer_storage;
    use crate::marketplace::types::{Offer, OfferStatus};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        f(&env)
    }

    fn setup_offer(
        env: &Env,
        token_id: TokenId,
        buyer: &Address,
        price: i128,
        expires_at: u64,
        status: OfferStatus,
    ) {
        let offer = Offer {
            offer_id: 0,
            token_id,
            buyer: buyer.clone(),
            price,
            payment_asset: Address::generate(env),
            expires_at,
            status: OfferStatus::Active,
            created_at: 0,
        };
        offer_storage::create_offer(env, &offer).ok();

        if status != OfferStatus::Active {
            offer_storage::update_offer_status(env, token_id, status).ok();
        }
    }

    // ─── Acceptance Criterion 1: Verify Offer Existence and State ──────────────

    #[test]
    fn criterion_1_1_active_offer_exists_and_valid() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let token_id = 1u32;
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let offer = offer_storage::get_offer(env, token_id).expect("Offer should exist");
            let result = verify_offer_is_active(env, &offer);
            assert!(result.is_ok(), "Active offer should pass state validation");
        });
    }

    #[test]
    fn criterion_1_2_accepted_offer_rejected() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let token_id = 1u32;
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Accepted);

            let offer = offer_storage::get_offer(env, token_id).expect("Offer should exist");
            let result = verify_offer_is_active(env, &offer);
            assert_eq!(
                result,
                Err(Error::OfferExpired),
                "Accepted offer should not pass state validation"
            );
        });
    }

    #[test]
    fn criterion_1_3_expired_offer_rejected() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let token_id = 1u32;
            env.ledger().set_timestamp(1000);
            setup_offer(env, token_id, &buyer, 1000, 500, OfferStatus::Active); // expires_at < now

            let offer = offer_storage::get_offer(env, token_id).expect("Offer should exist");
            let result = verify_offer_is_active(env, &offer);
            assert_eq!(
                result,
                Err(Error::OfferExpired),
                "Expired offer should not pass state validation"
            );
        });
    }

    #[test]
    fn criterion_1_4_offer_exists_check() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let token_id = 1u32;
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = verify_offer_exists(env, token_id);
            assert!(result.is_ok(), "Existing offer should pass existence check");
        });
    }

    // ─── Acceptance Criterion 2: Validate Accepting Account's Authority ───────

    #[test]
    fn criterion_2_1_owner_can_accept_offer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = require_offer_acceptance_auth(env, &owner, token_id);
            assert!(result.is_ok(), "Owner should have authority to accept offer");
        });
    }

    #[test]
    fn criterion_2_2_operator_can_accept_offer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            operator_approval::save_operator(env, &owner, &operator);
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = require_offer_acceptance_auth(env, &operator, token_id);
            assert!(result.is_ok(), "Operator should have authority to accept offer");
        });
    }

    #[test]
    fn criterion_2_3_non_owner_cannot_accept_offer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let non_owner = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = require_offer_acceptance_auth(env, &non_owner, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Non-owner should not have authority to accept offer"
            );
        });
    }

    #[test]
    fn criterion_2_4_buyer_cannot_accept_own_offer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            // Even if buyer owns the token somehow, they cannot accept their own offer
            let result = require_offer_acceptance_auth(env, &buyer, token_id);
            assert_eq!(
                result,
                Err(Error::SelfTransferNotAllowed),
                "Buyer cannot accept their own offer"
            );
        });
    }

    // ─── Acceptance Criterion 3: Reject Unauthorized Acceptance ───────────────

    #[test]
    fn criterion_3_1_unauthorized_random_address_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let unauthorized = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = require_offer_acceptance_auth(env, &unauthorized, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Unauthorized address should be rejected"
            );
        });
    }

    #[test]
    fn criterion_3_2_revoked_operator_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let buyer = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            let result = require_offer_acceptance_auth(env, &operator, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Revoked operator should be rejected"
            );
        });
    }

    #[test]
    fn criterion_3_3_nonexistent_offer_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let token_id = 999u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();

            let result = require_offer_acceptance_auth(env, &caller, token_id);
            assert!(result.is_err(), "Nonexistent offer should be rejected");
        });
    }

    // ─── Acceptance Criterion 4: Integration Tests ───────────────────────────

    #[test]
    fn criterion_4_1_comprehensive_offer_acceptance_flow() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let buyer = Address::generate(env);
            let unauthorized = Address::generate(env);
            let token_id = 1u32;

            token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
            operator_approval::save_operator(env, &owner, &operator);
            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            // Scenario 1: Owner can accept
            assert!(require_offer_acceptance_auth(env, &owner, token_id).is_ok());

            // Scenario 2: Operator can accept
            assert!(require_offer_acceptance_auth(env, &operator, token_id).is_ok());

            // Scenario 3: Unauthorized cannot accept
            assert_eq!(
                require_offer_acceptance_auth(env, &unauthorized, token_id),
                Err(Error::Unauthorized)
            );

            // Scenario 4: Buyer cannot accept
            assert_eq!(
                require_offer_acceptance_auth(env, &buyer, token_id),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn criterion_4_2_all_offer_states_validation() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);

            // Test each state
            let states = [
                (1u32, OfferStatus::Active, true),
                (2u32, OfferStatus::Accepted, false),
                (3u32, OfferStatus::Cancelled, false),
            ];

            for (token_id, status, should_be_acceptable) in states.iter() {
                token_owner_storage::assign_owner(env, token_id, &owner, token_id).ok();
                setup_offer(env, *token_id, &buyer, 1000, 0, *status);

                let result = require_offer_acceptance_auth(env, &owner, *token_id);
                if *should_be_acceptable {
                    assert!(result.is_ok(), "Status {:?} should allow acceptance", status);
                } else {
                    assert!(result.is_err(), "Status {:?} should reject acceptance", status);
                }
            }
        });
    }

    #[test]
    fn criterion_4_3_authorization_checked_before_self_dealing() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let token_id = 1u32;

            setup_offer(env, token_id, &buyer, 1000, 0, OfferStatus::Active);

            // Try to accept without being owner — should fail on authorization first
            let result = require_offer_acceptance_auth(env, &buyer, token_id);
            // When buyer is not owner, they're unauthorized, not self-dealing
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }
}
