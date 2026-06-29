#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{storage_cleanup, types::DataKey, ClipCashNFT};

fn setup(env: &Env) -> Address {
    let contract_id = env.register_contract(None, ClipCashNFT);
    contract_id
}

#[test]
fn non_existent_token_is_not_orphaned() {
    let env = Env::default();
    let contract_id = setup(&env);
    env.as_contract(&contract_id, || {
        assert!(!storage_cleanup::is_record_orphaned(&env, 99));
    });
}

#[test]
fn token_with_primary_entry_is_not_orphaned() {
    let env = Env::default();
    let contract_id = setup(&env);
    env.as_contract(&contract_id, || {
        let owner = Address::generate(&env);
        let token_data = crate::types::TokenData { owner, clip_id: 1 };
        env.storage().persistent().set(&DataKey::Token(0), &token_data);

        assert!(!storage_cleanup::is_record_orphaned(&env, 0));
    });
}

#[test]
fn metadata_without_token_is_orphaned() {
    let env = Env::default();
    let contract_id = setup(&env);
    env.as_contract(&contract_id, || {
        let uri = String::from_str(&env, "ipfs://test");
        env.storage().persistent().set(&DataKey::Metadata(5), &uri);

        assert!(storage_cleanup::is_record_orphaned(&env, 5));
    });
}

#[test]
fn remove_token_records_clears_all_entries() {
    let env = Env::default();
    let contract_id = setup(&env);
    env.as_contract(&contract_id, || {
        let owner = Address::generate(&env);
        let token_data = crate::types::TokenData { owner, clip_id: 7 };
        let uri = String::from_str(&env, "ipfs://example");

        env.storage().persistent().set(&DataKey::Token(7), &token_data);
        env.storage().persistent().set(&DataKey::Metadata(7), &uri);

        storage_cleanup::remove_token_records(&env, 7);

        assert!(!env.storage().persistent().has(&DataKey::Token(7)));
        assert!(!env.storage().persistent().has(&DataKey::Metadata(7)));
        assert!(!storage_cleanup::is_record_orphaned(&env, 7));
    });
}

#[test]
fn remove_token_records_is_idempotent() {
    let env = Env::default();
    let contract_id = setup(&env);
    env.as_contract(&contract_id, || {
        // Removing records for a token that doesn't exist should not panic.
        storage_cleanup::remove_token_records(&env, 100);
        storage_cleanup::remove_token_records(&env, 100);
        assert!(!storage_cleanup::is_record_orphaned(&env, 100));
    });
}
