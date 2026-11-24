//! Integration tests for marketplace listing cancellation authorization (Issue #1264).
//!
//! Ensures only the seller or explicitly authorized accounts can cancel listings.
//! Tests verify:
//! - Listing owner/seller retrieval
//! - Cancellation permission verification
//! - Rejection of unauthorized cancellation requests
//! - Authorization with seller, operator, and admin roles

#[cfg(test)]
mod listing_cancellation_authorization_tests {
    use clips_nft::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[path = "test_helpers.rs"]
    mod test_helpers;
    use test_helpers::{setup, mint_clip};

    fn create_test_addresses(env: &Env) -> (Address, Address, Address, Address) {
        let seller = Address::generate(env);
        let operator = Address::generate(env);
        let admin = Address::generate(env);
        let unauthorized = Address::generate(env);
        (seller, operator, admin, unauthorized)
    }

    // ─── Acceptance Criterion 1: Retrieve Listing Owner/Seller ──────────────────

    #[test]
    fn retrieve_listing_seller_owner_can_cancel_their_listing() {
        let ctx = setup();
        let (seller, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Seller should be able to cancel their own listing
        let result = ctx.client.try_cancel_listing(&seller, &token_id);
        assert!(
            result.is_ok(),
            "Seller should be able to cancel their own listing"
        );
    }

    // ─── Acceptance Criterion 2: Verify Cancellation Permissions ───────────────

    #[test]
    fn verify_permissions_seller_can_cancel() {
        let ctx = setup();
        let (seller, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        let result = ctx.client.try_cancel_listing(&seller, &token_id);
        assert!(result.is_ok(), "Seller should have permission to cancel");
    }

    #[test]
    fn verify_permissions_operator_can_cancel() {
        let ctx = setup();
        let (seller, operator, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Seller grants operator approval for all tokens
        ctx.client.set_approval_for_all(&seller, &operator, &true);

        let result = ctx.client.try_cancel_listing(&operator, &token_id);
        assert!(
            result.is_ok(),
            "Operator should have permission to cancel seller's listing"
        );
    }

    #[test]
    fn verify_permissions_admin_can_cancel() {
        let ctx = setup();
        let (seller, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Admin should be able to cancel any listing
        let result = ctx.client.try_cancel_listing(&ctx.admin, &token_id);
        assert!(result.is_ok(), "Admin should have permission to cancel any listing");
    }

    // ─── Acceptance Criterion 3: Reject Unauthorized Cancellation ──────────────

    #[test]
    fn reject_unauthorized_random_stranger() {
        let ctx = setup();
        let (seller, _, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        let result = ctx.client.try_cancel_listing(&unauthorized, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Unauthorized stranger should not be able to cancel listing"
        );
    }

    #[test]
    fn reject_unauthorized_revoked_operator() {
        let ctx = setup();
        let (seller, operator, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Grant operator approval, then revoke it
        ctx.client.set_approval_for_all(&seller, &operator, &true);
        ctx.client.set_approval_for_all(&seller, &operator, &false);

        let result = ctx.client.try_cancel_listing(&operator, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Revoked operator should not be able to cancel listing"
        );
    }

    #[test]
    fn reject_unauthorized_operator_for_different_seller() {
        let ctx = setup();
        let seller = Address::generate(ctx.env);
        let operator = Address::generate(ctx.env);
        let other_seller = Address::generate(ctx.env);
        let token_id_1 = mint_clip(&ctx, &seller, 1, false);
        let token_id_2 = mint_clip(&ctx, &other_seller, 2, false);

        let listing1 = clips_nft::ListingRequest {
            token_id: token_id_1,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let listing2 = clips_nft::ListingRequest {
            token_id: token_id_2,
            seller: other_seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing1);
        ctx.client.create_listing(&listing2);

        // Operator for seller, not other_seller
        ctx.client.set_approval_for_all(&seller, &operator, &true);

        // Operator cannot cancel other_seller's listing
        let result = ctx.client.try_cancel_listing(&operator, &token_id_2);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Operator of different seller should not be able to cancel their listing"
        );
    }

    #[test]
    fn reject_unauthorized_nonexistent_listing() {
        let ctx = setup();
        let (_, _, _, unauthorized) = create_test_addresses(ctx.env);
        let nonexistent_token_id = 999u32;

        let result = ctx.client.try_cancel_listing(&unauthorized, &nonexistent_token_id);
        assert_eq!(
            result,
            Err(Ok(Error::ListingNotFound)),
            "Cancelling nonexistent listing should fail"
        );
    }

    // ─── Authorization Edge Cases ────────────────────────────────────────────────

    #[test]
    fn edge_case_seller_cannot_cancel_another_seller_listing() {
        let ctx = setup();
        let seller_a = Address::generate(ctx.env);
        let seller_b = Address::generate(ctx.env);
        let token_id = mint_clip(&ctx, &seller_a, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller_a.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Seller B should not be able to cancel seller A's listing
        let result = ctx.client.try_cancel_listing(&seller_b, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Different seller should not be able to cancel listing"
        );
    }

    #[test]
    fn edge_case_multiple_operators_permission_check() {
        let ctx = setup();
        let seller = Address::generate(ctx.env);
        let operator_1 = Address::generate(ctx.env);
        let operator_2 = Address::generate(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Grant approval to both operators
        ctx.client.set_approval_for_all(&seller, &operator_1, &true);
        ctx.client.set_approval_for_all(&seller, &operator_2, &true);

        // Both should be able to cancel
        let result1 = ctx.client.try_cancel_listing(&operator_1, &token_id);
        assert!(result1.is_ok(), "First operator should be able to cancel");

        // Create another listing for second operator test
        let token_id_2 = mint_clip(&ctx, &seller, 2, false);
        let listing2 = clips_nft::ListingRequest {
            token_id: token_id_2,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };
        ctx.client.create_listing(&listing2);

        let result2 = ctx.client.try_cancel_listing(&operator_2, &token_id_2);
        assert!(result2.is_ok(), "Second operator should be able to cancel");
    }

    #[test]
    fn edge_case_frozen_token_cannot_be_cancelled() {
        let ctx = setup();
        let (seller, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &seller, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        ctx.client.create_listing(&listing);

        // Freeze the token
        ctx.client.freeze_token(&ctx.admin, &token_id);

        // Even seller cannot cancel frozen token's listing
        let result = ctx.client.try_cancel_listing(&seller, &token_id);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Frozen token listing should not be cancellable"
        );
    }

    #[test]
    fn edge_case_authorization_checked_before_validation() {
        let ctx = setup();
        let (_, _, _, unauthorized) = create_test_addresses(ctx.env);
        let nonexistent_token_id = 999u32;

        // Attempt to cancel nonexistent token with unauthorized address
        // Should fail on authorization (not found) not on permissions
        let result = ctx.client.try_cancel_listing(&unauthorized, &nonexistent_token_id);
        assert_eq!(
            result,
            Err(Ok(Error::ListingNotFound)),
            "Should fail on listing not found, not authorization"
        );
    }

    // ─── Acceptance Criterion 4: Unit Tests ──────────────────────────────────────

    #[test]
    fn criterion_4_comprehensive_authorization_hierarchy() {
        let ctx = setup();
        let seller = Address::generate(ctx.env);
        let operator = Address::generate(ctx.env);
        let unauthorized = Address::generate(ctx.env);
        let token_id_1 = mint_clip(&ctx, &seller, 1, false);
        let token_id_2 = mint_clip(&ctx, &seller, 2, false);
        let token_id_3 = mint_clip(&ctx, &seller, 3, false);

        // Setup listings
        for (i, token_id) in [token_id_1, token_id_2, token_id_3].iter().enumerate() {
            let listing = clips_nft::ListingRequest {
                token_id: *token_id,
                seller: seller.clone(),
                price: 1000,
                payment_asset: Address::generate(ctx.env),
                expiration: 0,
                listing_id: 0,
            };
            ctx.client.create_listing(&listing);
        }

        // Grant operator approval
        ctx.client.set_approval_for_all(&seller, &operator, &true);

        // Scenario 1: Seller can cancel
        let result = ctx.client.try_cancel_listing(&seller, &token_id_1);
        assert!(result.is_ok(), "Seller can cancel");

        // Scenario 2: Operator can cancel
        let result = ctx.client.try_cancel_listing(&operator, &token_id_2);
        assert!(result.is_ok(), "Operator can cancel");

        // Scenario 3: Admin can cancel
        let result = ctx.client.try_cancel_listing(&ctx.admin, &token_id_3);
        assert!(result.is_ok(), "Admin can cancel");

        // Scenario 4: Unauthorized cannot cancel
        let token_id_4 = mint_clip(&ctx, &seller, 4, false);
        let listing4 = clips_nft::ListingRequest {
            token_id: token_id_4,
            seller: seller.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };
        ctx.client.create_listing(&listing4);

        let result = ctx.client.try_cancel_listing(&unauthorized, &token_id_4);
        assert_eq!(result, Err(Ok(Error::Unauthorized)), "Unauthorized cannot cancel");
    }
}
