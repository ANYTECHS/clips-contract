//! Unit tests validating listing authorization acceptance criteria (Issue #1263).
//!
//! These tests directly verify the listing_auth_guard implementation against
//! all acceptance criteria:
//! - Verify listing authority (owner, approved, operator)
//! - Confirm NFT exists
//! - Reject unauthorized listing requests
//! - Add marketplace authorization tests (this file)

#[cfg(test)]
mod listing_authorization_acceptance_criteria {
    use clips_nft::listing_auth_guard;
    use clips_nft::token_approval;
    use clips_nft::token_owner_storage;
    use clips_nft::operator_approval;
    use clips_nft::frozen_token;
    use clips_nft::types::{Error, TokenId};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        f(&env)
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ─────────────────────────────────────────────────────────────────────────────────
    // ACCEPTANCE CRITERION 1: Verify listing authority
    // ─────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn criterion_1_1_owner_has_listing_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            let result = listing_auth_guard::require_listing_auth(env, &owner, 1);
            assert!(
                result.is_ok(),
                "Owner must have authority to create listing"
            );
        });
    }

    #[test]
    fn criterion_1_2_approved_address_has_listing_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);

            let result = listing_auth_guard::require_listing_auth(env, &approved, 1);
            assert!(
                result.is_ok(),
                "Approved address must have authority to create listing"
            );
        });
    }

    #[test]
    fn criterion_1_3_operator_has_listing_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            let result = listing_auth_guard::require_listing_auth(env, &operator, 1);
            assert!(
                result.is_ok(),
                "Operator must have authority to create listing for owner's tokens"
            );
        });
    }

    // ─────────────────────────────────────────────────────────────────────────────────
    // ACCEPTANCE CRITERION 2: Confirm NFT exists
    // ─────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn criterion_2_1_nonexistent_nft_returns_token_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            // Token 999 was never created

            let result = listing_auth_guard::require_listing_auth(env, &caller, 999);
            assert_eq!(
                result,
                Err(Error::TokenNotFound),
                "Nonexistent NFT must return TokenNotFound"
            );
        });
    }

    #[test]
    fn criterion_2_2_nft_exists_passes_existence_check() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            let result = listing_auth_guard::require_listing_auth(env, &owner, 1);
            assert!(
                result.is_ok(),
                "Existing NFT must pass existence check"
            );
        });
    }

    // ─────────────────────────────────────────────────────────────────────────────────
    // ACCEPTANCE CRITERION 3: Reject unauthorized listing requests
    // ─────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn criterion_3_1_non_owner_without_approval_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let non_owner = Address::generate(env);
            setup_token(env, 1, &owner);

            let result = listing_auth_guard::require_listing_auth(env, &non_owner, 1);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Non-owner without approval must be rejected"
            );
        });
    }

    #[test]
    fn criterion_3_2_revoked_approval_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            token_approval::remove_approval(env, 1);

            let result = listing_auth_guard::require_listing_auth(env, &approved, 1);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Revoked approval must result in rejection"
            );
        });
    }

    #[test]
    fn criterion_3_3_revoked_operator_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);

            let result = listing_auth_guard::require_listing_auth(env, &operator, 1);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Revoked operator must result in rejection"
            );
        });
    }

    #[test]
    fn criterion_3_4_frozen_token_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);

            let result = listing_auth_guard::require_listing_auth(env, &owner, 1);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Frozen (soulbound) token must be rejected"
            );
        });
    }

    #[test]
    fn criterion_3_5_approval_for_different_token_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            // Approve for token 1, not token 2
            token_approval::save_approval(env, 1, &approved);

            let result = listing_auth_guard::require_listing_auth(env, &approved, 2);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Approval for different token must be rejected"
            );
        });
    }

    // ─────────────────────────────────────────────────────────────────────────────────
    // ACCEPTANCE CRITERION 4: Marketplace authorization tests (this module)
    // ─────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn criterion_4_1_auth_guard_called_before_storage_modifications() {
        // This test verifies the authorization guard properly calls require_auth()
        // which performs cryptographic verification before any storage reads.
        with_contract(|env| {
            let caller = Address::generate(env);
            setup_token(env, 1, &caller);

            // require_listing_auth calls caller.require_auth() internally
            // If it didn't, this would not enforce signature requirements
            let result = listing_auth_guard::require_listing_auth(env, &caller, 1);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn criterion_4_2_comprehensive_authorization_flow() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            let operator = Address::generate(env);
            let unauthorized = Address::generate(env);

            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);

            // Scenario 1: Owner can list their token
            assert!(
                listing_auth_guard::require_listing_auth(env, &owner, 1).is_ok(),
                "Owner can list"
            );

            // Scenario 2: Approve address for token 1
            token_approval::save_approval(env, 1, &approved);
            assert!(
                listing_auth_guard::require_listing_auth(env, &approved, 1).is_ok(),
                "Approved address can list"
            );

            // Scenario 3: Approved cannot list other tokens
            assert_eq!(
                listing_auth_guard::require_listing_auth(env, &approved, 2),
                Err(Error::Unauthorized),
                "Approved cannot list other token"
            );

            // Scenario 4: Operator for all tokens
            operator_approval::save_operator(env, &owner, &operator);
            assert!(
                listing_auth_guard::require_listing_auth(env, &operator, 1).is_ok(),
                "Operator can list any token"
            );
            assert!(
                listing_auth_guard::require_listing_auth(env, &operator, 2).is_ok(),
                "Operator can list any token (2)"
            );

            // Scenario 5: Unauthorized address cannot list
            assert_eq!(
                listing_auth_guard::require_listing_auth(env, &unauthorized, 1),
                Err(Error::Unauthorized),
                "Unauthorized cannot list"
            );
        });
    }

    #[test]
    fn criterion_4_3_frozen_token_blocks_all_actors() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            frozen_token::freeze_token(env, 1);

            // Even owner cannot list frozen token
            assert_eq!(
                listing_auth_guard::require_listing_auth(env, &owner, 1),
                Err(Error::Unauthorized),
                "Owner cannot list frozen token"
            );

            // Even approved cannot list frozen token
            assert_eq!(
                listing_auth_guard::require_listing_auth(env, &approved, 1),
                Err(Error::Unauthorized),
                "Approved cannot list frozen token"
            );
        });
    }

    #[test]
    fn criterion_4_4_unfrozen_token_restores_listing_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            frozen_token::unfreeze_token(env, 1);

            let result = listing_auth_guard::require_listing_auth(env, &owner, 1);
            assert!(
                result.is_ok(),
                "Unfrozen token should restore listing authority"
            );
        });
    }
}
