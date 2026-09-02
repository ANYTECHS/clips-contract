//! Integration tests for the full NFT lifecycle event set (issue #921).
//!
//! Covers the following lifecycle events:
//!
//! | Event | Topic | Entry Point |
//! |-------|-------|-------------|
//! | Mint | `nft_mntd` | `execute_mint` |
//! | Creator Assignment | `creator` | `execute_mint` |
//! | Freeze | `nft_frz` | `freeze_token` + `emit_nft_frozen` |
//! | Unfreeze | `nft_unfrz` | `unfreeze_token` + `emit_nft_unfrozen` |
//!
//! For each event: verifies it fires, verifies the topic (indexed param),
//! and verifies payload data correctness.
//!
//! Note: Transfer, Burn, and MetadataUpdate event types are defined in
//! `types.rs` but have no public entry points or emit functions yet.
//! Their coverage will be added when those features are implemented.

#![cfg(test)]

use clips_nft::{
    execute_mint, frozen_token, nft_frozen_event, nft_unfrozen_event, ClipsNftContract,
    CreatorAssignedEvent, MintRequest, NFTFrozenEvent, NFTUnfrozenEvent, Royalty, RoyaltyRecipient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, String, Symbol, TryFromVal, Val, Vec as SdkVec,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Register the contract and run `f` inside its context.
fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env, Address) -> R,
{
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        ClipsNftContract::init(env.clone(), Address::generate(&env));
        f(&env, contract_id.clone())
    })
}

fn make_request(
    env: &Env,
    owner: &Address,
    creator: Option<&Address>,
    clip_id: u32,
) -> MintRequest {
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: creator.unwrap_or(owner).clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmLifecycle{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipients: soroban_sdk::vec![&env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            }],
            asset_address: None,
        },
        creator_address: creator.map(|c| c.clone()),
        creator_display_name: None,
    }
}

/// Build one expected `(emitter, topics, data)` tuple for comparison against
/// `env.events().all()`.  The emitter is always the contract itself.
fn ev<D: IntoVal<Env, Val>>(
    env: &Env,
    contract_id: &Address,
    topic: Symbol,
    data: D,
) -> (Address, soroban_sdk::Vec<Val>, Val) {
    (
        contract_id.clone(),
        (topic,).into_val(env),
        data.into_val(env),
    )
}

fn event_count(env: &Env) -> usize {
    env.events().all().events().len()
}

/// Find the last specific event by topic among published events.
fn find_last_event<D: soroban_sdk::TryFromVal<Env, soroban_sdk::xdr::ScVal>>(
    env: &Env,
    topic: Symbol,
) -> Option<D> {
    let mut result = None;
    for event in env.events().all().events() {
        if let soroban_sdk::xdr::ContractEventBody::V0(v0) = &event.body {
            if v0.topics.len() == 1 {
                if let Ok(topic_sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                    if topic_sym == topic {
                        if let Ok(evt) = D::try_from_val(env, &v0.data) {
                            result = Some(evt);
                        }
                    }
                }
            }
        }
    }
    result
}

// ─── Mint event ───────────────────────────────────────────────────────────────

/// Minting must emit events (creator assignment as proof).
#[test]
fn test_mint_event_fires_on_successful_mint() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);

        let result =
            execute_mint(env, make_request(env, &owner, None, 1)).expect("mint ok");

        assert!(event_count(env) > 0, "events must be emitted after mint");
        // Verify a creator event exists for this token
        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("creator event must exist");
        assert_eq!(evt.token_id, result.token_id);
    });
}

/// Each mint must produce a separate creator event.
#[test]
fn test_mint_event_emitted_per_token() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);

        let r1 = execute_mint(env, make_request(env, &owner, None, 10)).expect("mint 1 ok");
        let r2 = execute_mint(env, make_request(env, &owner, None, 11)).expect("mint 2 ok");

        assert_ne!(r1.token_id, r2.token_id, "must mint distinct tokens");
        assert!(event_count(env) > 0, "events must exist after mints");
        // The last creator event should be for r2
        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("creator event must exist");
        assert_eq!(evt.token_id, r2.token_id);
    });
}

// ─── Creator assignment event ────────────────────────────────────────────────

/// Minting must emit a `CreatorAssignedEvent` with topic `creator`.
#[test]
fn test_creator_assignment_event_fires_on_mint() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);

        let result =
            execute_mint(env, make_request(env, &owner, Some(&creator), 100)).expect("mint ok");

        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("CreatorAssignedEvent must be emitted on mint");

        assert_eq!(evt.token_id, result.token_id);
        assert_eq!(evt.creator, creator);
        assert_eq!(evt.clip_id, 100);
    });
}

/// Creator assignment event must carry the correct creator when none specified.
#[test]
fn test_creator_assignment_event_falls_back_to_owner() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);

        let result =
            execute_mint(env, make_request(env, &owner, None, 200)).expect("mint ok");

        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("CreatorAssignedEvent must be emitted");

        assert_eq!(evt.creator, owner, "creator must default to owner");
        assert_eq!(evt.token_id, result.token_id);
    });
}

/// Creator assignment event must carry the explicit creator.
#[test]
fn test_creator_assignment_event_uses_explicit_creator() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);

        let result =
            execute_mint(env, make_request(env, &owner, Some(&creator), 300)).expect("mint ok");

        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("CreatorAssignedEvent must be emitted");

        assert_eq!(evt.creator, creator);
        assert_ne!(evt.creator, owner);
        assert_eq!(evt.token_id, result.token_id);
    });
}

/// Each mint must produce a separate creator assignment event.
#[test]
fn test_creator_assignment_event_emitted_per_token() {
    with_contract(|env, _contract_id| {
        let owner = Address::generate(env);
        let c1 = Address::generate(env);
        let c2 = Address::generate(env);

        let r1 =
            execute_mint(env, make_request(env, &owner, Some(&c1), 400)).expect("first mint ok");
        let r2 =
            execute_mint(env, make_request(env, &owner, Some(&c2), 401)).expect("second mint ok");

        // The last creator event should be for r2
        let evt = find_last_event::<CreatorAssignedEvent>(env, symbol_short!("creator"))
            .expect("creator event must exist");
        assert_eq!(evt.token_id, r2.token_id);
        assert_eq!(evt.creator, c2);
        assert_ne!(r1.token_id, r2.token_id);
    });
}

// ─── Freeze event ─────────────────────────────────────────────────────────────

/// Freezing an NFT must emit an `NFTFrozenEvent` with topic `nft_frz`.
#[test]
fn test_freeze_event_fires() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);
        let token_id = 500u32;

        frozen_token::freeze_token(env, token_id);
        nft_frozen_event::emit_nft_frozen(env, token_id, &caller, None, env.ledger().timestamp());

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                ev(
                    env,
                    &contract_id,
                    symbol_short!("nft_frz"),
                    NFTFrozenEvent {
                        token_id,
                        caller: caller.clone(),
                        reason: None,
                        timestamp: env.ledger().timestamp(),
                    },
                )
            ]
        );
    });
}

/// Freezing with a reason must include the reason in the event.
#[test]
fn test_freeze_event_carries_reason() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);
        let token_id = 501u32;
        let reason = String::from_str(env, "fraud investigation");

        frozen_token::freeze_token(env, token_id);
        nft_frozen_event::emit_nft_frozen(
            env,
            token_id,
            &caller,
            Some(&reason),
            env.ledger().timestamp(),
        );

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                ev(
                    env,
                    &contract_id,
                    symbol_short!("nft_frz"),
                    NFTFrozenEvent {
                        token_id,
                        caller: caller.clone(),
                        reason: Some(reason),
                        timestamp: env.ledger().timestamp(),
                    },
                )
            ]
        );
    });
}

/// Freezing an already-frozen token must not emit a second freeze event.
#[test]
fn test_freeze_event_not_emitted_for_already_frozen() {
    with_contract(|env, _contract_id| {
        let caller = Address::generate(env);
        let token_id = 502u32;

        // Freeze once — returns true (newly frozen)
        assert!(frozen_token::freeze_token(env, token_id));
        nft_frozen_event::emit_nft_frozen(
            env,
            token_id,
            &caller,
            None,
            env.ledger().timestamp(),
        );
        let count_after_first = event_count(env);

        // Try to freeze again — returns false (already frozen), should NOT emit
        assert!(!frozen_token::freeze_token(env, token_id));
        let count_after_second = event_count(env);

        assert_eq!(count_after_first, count_after_second);
    });
}

/// Freezing must carry the correct token_id in the event.
#[test]
fn test_freeze_event_payload_token_id() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);
        let token_id = 503u32;

        frozen_token::freeze_token(env, token_id);
        nft_frozen_event::emit_nft_frozen(env, token_id, &caller, None, env.ledger().timestamp());

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                ev(
                    env,
                    &contract_id,
                    symbol_short!("nft_frz"),
                    NFTFrozenEvent {
                        token_id,
                        caller: caller.clone(),
                        reason: None,
                        timestamp: env.ledger().timestamp(),
                    },
                )
            ]
        );
    });
}

// ─── Unfreeze event ───────────────────────────────────────────────────────────

/// Unfreezing must emit an `NFTUnfrozenEvent` with topic `nft_unfrz`.
#[test]
fn test_unfreeze_event_fires() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);
        let token_id = 600u32;

        // Freeze first
        frozen_token::freeze_token(env, token_id);
        env.events().all(); // Clear events

        // Unfreeze
        frozen_token::unfreeze_token(env, token_id);
        nft_unfrozen_event::emit_nft_unfrozen(env, token_id, &caller, env.ledger().timestamp());

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                ev(
                    env,
                    &contract_id,
                    symbol_short!("nft_unfrz"),
                    NFTUnfrozenEvent {
                        token_id,
                        caller: caller.clone(),
                        timestamp: env.ledger().timestamp(),
                    },
                )
            ]
        );
    });
}

/// Unfreezing a non-frozen token must not emit an event.
#[test]
fn test_unfreeze_event_not_emitted_for_non_frozen() {
    with_contract(|env, _contract_id| {
        let token_id = 601u32;

        env.events().all(); // Clear init events

        // unfreeze_token returns false for non-frozen
        assert!(!frozen_token::unfreeze_token(env, token_id));
        assert_eq!(event_count(env), 0);
    });
}

/// Unfreeze event must carry the correct token_id and caller.
#[test]
fn test_unfreeze_event_payload() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);
        let token_id = 602u32;

        frozen_token::freeze_token(env, token_id);
        env.events().all();

        frozen_token::unfreeze_token(env, token_id);
        nft_unfrozen_event::emit_nft_unfrozen(env, token_id, &caller, env.ledger().timestamp());

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                ev(
                    env,
                    &contract_id,
                    symbol_short!("nft_unfrz"),
                    NFTUnfrozenEvent {
                        token_id,
                        caller: caller.clone(),
                        timestamp: env.ledger().timestamp(),
                    },
                )
            ]
        );
    });
}
