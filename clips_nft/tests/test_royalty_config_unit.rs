//! Unit tests for royalty configuration and storage (issue #787).
//!
//! Covers:
//! - Default royalty (getter fallback, set/get round-trip, boundary, auth)
//! - Maximum royalty constant validation
//! - NFT royalty storage (save / get / update)
//! - Creator royalty storage (royalty recipient per-token)
//! - Recipient indexing (add / remove / query / duplicate prevention)
//! - Invalid configurations (bps out of range, bad recipient)
//!
//! Target: 90 %+ coverage of all royalty-related modules.

#![cfg(test)]

use clips_nft::{
    royalty_recipient_index::{
        add_token_to_recipient, get_recipient_tokens, recipient_contains_token,
        recipient_token_count, remove_token_from_recipient,
    },
    royalty_recipient_struct::{new_royalty_recipient, validate_royalty_recipient_struct},
    AtomicMintContract, DataKey, Error, Royalty, RoyaltyRecipient, DEFAULT_ROYALTY_BPS,
    MAX_ROYALTY_BPS,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn make_royalty(env: &Env, bps: u32) -> Royalty {
    Royalty {
        recipient: Address::generate(env),
        basis_points: bps,
        asset_address: None,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. DEFAULT ROYALTY
// ═════════════════════════════════════════════════════════════════════════════

mod default_royalty_tests {
    use super::*;
    use clips_nft::default_royalty::{
        get_default_royalty_bps, has_default_royalty_bps, set_default_royalty_bps,
    };

    #[test]
    fn returns_compile_time_default_before_any_set() {
        with_contract(|env| {
            assert_eq!(get_default_royalty_bps(env), DEFAULT_ROYALTY_BPS);
            assert_eq!(DEFAULT_ROYALTY_BPS, 500);
        });
    }

    #[test]
    fn has_default_royalty_bps_false_before_set() {
        with_contract(|env| {
            assert!(!has_default_royalty_bps(env));
        });
    }

    #[test]
    fn has_default_royalty_bps_true_after_set() {
        with_contract(|env| {
            set_default_royalty_bps(env, 300).unwrap();
            assert!(has_default_royalty_bps(env));
        });
    }

    #[test]
    fn set_and_get_round_trip() {
        with_contract(|env| {
            set_default_royalty_bps(env, 750).unwrap();
            assert_eq!(get_default_royalty_bps(env), 750);
        });
    }

    #[test]
    fn overwrite_stores_latest_value() {
        with_contract(|env| {
            set_default_royalty_bps(env, 100).unwrap();
            set_default_royalty_bps(env, 9_000).unwrap();
            assert_eq!(get_default_royalty_bps(env), 9_000);
        });
    }

    #[test]
    fn zero_bps_accepted() {
        with_contract(|env| {
            set_default_royalty_bps(env, 0).unwrap();
            assert_eq!(get_default_royalty_bps(env), 0);
        });
    }

    #[test]
    fn max_bps_accepted() {
        with_contract(|env| {
            set_default_royalty_bps(env, MAX_ROYALTY_BPS).unwrap();
            assert_eq!(get_default_royalty_bps(env), MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn above_max_returns_invalid_basis_points() {
        with_contract(|env| {
            assert_eq!(
                set_default_royalty_bps(env, MAX_ROYALTY_BPS + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn large_value_returns_invalid_basis_points() {
        with_contract(|env| {
            assert_eq!(
                set_default_royalty_bps(env, 50_000),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn u32_max_returns_invalid_basis_points() {
        with_contract(|env| {
            assert_eq!(
                set_default_royalty_bps(env, u32::MAX),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    // Storage is written to instance scope — verify it persists across
    // consecutive reads without re-setting.
    #[test]
    fn value_persists_across_multiple_reads() {
        with_contract(|env| {
            set_default_royalty_bps(env, 600).unwrap();
            assert_eq!(get_default_royalty_bps(env), 600);
            assert_eq!(get_default_royalty_bps(env), 600);
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. MAXIMUM ROYALTY CONSTANT
// ═════════════════════════════════════════════════════════════════════════════

mod max_royalty_tests {
    use super::*;

    #[test]
    fn max_royalty_bps_is_ten_thousand() {
        assert_eq!(MAX_ROYALTY_BPS, 10_000);
    }

    #[test]
    fn max_royalty_equals_one_hundred_percent_in_bps() {
        // 10 000 bps = 100 %
        assert_eq!(MAX_ROYALTY_BPS, 100 * 100);
    }

    #[test]
    fn default_royalty_bps_within_max() {
        assert!(DEFAULT_ROYALTY_BPS <= MAX_ROYALTY_BPS);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. NFT ROYALTY STORAGE  (royalty_storage module)
// ═════════════════════════════════════════════════════════════════════════════

mod nft_royalty_storage_tests {
    use super::*;
    use clips_nft::royalty_storage::{get_royalty, save_royalty, update_royalty};

    #[test]
    fn save_and_get_royalty_round_trip() {
        with_contract(|env| {
            let royalty = make_royalty(env, 500);
            save_royalty(env, 1, &royalty);
            let loaded = get_royalty(env, 1).unwrap();
            assert_eq!(loaded.basis_points, 500);
            assert_eq!(loaded.recipient, royalty.recipient);
        });
    }

    #[test]
    fn get_royalty_missing_token_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_royalty(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn save_royalty_overwrites_on_second_call() {
        with_contract(|env| {
            let royalty_a = make_royalty(env, 300);
            let royalty_b = make_royalty(env, 700);
            save_royalty(env, 1, &royalty_a);
            save_royalty(env, 1, &royalty_b);
            let loaded = get_royalty(env, 1).unwrap();
            assert_eq!(loaded.basis_points, 700);
            assert_eq!(loaded.recipient, royalty_b.recipient);
        });
    }

    #[test]
    fn royalties_are_isolated_per_token() {
        with_contract(|env| {
            let r1 = make_royalty(env, 200);
            let r2 = make_royalty(env, 800);
            save_royalty(env, 1, &r1);
            save_royalty(env, 2, &r2);
            assert_eq!(get_royalty(env, 1).unwrap().basis_points, 200);
            assert_eq!(get_royalty(env, 2).unwrap().basis_points, 800);
        });
    }

    #[test]
    fn update_royalty_succeeds_when_token_exists() {
        with_contract(|env| {
            let original = make_royalty(env, 250);
            save_royalty(env, 10, &original);

            let updated = make_royalty(env, 999);
            update_royalty(env, 10, &updated).unwrap();

            let loaded = get_royalty(env, 10).unwrap();
            assert_eq!(loaded.basis_points, 999);
        });
    }

    #[test]
    fn update_royalty_returns_token_not_found_when_absent() {
        with_contract(|env| {
            let royalty = make_royalty(env, 500);
            assert_eq!(update_royalty(env, 42, &royalty), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn save_royalty_zero_bps_is_valid() {
        with_contract(|env| {
            let royalty = make_royalty(env, 0);
            save_royalty(env, 3, &royalty);
            assert_eq!(get_royalty(env, 3).unwrap().basis_points, 0);
        });
    }

    #[test]
    fn save_royalty_max_bps_is_valid() {
        with_contract(|env| {
            let royalty = make_royalty(env, MAX_ROYALTY_BPS);
            save_royalty(env, 4, &royalty);
            assert_eq!(get_royalty(env, 4).unwrap().basis_points, MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn royalty_with_asset_address_round_trips() {
        with_contract(|env| {
            let asset = Address::generate(env);
            let royalty = Royalty {
                recipient: Address::generate(env),
                basis_points: 500,
                asset_address: Some(asset.clone()),
            };
            save_royalty(env, 5, &royalty);
            let loaded = get_royalty(env, 5).unwrap();
            assert_eq!(loaded.asset_address, Some(asset));
        });
    }

    #[test]
    fn royalty_without_asset_address_round_trips() {
        with_contract(|env| {
            let royalty = Royalty {
                recipient: Address::generate(env),
                basis_points: 500,
                asset_address: None,
            };
            save_royalty(env, 6, &royalty);
            let loaded = get_royalty(env, 6).unwrap();
            assert_eq!(loaded.asset_address, None);
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. CREATOR / RECIPIENT ROYALTY STORAGE  (royalty_recipient module)
// ═════════════════════════════════════════════════════════════════════════════

mod creator_royalty_storage_tests {
    use super::*;
    use clips_nft::royalty_recipient::{
        get_royalty_recipient, set_royalty_recipient, update_royalty_recipient,
    };
    use clips_nft::types::TokenData;

    fn seed_token(env: &Env, token_id: u32, owner: &Address) {
        env.storage().persistent().set(
            &DataKey::Token(token_id),
            &TokenData {
                owner: owner.clone(),
                clip_id: token_id,
            },
        );
    }

    #[test]
    fn set_and_get_recipient_round_trip() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            set_royalty_recipient(env, 1, &recipient);
            assert_eq!(get_royalty_recipient(env, 1), Ok(recipient));
        });
    }

    #[test]
    fn get_recipient_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_royalty_recipient(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn recipients_are_isolated_per_token() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            set_royalty_recipient(env, 1, &a);
            set_royalty_recipient(env, 2, &b);
            assert_eq!(get_royalty_recipient(env, 1), Ok(a));
            assert_eq!(get_royalty_recipient(env, 2), Ok(b));
        });
    }

    #[test]
    fn set_royalty_recipient_can_be_overwritten() {
        with_contract(|env| {
            let old = Address::generate(env);
            let new = Address::generate(env);
            set_royalty_recipient(env, 3, &old);
            set_royalty_recipient(env, 3, &new);
            assert_eq!(get_royalty_recipient(env, 3), Ok(new));
        });
    }

    #[test]
    fn update_royalty_recipient_succeeds_when_token_exists() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let new_recipient = Address::generate(env);
            seed_token(env, 10, &owner);
            set_royalty_recipient(env, 10, &owner);

            update_royalty_recipient(env, 10, &new_recipient).unwrap();
            assert_eq!(get_royalty_recipient(env, 10), Ok(new_recipient));
        });
    }

    #[test]
    fn update_royalty_recipient_fails_when_token_absent() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert_eq!(
                update_royalty_recipient(env, 42, &recipient),
                Err(Error::TokenNotFound)
            );
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. RECIPIENT INDEXING  (royalty_recipient_index module)
// ═════════════════════════════════════════════════════════════════════════════

mod recipient_index_tests {
    use super::*;

    // ── Add token ─────────────────────────────────────────────────────────────

    #[test]
    fn add_token_appends_to_index() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 1).unwrap();
            assert_eq!(get_recipient_tokens(env, &r).len(), 1);
        });
    }

    #[test]
    fn add_multiple_tokens_preserves_insertion_order() {
        with_contract(|env| {
            let r = Address::generate(env);
            for id in [10u32, 20, 30] {
                add_token_to_recipient(env, &r, id).unwrap();
            }
            let tokens = get_recipient_tokens(env, &r);
            assert_eq!(tokens.get(0).unwrap(), 10);
            assert_eq!(tokens.get(1).unwrap(), 20);
            assert_eq!(tokens.get(2).unwrap(), 30);
        });
    }

    // ── Remove token ──────────────────────────────────────────────────────────

    #[test]
    fn remove_token_deletes_entry() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 5).unwrap();
            remove_token_from_recipient(env, &r, 5);
            assert_eq!(recipient_token_count(env, &r), 0);
            assert!(!recipient_contains_token(env, &r, 5));
        });
    }

    #[test]
    fn remove_middle_entry_preserves_remaining() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 1).unwrap();
            add_token_to_recipient(env, &r, 2).unwrap();
            add_token_to_recipient(env, &r, 3).unwrap();
            remove_token_from_recipient(env, &r, 2);

            let tokens = get_recipient_tokens(env, &r);
            assert_eq!(tokens.len(), 2);
            assert_eq!(tokens.get(0).unwrap(), 1);
            assert_eq!(tokens.get(1).unwrap(), 3);
        });
    }

    #[test]
    fn remove_nonexistent_token_is_noop() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 1).unwrap();
            remove_token_from_recipient(env, &r, 99);
            assert_eq!(recipient_token_count(env, &r), 1);
        });
    }

    #[test]
    fn remove_from_empty_index_is_noop() {
        with_contract(|env| {
            let r = Address::generate(env);
            remove_token_from_recipient(env, &r, 7); // must not panic
            assert_eq!(recipient_token_count(env, &r), 0);
        });
    }

    // ── Query recipient NFTs ──────────────────────────────────────────────────

    #[test]
    fn get_recipient_tokens_empty_for_unknown_address() {
        with_contract(|env| {
            let r = Address::generate(env);
            assert_eq!(get_recipient_tokens(env, &r).len(), 0);
        });
    }

    #[test]
    fn recipient_contains_token_true_after_add() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 11).unwrap();
            assert!(recipient_contains_token(env, &r, 11));
        });
    }

    #[test]
    fn recipient_contains_token_false_before_add() {
        with_contract(|env| {
            let r = Address::generate(env);
            assert!(!recipient_contains_token(env, &r, 11));
        });
    }

    #[test]
    fn recipient_contains_token_false_after_remove() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 11).unwrap();
            remove_token_from_recipient(env, &r, 11);
            assert!(!recipient_contains_token(env, &r, 11));
        });
    }

    // ── Prevent duplicate entries ─────────────────────────────────────────────

    #[test]
    fn duplicate_add_returns_duplicate_record_error() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 5).unwrap();
            assert_eq!(
                add_token_to_recipient(env, &r, 5),
                Err(Error::DuplicateRecord)
            );
        });
    }

    #[test]
    fn duplicate_add_does_not_grow_the_list() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 5).unwrap();
            let _ = add_token_to_recipient(env, &r, 5);
            assert_eq!(recipient_token_count(env, &r), 1);
        });
    }

    // ── Index isolation ───────────────────────────────────────────────────────

    #[test]
    fn indexes_are_isolated_per_recipient() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            add_token_to_recipient(env, &alice, 1).unwrap();
            add_token_to_recipient(env, &alice, 2).unwrap();
            add_token_to_recipient(env, &bob, 3).unwrap();

            assert_eq!(recipient_token_count(env, &alice), 2);
            assert_eq!(recipient_token_count(env, &bob), 1);
            assert!(!recipient_contains_token(env, &alice, 3));
            assert!(!recipient_contains_token(env, &bob, 1));
        });
    }

    #[test]
    fn same_token_can_appear_in_multiple_recipient_indexes() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            add_token_to_recipient(env, &alice, 1).unwrap();
            add_token_to_recipient(env, &bob, 1).unwrap();
            assert!(recipient_contains_token(env, &alice, 1));
            assert!(recipient_contains_token(env, &bob, 1));
        });
    }

    // ── Add-remove-re-add cycle ───────────────────────────────────────────────

    #[test]
    fn token_can_be_re_added_after_removal() {
        with_contract(|env| {
            let r = Address::generate(env);
            add_token_to_recipient(env, &r, 5).unwrap();
            remove_token_from_recipient(env, &r, 5);
            add_token_to_recipient(env, &r, 5).unwrap(); // must not error
            assert_eq!(recipient_token_count(env, &r), 1);
        });
    }

    // ── Count ─────────────────────────────────────────────────────────────────

    #[test]
    fn count_increases_on_add_decreases_on_remove() {
        with_contract(|env| {
            let r = Address::generate(env);
            assert_eq!(recipient_token_count(env, &r), 0);
            add_token_to_recipient(env, &r, 1).unwrap();
            assert_eq!(recipient_token_count(env, &r), 1);
            add_token_to_recipient(env, &r, 2).unwrap();
            assert_eq!(recipient_token_count(env, &r), 2);
            remove_token_from_recipient(env, &r, 1);
            assert_eq!(recipient_token_count(env, &r), 1);
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. ROYALTY PERCENTAGE STORAGE  (royalty_percentage module)
// ═════════════════════════════════════════════════════════════════════════════

mod royalty_percentage_storage_tests {
    use super::*;
    use clips_nft::royalty_percentage::{get_royalty_percentage, set_royalty_percentage};

    #[test]
    fn set_and_get_royalty_percentage() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 500).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 500);
        });
    }

    #[test]
    fn percentage_accepts_zero() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 0).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 0);
        });
    }

    #[test]
    fn percentage_accepts_max_bps() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, MAX_ROYALTY_BPS).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn percentage_rejects_above_max() {
        with_contract(|env| {
            assert_eq!(
                set_royalty_percentage(env, 1, MAX_ROYALTY_BPS + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn percentage_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_royalty_percentage(env, 99), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn percentage_can_be_overwritten() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 250).unwrap();
            set_royalty_percentage(env, 1, 750).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 750);
        });
    }

    #[test]
    fn percentages_are_isolated_per_token() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 100).unwrap();
            set_royalty_percentage(env, 2, 200).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 100);
            assert_eq!(get_royalty_percentage(env, 2).unwrap(), 200);
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 7. INVALID CONFIGURATIONS
// ═════════════════════════════════════════════════════════════════════════════

mod invalid_config_tests {
    use super::*;
    use clips_nft::royalty_validator::{validate_royalty, validate_royalty_bps};

    // ── validate_royalty ──────────────────────────────────────────────────────

    #[test]
    fn validate_royalty_passes_for_valid_bps() {
        let env = Env::default();
        for bps in [0, 1, 500, 5_000, MAX_ROYALTY_BPS] {
            assert!(
                validate_royalty(&make_royalty(&env, bps)).is_ok(),
                "expected Ok for bps = {bps}"
            );
        }
    }

    #[test]
    fn validate_royalty_fails_for_above_max() {
        let env = Env::default();
        assert_eq!(
            validate_royalty(&make_royalty(&env, MAX_ROYALTY_BPS + 1)),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn validate_royalty_fails_for_u32_max() {
        let env = Env::default();
        assert_eq!(
            validate_royalty(&make_royalty(&env, u32::MAX)),
            Err(Error::InvalidBasisPoints)
        );
    }

    // ── validate_royalty_bps ──────────────────────────────────────────────────

    #[test]
    fn validate_royalty_bps_accepts_zero() {
        assert!(validate_royalty_bps(0).is_ok());
    }

    #[test]
    fn validate_royalty_bps_accepts_max() {
        assert!(validate_royalty_bps(MAX_ROYALTY_BPS).is_ok());
    }

    #[test]
    fn validate_royalty_bps_rejects_one_over_max() {
        assert_eq!(
            validate_royalty_bps(MAX_ROYALTY_BPS + 1),
            Err(Error::InvalidBasisPoints)
        );
    }

    // ── RoyaltyRecipient validation ───────────────────────────────────────────

    #[test]
    fn validate_royalty_recipient_struct_passes_valid() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            };
            assert!(validate_royalty_recipient_struct(env, &r).is_ok());
        });
    }

    #[test]
    fn validate_royalty_recipient_struct_rejects_contract_self() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: env.current_contract_address(),
                basis_points: 500,
            };
            assert_eq!(
                validate_royalty_recipient_struct(env, &r),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn validate_royalty_recipient_struct_rejects_bps_above_max() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: MAX_ROYALTY_BPS + 1,
            };
            assert_eq!(
                validate_royalty_recipient_struct(env, &r),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn new_royalty_recipient_succeeds_for_valid_input() {
        with_contract(|env| {
            let addr = Address::generate(env);
            let r = new_royalty_recipient(env, addr.clone(), 750).unwrap();
            assert_eq!(r.recipient, addr);
            assert_eq!(r.basis_points, 750);
        });
    }

    #[test]
    fn new_royalty_recipient_rejects_invalid_bps() {
        with_contract(|env| {
            assert_eq!(
                new_royalty_recipient(env, Address::generate(env), 20_000),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn new_royalty_recipient_rejects_contract_address() {
        with_contract(|env| {
            assert_eq!(
                new_royalty_recipient(env, env.current_contract_address(), 500),
                Err(Error::InvalidRecipient)
            );
        });
    }

    // ── RoyaltyConfig validate() ───────────────────────────────────────────────

    #[test]
    fn royalty_config_validate_passes_for_in_range_bps() {
        let env = Env::default();
        let cfg = clips_nft::RoyaltyConfig {
            recipient: Address::generate(&env),
            royalty_bps: 500,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn royalty_config_validate_rejects_above_max() {
        let env = Env::default();
        let cfg = clips_nft::RoyaltyConfig {
            recipient: Address::generate(&env),
            royalty_bps: MAX_ROYALTY_BPS + 1,
        };
        // RoyaltyConfig uses its own Error::RoyaltyTooHigh variant
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn royalty_config_validate_accepts_zero_bps() {
        let env = Env::default();
        let cfg = clips_nft::RoyaltyConfig {
            recipient: Address::generate(&env),
            royalty_bps: 0,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn royalty_config_validate_accepts_max_bps() {
        let env = Env::default();
        let cfg = clips_nft::RoyaltyConfig {
            recipient: Address::generate(&env),
            royalty_bps: MAX_ROYALTY_BPS,
        };
        assert!(cfg.validate().is_ok());
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 8. MINT ROYALTY INITIALISATION  (mint_royalty_init — ties all pieces together)
// ═════════════════════════════════════════════════════════════════════════════

mod mint_royalty_init_tests {
    use super::*;
    use clips_nft::default_royalty::{set_default_royalty_bps, DEFAULT_ROYALTY_BPS};
    use clips_nft::mint_royalty_init::{
        initialize_nft_royalty, initialize_nft_royalty_from_royalty, RoyaltyInitParams,
    };
    use clips_nft::royalty_percentage::get_royalty_percentage;
    use clips_nft::royalty_storage::get_royalty;

    #[test]
    fn init_saves_explicit_recipient_and_bps() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(recipient.clone()),
                basis_points: Some(750),
                asset_address: None,
            };
            let royalty = initialize_nft_royalty(env, 1, &params, &owner).unwrap();
            assert_eq!(royalty.recipient, recipient);
            assert_eq!(royalty.basis_points, 750);

            // All three storage keys must be written atomically.
            let stored = get_royalty(env, 1).unwrap();
            assert_eq!(stored.recipient, recipient);
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 750);
        });
    }

    #[test]
    fn init_falls_back_to_default_bps_when_none_supplied() {
        with_contract(|env| {
            set_default_royalty_bps(env, 300).unwrap();
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: None,
                basis_points: None,
                asset_address: None,
            };
            let royalty = initialize_nft_royalty(env, 2, &params, &owner).unwrap();
            assert_eq!(royalty.basis_points, 300);
            assert_eq!(royalty.recipient, owner);
        });
    }

    #[test]
    fn init_uses_builtin_default_when_never_configured() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: None,
                basis_points: None,
                asset_address: None,
            };
            let royalty = initialize_nft_royalty(env, 3, &params, &owner).unwrap();
            assert_eq!(royalty.basis_points, DEFAULT_ROYALTY_BPS);
        });
    }

    #[test]
    fn init_rejects_invalid_recipient() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            let params = RoyaltyInitParams {
                recipient: Some(contract.clone()),
                basis_points: Some(500),
                asset_address: None,
            };
            assert_eq!(
                initialize_nft_royalty(env, 4, &params, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn init_rejects_bps_above_max() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(owner.clone()),
                basis_points: Some(MAX_ROYALTY_BPS + 1),
                asset_address: None,
            };
            assert_eq!(
                initialize_nft_royalty(env, 5, &params, &owner),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn init_from_full_royalty_struct() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipient: recipient.clone(),
                basis_points: 250,
                asset_address: None,
            };
            let stored = initialize_nft_royalty_from_royalty(env, 6, &royalty).unwrap();
            assert_eq!(stored.recipient, recipient);
            assert_eq!(stored.basis_points, 250);
            assert_eq!(get_royalty_percentage(env, 6).unwrap(), 250);
        });
    }

    #[test]
    fn init_with_zero_bps_is_valid() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(owner.clone()),
                basis_points: Some(0),
                asset_address: None,
            };
            let royalty = initialize_nft_royalty(env, 7, &params, &owner).unwrap();
            assert_eq!(royalty.basis_points, 0);
        });
    }

    #[test]
    fn init_with_max_bps_is_valid() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(owner.clone()),
                basis_points: Some(MAX_ROYALTY_BPS),
                asset_address: None,
            };
            let royalty = initialize_nft_royalty(env, 8, &params, &owner).unwrap();
            assert_eq!(royalty.basis_points, MAX_ROYALTY_BPS);
        });
    }
}
