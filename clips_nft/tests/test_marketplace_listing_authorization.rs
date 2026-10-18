//! Integration tests for marketplace listing authorization (Issue #1263).
//!
//! Ensures only NFT owners or explicitly approved accounts can create listings.
//! Tests verify:
//! - Listing authority checks (owner, approved, operator)
//! - NFT existence validation
//! - Rejection of unauthorized listing requests
//! - Authorization with different approval types

#[cfg(test)]
mod marketplace_listing_authorization_tests {
    use clips_nft::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[path = "test_helpers.rs"]
    mod test_helpers;
    use test_helpers::{setup, mint_clip};

    fn create_test_addresses(env: &Env) -> (Address, Address, Address, Address) {
        let owner = Address::generate(env);
        let approved = Address::generate(env);
        let operator = Address::generate(env);
        let unauthorized = Address::generate(env);
        (owner, approved, operator, unauthorized)
    }

    // ─── Authorization Acceptance Criteria ────────────────────────────────────────

    #[test]
    fn verify_listing_authority_owner_can_create_listing() {
        let ctx = setup();
        let (owner, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: owner.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert!(result.is_ok(), "Owner should be able to create listing");
    }

    #[test]
    fn verify_listing_authority_non_owner_cannot_create_listing() {
        let ctx = setup();
        let (owner, _, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: unauthorized.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Non-owner should not be able to create listing"
        );
    }

    #[test]
    fn verify_listing_authority_approved_address_can_create_listing() {
        let ctx = setup();
        let (owner, approved, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Owner grants approval to approved address
        ctx.client.approve(&owner, &approved, &token_id);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: approved.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert!(
            result.is_ok(),
            "Approved address should be able to create listing"
        );
    }

    #[test]
    fn verify_listing_authority_revoked_approval_cannot_create_listing() {
        let ctx = setup();
        let (owner, approved, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Owner grants and then revokes approval
        ctx.client.approve(&owner, &approved, &token_id);
        ctx.client.revoke_approval(&owner, &approved, &token_id);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: approved.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Previously approved address should not be able to create listing after revocation"
        );
    }

    #[test]
    fn verify_listing_authority_operator_can_create_listing() {
        let ctx = setup();
        let (owner, _, operator, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Owner grants operator approval for all tokens
        ctx.client.set_approval_for_all(&owner, &operator, &true);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: operator.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert!(
            result.is_ok(),
            "Operator should be able to create listing for owner's token"
        );
    }

    #[test]
    fn verify_listing_authority_revoked_operator_cannot_create_listing() {
        let ctx = setup();
        let (owner, _, operator, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Owner grants and then revokes operator approval
        ctx.client.set_approval_for_all(&owner, &operator, &true);
        ctx.client.set_approval_for_all(&owner, &operator, &false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: operator.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Operator should not be able to create listing after revocation"
        );
    }

    // ─── NFT Existence Confirmation ───────────────────────────────────────────────

    #[test]
    fn confirm_nft_exists_returns_error_for_nonexistent_token() {
        let ctx = setup();
        let (_, _, _, unauthorized) = create_test_addresses(ctx.env);
        let nonexistent_token_id = 999u32;

        let listing = clips_nft::ListingRequest {
            token_id: nonexistent_token_id,
            seller: unauthorized.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::TokenNotFound)),
            "Creating listing for nonexistent token should fail"
        );
    }

    // ─── Unauthorized Request Rejection ───────────────────────────────────────────

    #[test]
    fn reject_unauthorized_listing_requests_from_random_address() {
        let ctx = setup();
        let (owner, _, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: unauthorized.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Random unauthorized address should not be able to create listing"
        );
    }

    #[test]
    fn reject_unauthorized_listing_requests_approved_for_different_token() {
        let ctx = setup();
        let (owner, approved, _, _) = create_test_addresses(ctx.env);
        let token_id_1 = mint_clip(&ctx, &owner, 1, false);
        let token_id_2 = mint_clip(&ctx, &owner, 2, false);

        // Approve for token 1 only
        ctx.client.approve(&owner, &approved, &token_id_1);

        // Try to create listing for token 2
        let listing = clips_nft::ListingRequest {
            token_id: token_id_2,
            seller: approved.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Approved for different token should not be able to create listing"
        );
    }

    #[test]
    fn reject_unauthorized_listing_requests_frozen_token() {
        let ctx = setup();
        let (owner, _, _, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Freeze the token
        ctx.client.freeze_token(&ctx.admin, &token_id);

        let listing = clips_nft::ListingRequest {
            token_id,
            seller: owner.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Frozen token should not be listable"
        );
    }

    // ─── Edge Cases ───────────────────────────────────────────────────────────────

    #[test]
    fn multiple_approvals_owner_can_still_create_listing() {
        let ctx = setup();
        let (owner, approved_1, approved_2, _) = create_test_addresses(ctx.env);
        let token_id = mint_clip(&ctx, &owner, 1, false);

        // Owner grants multiple approvals
        ctx.client.approve(&owner, &approved_1, &token_id);
        ctx.client.approve(&owner, &approved_2, &token_id);

        // Owner should still be able to create listing
        let listing = clips_nft::ListingRequest {
            token_id,
            seller: owner.clone(),
            price: 1000,
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        assert!(
            result.is_ok(),
            "Owner should be able to create listing even with other approvals"
        );
    }

    #[test]
    fn authorization_check_happens_before_validation() {
        let ctx = setup();
        let (_, _, _, unauthorized) = create_test_addresses(ctx.env);
        let token_id = 999u32; // Nonexistent token

        // Create listing with invalid price and nonexistent token
        let listing = clips_nft::ListingRequest {
            token_id,
            seller: unauthorized.clone(),
            price: 0, // Invalid price
            payment_asset: Address::generate(ctx.env),
            expiration: 0,
            listing_id: 0,
        };

        let result = ctx.client.try_create_listing(&listing);
        // Should fail on authorization first (token not found), not validation
        assert_eq!(
            result,
            Err(Ok(Error::TokenNotFound)),
            "Authorization check should fail before price validation"
        );
    }
}
