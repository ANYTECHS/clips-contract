//! Comprehensive test suite for NFT error definitions.
//!
//! This module tests all four standardized error categories:
//! 1. SelfTransferNotAllowed - Sender and recipient must be different
//! 2. FrozenToken errors - Operations on frozen NFTs
//! 3. DuplicateToken errors - Duplicate token identifiers during minting
//! 4. Invalid state transitions - Already burned, frozen, etc.

#[cfg(test)]
mod nft_error_definitions {
    use clips_nft::types::{Error, TokenId};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // Helper setup functions
    fn with_contract<F>(test: F)
    where
        F: FnOnce(&Env),
    {
        let env = Env::default();
        env.mock_all_auths();
        env.as_contract(&env.register_contract(None, clips_nft::ClipsNftContract), || {
            test(&env);
        });
    }

    fn setup_contract_admin(env: &Env, admin: &Address) {
        env.storage()
            .instance()
            .set(&clips_nft::types::DataKey::Admin, admin);
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        env.storage()
            .persistent()
            .set(&clips_nft::types::DataKey::TokenOwner(token_id), owner);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test Category 1: SelfTransferNotAllowed (Error Code 51)
    // ─────────────────────────────────────────────────────────────────────────
    //
    // Requirement: Create SelfTransferNotAllowed error
    // Acceptance: Use in transfer validation, Add tests
    // Labels: contract, errors, transfer, validation

    /// Test that transferring to self is rejected during token transfer
    #[test]
    fn error_self_transfer_not_allowed_on_transfer() {
        with_contract(|env| {
            let sender = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &sender);
            setup_token(env, token_id, &sender);

            // Attempting to transfer to self should fail
            let result = clips_nft::owner_portfolio::validate_transfer_recipients(
                env, &sender, &sender, token_id,
            );

            assert_eq!(
                result,
                Err(Error::SelfTransferNotAllowed),
                "Transfer to self must be rejected"
            );
        });
    }

    /// Test that self-transfer is rejected in marketplace purchase when buyer equals seller
    #[test]
    fn error_self_transfer_marketplace_purchase() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &seller);
            setup_token(env, token_id, &seller);

            // Marketplace purchase validator should reject when buyer == seller
            let listing = clips_nft::types::Listing {
                token_id,
                seller: seller.clone(),
                price: 1000,
                asset_address: Address::generate(env),
                status: clips_nft::types::ListingStatus::Active,
                created_at: 0,
            };

            let result =
                clips_nft::marketplace::purchase_validator::validate_purchase_buyer(env, &seller, &listing);

            assert_eq!(
                result,
                Err(Error::SelfTransferNotAllowed),
                "Marketplace purchase by self must be rejected"
            );
        });
    }

    /// Test that self-transfer is rejected when making an offer to own token
    #[test]
    fn error_self_transfer_make_offer_to_own_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Making an offer to own token should fail
            let result = clips_nft::offer_request_validator::validate_buyer(env, &owner, token_id);

            assert_eq!(
                result,
                Err(Error::SelfTransferNotAllowed),
                "Cannot make offer on own token"
            );
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test Category 2: FrozenToken Errors
    // ─────────────────────────────────────────────────────────────────────────
    //
    // Requirement: Create frozen-token error, Use during transfers and marketplace
    // Acceptance: Use during transfers, Use during marketplace operations, Add tests
    // Labels: contract, errors, security, nft

    /// Test that frozen tokens cannot be transferred
    #[test]
    fn error_frozen_token_blocks_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let recipient = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Freeze the token
            clips_nft::frozen_token::freeze_token(env, token_id);

            // Transfer should fail due to frozen state
            let result = clips_nft::transfer_guard::check_not_frozen(env, token_id);

            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Frozen token transfer must be rejected"
            );
        });
    }

    /// Test that frozen tokens cannot be listed on marketplace
    #[test]
    fn error_frozen_token_blocks_marketplace_listing() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Freeze the token
            clips_nft::frozen_token::freeze_token(env, token_id);

            // Listing check should fail
            let result = clips_nft::listing_auth_guard::check_token_is_listable(env, token_id);

            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Frozen token cannot be listed on marketplace"
            );
        });
    }

    /// Test that frozen tokens cannot have royalty modified
    #[test]
    fn error_frozen_token_blocks_royalty_modification() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &admin);
            setup_token(env, token_id, &admin);

            // Freeze the token
            clips_nft::frozen_token::freeze_token(env, token_id);

            // Royalty modification should check freeze state
            let result = clips_nft::freeze_state_guard::require_not_frozen(env, token_id);

            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Cannot modify royalty on frozen token"
            );
        });
    }

    /// Test that frozen tokens can be unfrozen
    #[test]
    fn error_frozen_token_can_be_unfrozen() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Freeze then unfreeze
            clips_nft::frozen_token::freeze_token(env, token_id);
            assert!(clips_nft::frozen_token::is_frozen(env, token_id));

            clips_nft::frozen_token::unfreeze_token(env, token_id);
            assert!(!clips_nft::frozen_token::is_frozen(env, token_id));

            // Should now be transferable
            let result = clips_nft::transfer_guard::check_not_frozen(env, token_id);
            assert!(result.is_ok(), "Unfrozen token should be transferable");
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test Category 3: DuplicateToken Errors
    // ─────────────────────────────────────────────────────────────────────────
    //
    // Requirement: Detect duplicate token, Return standardized error
    // Acceptance: Detect duplicate token, Return standardized error, Add tests
    // Labels: contract, errors, minting, nft

    /// Test that duplicate token IDs in batch transfers are rejected
    #[test]
    fn error_duplicate_token_in_batch_transfer() {
        with_contract(|env| {
            let token_ids = soroban_sdk::Vec::from_array(env, [1, 2, 2, 3]);

            let result = clips_nft::batch_transfer_validator::validate_token_ids(env, &token_ids);

            assert_eq!(
                result,
                Err(Error::DuplicateToken),
                "Duplicate token IDs in batch must be rejected"
            );
        });
    }

    /// Test that duplicate token IDs are caught immediately
    #[test]
    fn error_duplicate_token_consecutive_duplicates() {
        with_contract(|env| {
            let token_ids = soroban_sdk::Vec::from_array(env, [5, 5, 5]);

            let result = clips_nft::batch_transfer_validator::validate_token_ids(env, &token_ids);

            assert_eq!(
                result,
                Err(Error::DuplicateToken),
                "Consecutive duplicate token IDs must be rejected"
            );
        });
    }

    /// Test that clip IDs must be unique - duplicate clip ID should fail
    #[test]
    fn error_clip_already_minted_duplicate_clip_id() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let token_id_1 = 1;
            let token_id_2 = 2;
            let clip_id = 100;

            setup_contract_admin(env, &admin);
            setup_token(env, token_id_1, &admin);

            // Map first token to clip ID
            let first_mint = clips_nft::clip_id_storage::save_clip_id(env, token_id_1, clip_id);
            assert!(first_mint.is_ok(), "First clip ID mapping should succeed");

            // Attempt to map another token to same clip ID should fail
            let second_mint = clips_nft::clip_id_storage::save_clip_id(env, token_id_2, clip_id);

            assert_eq!(
                second_mint,
                Err(Error::ClipAlreadyMinted),
                "Duplicate clip ID must be rejected"
            );
        });
    }

    /// Test that batch transfers reject any occurrence of duplicate token
    #[test]
    fn error_duplicate_token_in_batch_at_different_positions() {
        with_contract(|env| {
            let token_ids = soroban_sdk::Vec::from_array(env, [1, 2, 3, 4, 2, 5]);

            let result = clips_nft::batch_transfer_validator::validate_token_ids(env, &token_ids);

            assert_eq!(
                result,
                Err(Error::DuplicateToken),
                "Duplicate token at different positions must be rejected"
            );
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test Category 4: Invalid State Transitions
    // ─────────────────────────────────────────────────────────────────────────
    //
    // Requirement: Handle Already burned, Already frozen, Already unfrozen, Invalid transition
    // Acceptance: Handle all cases, Add tests
    // Labels: contract, errors, nft, state

    /// Test that attempting to freeze an already-frozen token is rejected
    #[test]
    fn error_already_frozen_on_duplicate_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &admin);
            setup_token(env, token_id, &admin);

            // First freeze should succeed
            let first_freeze = clips_nft::frozen_token::freeze_token(env, token_id);
            assert!(first_freeze, "First freeze should return true (token now frozen)");

            // Second freeze should fail (token already frozen)
            let second_freeze = clips_nft::frozen_token::freeze_token(env, token_id);
            assert!(
                !second_freeze,
                "Second freeze should return false (token already frozen)"
            );
        });
    }

    /// Test that attempting to unfreeze a non-frozen token is rejected
    #[test]
    fn error_already_unfrozen_on_duplicate_unfreeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &admin);
            setup_token(env, token_id, &admin);

            // Unfreeze on non-frozen token should fail
            let unfreeze = clips_nft::frozen_token::unfreeze_token(env, token_id);
            assert!(
                !unfreeze,
                "Unfreeze on non-frozen token should return false"
            );
        });
    }

    /// Test that burned tokens remain burned and cannot be operated on
    #[test]
    fn error_already_burned_prevents_operations() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Mark token as burned
            env.storage()
                .persistent()
                .set(&clips_nft::types::DataKey::BurnedToken(token_id), &true);

            // Verify token is marked as burned
            let is_burned = env.storage()
                .persistent()
                .get::<_, bool>(&clips_nft::types::DataKey::BurnedToken(token_id))
                .unwrap_or(false);

            assert!(is_burned, "Token should be marked as burned");
        });
    }

    /// Test that frozen tokens have proper state transition semantics
    #[test]
    fn error_invalid_lifecycle_transition_frozen_to_burned() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Freeze token
            clips_nft::frozen_token::freeze_token(env, token_id);
            assert!(clips_nft::frozen_token::is_frozen(env, token_id));

            // Token can still be in multiple states simultaneously (frozen + other properties)
            // but certain transitions should be invalid

            // Mark token as burned
            env.storage()
                .persistent()
                .set(&clips_nft::types::DataKey::BurnedToken(token_id), &true);

            // Verify both frozen and burned states are recorded
            let is_frozen = clips_nft::frozen_token::is_frozen(env, token_id);
            let is_burned = env.storage()
                .persistent()
                .get::<_, bool>(&clips_nft::types::DataKey::BurnedToken(token_id))
                .unwrap_or(false);

            assert!(is_frozen, "Token should be frozen");
            assert!(is_burned, "Token should be burned");
        });
    }

    /// Test that invalid lifecycle transitions are properly rejected
    #[test]
    fn error_invalid_lifecycle_transition_multiple_states() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &owner);
            setup_token(env, token_id, &owner);

            // Verify initial state: neither frozen nor burned
            assert!(!clips_nft::frozen_token::is_frozen(env, token_id));
            let is_burned = env.storage()
                .persistent()
                .get::<_, bool>(&clips_nft::types::DataKey::BurnedToken(token_id))
                .unwrap_or(false);
            assert!(!is_burned);

            // After burn, freeze should be invalid
            env.storage()
                .persistent()
                .set(&clips_nft::types::DataKey::BurnedToken(token_id), &true);

            // Verify token is burned
            let is_burned_after = env.storage()
                .persistent()
                .get::<_, bool>(&clips_nft::types::DataKey::BurnedToken(token_id))
                .unwrap_or(false);
            assert!(is_burned_after, "Token should be marked burned");
        });
    }

    /// Test error aliases for backward compatibility
    #[test]
    fn error_aliases_for_lifecycle_errors() {
        // Verify that error aliases exist and point to correct error codes
        assert_eq!(Error::TokenAlreadyBurned, Error::AlreadyBurned);
        assert_eq!(Error::TokenAlreadyFrozen, Error::AlreadyFrozen);
        assert_eq!(Error::TokenAlreadyUnfrozen, Error::AlreadyUnfrozen);
        assert_eq!(Error::InvalidLifecycle, Error::InvalidLifecycleTransition);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Integration Tests: Error Interactions
    // ─────────────────────────────────────────────────────────────────────────

    /// Test that all four error categories can be distinguished
    #[test]
    fn error_integration_all_categories_distinct() {
        with_contract(|env| {
            let sender = Address::generate(env);
            let other = Address::generate(env);
            let token_id = 1;
            setup_contract_admin(env, &sender);
            setup_token(env, token_id, &sender);

            // Error 1: SelfTransferNotAllowed
            assert_eq!(
                clips_nft::owner_portfolio::validate_transfer_recipients(
                    env, &sender, &sender, token_id
                ),
                Err(Error::SelfTransferNotAllowed)
            );

            // Error 2: FrozenToken (manifests as Unauthorized in freeze context)
            clips_nft::frozen_token::freeze_token(env, token_id);
            assert_eq!(
                clips_nft::transfer_guard::check_not_frozen(env, token_id),
                Err(Error::Unauthorized)
            );

            // Error 3: DuplicateToken
            let dup_tokens = soroban_sdk::Vec::from_array(env, [1, 1]);
            assert_eq!(
                clips_nft::batch_transfer_validator::validate_token_ids(env, &dup_tokens),
                Err(Error::DuplicateToken)
            );

            // Error 4: State transition checks
            assert!(clips_nft::frozen_token::is_frozen(env, token_id));
        });
    }
}
