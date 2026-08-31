//! Integration tests for the administrative lifecycle events
//! (issues #931, #932, #933, #934).
//!
//! Covers the four events end-to-end through the contract client:
//!
//! | Issue | Event | Topic |
//! |-------|-------|-------|
//! | #933  | [`ContractPausedEvent`]    | `ctr_pause` |
//! | #934  | [`ContractUnpausedEvent`]  | `ctr_unpse` |
//! | #932  | [`ConfigUpdatedEvent`]     | `cfg_updt`  |
//! | #931  | [`ApprovalRevokedEvent`]   | `aprv_rvk`  |
//!
//! Every test asserts both that the event fires on a real state change and that
//! it stays silent on a no-op, so a subscriber can treat each event as proof
//! that the state actually changed.

#![cfg(test)]

use clips_nft::{
    operator_approval, token_approval, ApprovalRevokedEvent, ApprovalScope, ClipsNftContract,
    ClipsNftContractClient, ConfigField, ConfigUpdatedEvent, ConfigValue, ContractPausedEvent,
    ContractUnpausedEvent, Error,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, String, Symbol, Val, Vec,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (Env, ClipsNftContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, client, admin, contract_id)
}

/// Number of published events recorded so far.
/// Number of events published by the most recent top-level invocation.
///
/// `env.events().all()` is reset by every contract call, so this must be read
/// immediately after the call under test.
fn event_count(env: &Env) -> usize {
    env.events().all().events().len()
}

/// Build one expected `(emitter, topics, data)` tuple for comparison against
/// `env.events().all()`.
fn ev<D: IntoVal<Env, Val>>(
    env: &Env,
    contract_id: &Address,
    topic: Symbol,
    data: D,
) -> (Address, Vec<Val>, Val) {
    (
        contract_id.clone(),
        (topic,).into_val(env),
        data.into_val(env),
    )
}

// ─── #933 — contract paused ───────────────────────────────────────────────────

#[test]
fn pause_emits_contract_paused_event() {
    let (env, client, admin, contract_id) = setup();

    client.pause(&admin, &None);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("ctr_pause"),
                ContractPausedEvent {
                    admin: admin.clone(),
                    reason: None,
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
    assert!(client.is_paused());
}

#[test]
fn pause_carries_the_optional_reason() {
    let (env, client, admin, contract_id) = setup();
    let reason = String::from_str(&env, "oracle outage");

    client.pause(&admin, &Some(reason.clone()));

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("ctr_pause"),
                ContractPausedEvent {
                    admin: admin.clone(),
                    reason: Some(reason),
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
}

#[test]
fn pausing_an_already_paused_contract_emits_nothing() {
    let (env, client, admin, _) = setup();
    client.pause(&admin, &None);

    assert_eq!(
        client.try_pause(&admin, &None),
        Err(Ok(Error::ContractPaused))
    );
    assert_eq!(event_count(&env), 0);
}

#[test]
fn a_non_admin_cannot_pause() {
    let (env, client, _, _) = setup();
    let intruder = Address::generate(&env);

    assert_eq!(
        client.try_pause(&intruder, &None),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    assert!(!client.is_paused());
    assert_eq!(event_count(&env), 0);
}

// ─── #934 — contract unpaused ────────────────────────────────────────────────

#[test]
fn unpause_emits_contract_unpaused_event() {
    let (env, client, admin, contract_id) = setup();
    client.pause(&admin, &None);

    client.unpause(&admin);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("ctr_unpse"),
                ContractUnpausedEvent {
                    admin: admin.clone(),
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
    assert!(!client.is_paused());
}

#[test]
fn unpausing_a_running_contract_emits_nothing() {
    let (env, client, admin, _) = setup();

    assert_eq!(client.try_unpause(&admin), Err(Ok(Error::NotPaused)));
    assert_eq!(event_count(&env), 0);
}

#[test]
fn a_non_admin_cannot_unpause() {
    let (env, client, admin, _) = setup();
    client.pause(&admin, &None);
    let intruder = Address::generate(&env);

    assert_eq!(
        client.try_unpause(&intruder),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    assert_eq!(event_count(&env), 0);
    assert!(client.is_paused());
}

// ─── #932 — configuration updated ────────────────────────────────────────────

#[test]
fn adding_a_supported_asset_emits_config_updated() {
    let (env, client, admin, contract_id) = setup();
    let asset = Address::generate(&env);

    client.add_currency(&admin, &asset);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("cfg_updt"),
                ConfigUpdatedEvent {
                    field: ConfigField::SupportedAsset,
                    previous_value: ConfigValue::Unset,
                    new_value: ConfigValue::Address(asset),
                    admin: admin.clone(),
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
}

#[test]
fn removing_a_supported_asset_emits_config_updated() {
    let (env, client, admin, contract_id) = setup();
    let asset = Address::generate(&env);
    client.add_currency(&admin, &asset);

    client.remove_currency(&admin, &asset);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("cfg_updt"),
                ConfigUpdatedEvent {
                    field: ConfigField::SupportedAsset,
                    previous_value: ConfigValue::Address(asset),
                    new_value: ConfigValue::Unset,
                    admin: admin.clone(),
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
}

#[test]
fn a_rejected_asset_change_emits_nothing() {
    let (env, client, admin, _) = setup();
    let asset = Address::generate(&env);
    client.add_currency(&admin, &asset);

    // Re-adding the same asset is rejected, so no configuration changed.
    assert!(client.try_add_currency(&admin, &asset).is_err());
    assert_eq!(event_count(&env), 0);
}

#[test]
fn a_non_admin_cannot_change_supported_assets() {
    let (env, client, _, _) = setup();
    let intruder = Address::generate(&env);
    let asset = Address::generate(&env);

    assert_eq!(
        client.try_add_currency(&intruder, &asset),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    assert_eq!(event_count(&env), 0);
}

// ─── #931 — approval revoked ─────────────────────────────────────────────────

#[test]
fn revoking_an_operator_emits_approval_revoked() {
    let (env, client, _, contract_id) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);
    env.as_contract(&contract_id, || {
        operator_approval::save_operator(&env, &owner, &operator);
    });

    assert!(client.revoke_operator_approval(&owner, &operator));

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("aprv_rvk"),
                ApprovalRevokedEvent {
                    owner: owner.clone(),
                    approved: operator.clone(),
                    scope: ApprovalScope::AllTokens,
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
}

#[test]
fn revoking_an_operator_that_was_never_approved_emits_nothing() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);

    assert!(!client.revoke_operator_approval(&owner, &operator));
    assert_eq!(event_count(&env), 0);
}

#[test]
fn revoking_a_token_approval_emits_the_token_scope() {
    let (env, _, _, contract_id) = setup();
    let owner = Address::generate(&env);
    let approved = Address::generate(&env);

    env.as_contract(&contract_id, || {
        token_approval::save_approval(&env, 7, &approved);
        assert_eq!(
            token_approval::revoke_approval(&env, &owner, 7),
            Some(approved.clone())
        );
        assert_eq!(token_approval::get_approval(&env, 7), None);
    });

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            ev(
                &env,
                &contract_id,
                symbol_short!("aprv_rvk"),
                ApprovalRevokedEvent {
                    owner: owner.clone(),
                    approved: approved.clone(),
                    scope: ApprovalScope::Token(7),
                    timestamp: env.ledger().timestamp(),
                },
            )
        ]
    );
}

#[test]
fn revoking_an_absent_token_approval_emits_nothing() {
    let (env, _, _, contract_id) = setup();
    let owner = Address::generate(&env);

    env.as_contract(&contract_id, || {
        assert_eq!(token_approval::revoke_approval(&env, &owner, 7), None);
    });
    assert_eq!(event_count(&env), 0);
}
