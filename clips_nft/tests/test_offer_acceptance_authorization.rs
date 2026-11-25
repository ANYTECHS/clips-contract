//! Integration tests for offer acceptance authorization (Issue #1266).
//!
//! Ensures only the NFT owner or an authorized representative can accept offers.
//! Tests verify:
//! - Offer existence and active state
//! - Accepting account's authority (owner and operator)
//! - Rejection of unauthorized acceptance
//! - Self-dealing prevention

#[cfg(test)]
mod offer_acceptance_authorization_tests {
    use clips_nft::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[path = "test_helpers.rs"]
    mod test_helpers;
    use test_helpers::{setup, mint_clip};

    fn create_test_addresses(env: &Env) -> (Address, Address, Address, Address) {
        let owner = Address::generate(env);
        let buyer = Address::generate(env);
        let operator = Address::generate(env);
        let unauthorized = Address::generate(env);
        (owner, buyer, operator, unauthorized)
    }

    // ─── Acceptance Criterion 1: Verify Offer Existence and State ──────────────

    #[test]
    fn verify_offer_exists_active_offer() {
        let ctx = setup();
        let (owner, buyer, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Create an offer
        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        // Owner should be able to accept active offer
        let result = ctx.client.try_accept_offer(&owner, &token_id);
        assert!(
            result.is_ok(),
            "Owner should be able to accept active offer"
        );
    }

    // ─── Acceptance Criterion 2: Validate Accepting Account's Authority ───────

    #[test]
    fn validate_authority_owner_can_accept() {
        let ctx = setup();
        let (owner, buyer, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        let result = ctx.client.try_accept_offer(&owner, &token_id);
        assert!(result.is_ok(), "Owner should have authority to accept offer");
    }

    #[test]
    fn validate_authority_operator_can_accept() {
        let ctx = setup();
        let (owner, buyer, operator, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        // Owner grants operator approval for all tokens
        ctx.client.set_approval_for_all(&owner, &operator, &true);

        let result = ctx.client.try_accept_offer(&operator, &token_id);
        assert!(
            result.is_ok(),
            "Operator should have authority to accept offer"
        );
    }

    #[test]
    fn validate_authority_non_owner_cannot_accept() {
        let ctx = setup();
        let (owner, buyer, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        let result = ctx.client.try_accept_offer(&unauthorized, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Non-owner should not have authority to accept offer"
        );
    }

    // ─── Acceptance Criterion 3: Reject Unauthorized Acceptance ───────────────

    #[test]
    fn reject_unauthorized_random_address() {
        let ctx = setup();
        let (owner, buyer, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        let result = ctx.client.try_accept_offer(&unauthorized, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Unauthorized address should be rejected"
        );
    }

    #[test]
    fn reject_unauthorized_buyer_cannot_accept_own_offer() {
        let ctx = setup();
        let (owner, buyer, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        // Buyer tries to accept their own offer
        let result = ctx.client.try_accept_offer(&buyer, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::SelfTransferNotAllowed)),
            "Buyer should not be able to accept their own offer"
        );
    }

    #[test]
    fn reject_unauthorized_revoked_operator() {
        let ctx = setup();
        let (owner, buyer, operator, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        ctx.client.make_offer(&buyer, &token_id, &1000, &Address::generate(ctx.env), &0);

        // Grant operator approval, then revoke it
        ctx.client.set_approval_for_all(&owner, &operator, &true);
        ctx.client.set_approval_for_all(&owner, &operator, &false);

        let result = ctx.client.try_accept_offer(&operator, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Revoked operator should not be able to accept offer"
        );
    }

    #[test]
    fn reject_unauthorized_nonexistent_offer() {
        let ctx = setup();
        let (owner, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // No offer made for this token

        let result = ctx.client.try_accept_offer(&owner, &token_id);
        assert!(
            result.is_err(),
            "Accepting nonexistent offer should be rejected"
        );
    }

    // ─── Acceptance Criterion 4: Add Integration Tests ───────────────────────

    #[test]
    fn criterion_4_comprehensive_offer_acceptance_flow() {
        let ctx = setup();
        let owner = Address::generate(ctx.env);
        let buyer = Address::generate(ctx.env);
        let operator = Address::generate(ctx.env);
        let unauthorized = Address::generate(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        let payment_asset = Address::generate(ctx.env);
        ctx.client.make_offer(&buyer, &token_id, &1000, &payment_asset, &0);

        // Grant operator approval
        ctx.client.set_approval_for_all(&owner, &operator, &true);

        // Scenario 1: Owner can accept
        let token_id_2 = mint_clip(&ctx, &owner, 2, false);
        ctx.client.make_offer(&buyer, &token_id_2, &1000, &payment_asset, &0);
        assert!(
            ctx.client.try_accept_offer(&owner, &token_id_2).is_ok(),
            "Owner can accept"
        );

        // Scenario 2: Operator can accept
        let token_id_3 = mint_clip(&ctx, &owner, 3, false);
        ctx.client.make_offer(&buyer, &token_id_3, &1000, &payment_asset, &0);
        assert!(
            ctx.client.try_accept_offer(&operator, &token_id_3).is_ok(),
            "Operator can accept"
        );

        // Scenario 3: Unauthorized cannot accept
        let token_id_4 = mint_clip(&ctx, &owner, 4, false);
        ctx.client.make_offer(&buyer, &token_id_4, &1000, &payment_asset, &0);
        assert_eq!(
            ctx.client.try_accept_offer(&unauthorized, &token_id_4),
            Err(Ok(Error::Unauthorized)),
            "Unauthorized cannot accept"
        );

        // Scenario 4: Buyer cannot accept own offer
        let token_id_5 = mint_clip(&ctx, &owner, 5, false);
        ctx.client.make_offer(&buyer, &token_id_5, &1000, &payment_asset, &0);
        assert_eq!(
            ctx.client.try_accept_offer(&buyer, &token_id_5),
            Err(Ok(Error::SelfTransferNotAllowed)),
            "Buyer cannot accept own offer"
        );
    }

    #[test]
    fn criterion_4_multiple_offers_isolated() {
        let ctx = setup();
        let owner_a = Address::generate(ctx.env);
        let owner_b = Address::generate(ctx.env);
        let buyer = Address::generate(ctx.env);
        let token_id_a = mint_clip(&ctx, &owner_a, 1, false);
        let token_id_b = mint_clip(&ctx, &owner_b, 2, false);
        let payment_asset = Address::generate(ctx.env);

        // Make offers on both tokens
        ctx.client
            .make_offer(&buyer, &token_id_a, &1000, &payment_asset, &0);
        ctx.client
            .make_offer(&buyer, &token_id_b, &1000, &payment_asset, &0);

        // Owner A can accept their offer
        assert!(
            ctx.client.try_accept_offer(&owner_a, &token_id_a).is_ok(),
            "Owner A can accept their offer"
        );

        // Create new offer for token B
        ctx.client
            .make_offer(&buyer, &token_id_b, &1000, &payment_asset, &0);

        // Owner A cannot accept Owner B's offer
        assert_eq!(
            ctx.client.try_accept_offer(&owner_a, &token_id_b),
            Err(Ok(Error::Unauthorized)),
            "Owner A cannot accept Owner B's offer"
        );

        // Owner B can accept their offer
        assert!(
            ctx.client.try_accept_offer(&owner_b, &token_id_b).is_ok(),
            "Owner B can accept their offer"
        );
    }

    #[test]
    fn criterion_4_operator_cannot_accept_other_owner_offer() {
        let ctx = setup();
        let owner_a = Address::generate(ctx.env);
        let owner_b = Address::generate(ctx.env);
        let operator = Address::generate(ctx.env);
        let buyer = Address::generate(ctx.env);
        let token_id_a = mint_clip(&ctx, &owner_a, 1, false);
        let token_id_b = mint_clip(&ctx, &owner_b, 2, false);
        let payment_asset = Address::generate(ctx.env);

        // Operator for owner A
        ctx.client.set_approval_for_all(&owner_a, &operator, &true);

        // Make offers
        ctx.client
            .make_offer(&buyer, &token_id_a, &1000, &payment_asset, &0);
        ctx.client
            .make_offer(&buyer, &token_id_b, &1000, &payment_asset, &0);

        // Operator can accept owner A's offer
        assert!(
            ctx.client.try_accept_offer(&operator, &token_id_a).is_ok(),
            "Operator can accept owner A's offer"
        );

        // Recreate offer for token B
        ctx.client
            .make_offer(&buyer, &token_id_b, &1000, &payment_asset, &0);

        // Operator cannot accept owner B's offer
        assert_eq!(
            ctx.client.try_accept_offer(&operator, &token_id_b),
            Err(Ok(Error::Unauthorized)),
            "Operator cannot accept owner B's offer"
        );
    }

    #[test]
    fn criterion_4_frozen_token_offer_cannot_be_accepted() {
        let ctx = setup();
        let (owner, buyer, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);
        let payment_asset = Address::generate(ctx.env);

        ctx.client
            .make_offer(&buyer, &token_id, &1000, &payment_asset, &0);

        // Freeze the token
        ctx.client.freeze_token(&ctx.admin, &token_id);

        // Owner cannot accept offer on frozen token
        let result = ctx.client.try_accept_offer(&owner, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Cannot accept offer on frozen token"
        );
    }

    #[test]
    fn criterion_4_authorization_checked_before_other_validations() {
        let ctx = setup();
        let (owner, buyer, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);
        let payment_asset = Address::generate(ctx.env);

        ctx.client
            .make_offer(&buyer, &token_id, &1000, &payment_asset, &0);

        // Freeze the token
        ctx.client.freeze_token(&ctx.admin, &token_id);

        // Unauthorized should fail on authorization, not frozen token check
        let result = ctx.client.try_accept_offer(&unauthorized, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Should fail on authorization first"
        );
    }
}
