//! Integration tests for storage infrastructure modules.

#![cfg(test)]

use clips_nft::{
    deserialize_metadata, deserialize_royalty, deserialize_token, get_migration_version,
    get_upgrade_timestamp, is_fully_migrated, migrate_to_current, record_upgrade, run_migrations,
    AtomicMintContract, AtomicMintContractClient, DataKey, Error, Royalty, TokenData,
    CONTRACT_VERSION, CURRENT_MIGRATION_VERSION, DEFAULT_ROYALTY_BPS, INITIAL_MIGRATION_VERSION,
    MAX_ROYALTY_BPS, VERSION,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

struct TestCtx {
    env: Env,
    contract_id: Address,
    admin: Address,
}

fn setup() -> TestCtx {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    TestCtx {
        env,
        contract_id,
        admin,
    }
}

fn with_contract<F, R>(ctx: &TestCtx, f: F) -> R
where
    F: FnOnce() -> R,
{
    ctx.env.as_contract(&ctx.contract_id, f)
}

#[test]
fn test_version_record_defaults_before_migration() {
    let ctx = setup();
    with_contract(&ctx, || {
        assert_eq!(get_migration_version(&ctx.env), INITIAL_MIGRATION_VERSION);
        assert_eq!(get_upgrade_timestamp(&ctx.env), 0);
    });
}

#[test]
fn test_record_upgrade_persists_version_and_timestamp() {
    let ctx = setup();
    ctx.env.ledger().set_timestamp(1_700_000_000);
    with_contract(&ctx, || {
        record_upgrade(&ctx.env, 1, ctx.env.ledger().timestamp());
        assert_eq!(get_migration_version(&ctx.env), 1);
        assert_eq!(get_upgrade_timestamp(&ctx.env), 1_700_000_000);
    });
}

#[test]
fn test_migrate_to_current_is_idempotent() {
    let ctx = setup();
    ctx.env.ledger().set_timestamp(100);

    with_contract(&ctx, || {
        let v1 = migrate_to_current(&ctx.env).expect("first migrate");
        assert_eq!(v1, CURRENT_MIGRATION_VERSION);
        assert!(is_fully_migrated(&ctx.env));

        let v2 = migrate_to_current(&ctx.env).expect("second migrate");
        assert_eq!(v2, CURRENT_MIGRATION_VERSION);
        assert_eq!(get_migration_version(&ctx.env), CURRENT_MIGRATION_VERSION);
    });
}

#[test]
fn test_migrate_seeds_total_supply_from_next_token_id() {
    let ctx = setup();
    with_contract(&ctx, || {
        ctx.env
            .storage()
            .instance()
            .set(&DataKey::NextTokenId, &5u32);
        ctx.env.storage().instance().remove(&DataKey::TotalSupply);

        run_migrations(&ctx.env, CURRENT_MIGRATION_VERSION).expect("migrate");

        let total: u32 = ctx
            .env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .expect("TotalSupply seeded");
        assert_eq!(total, 5);
    });
}

#[test]
fn test_deserialize_token_roundtrip() {
    let ctx = setup();
    with_contract(&ctx, || {
        let owner = Address::generate(&ctx.env);
        let data = TokenData {
            owner: owner.clone(),
            clip_id: 42,
        };
        ctx.env
            .storage()
            .persistent()
            .set(&DataKey::Token(1), &data);

        let loaded = deserialize_token(&ctx.env, 1).expect("valid token");
        assert_eq!(loaded.owner, owner);
        assert_eq!(loaded.clip_id, 42);
    });
}

#[test]
fn test_deserialize_metadata_rejects_empty_uri() {
    let ctx = setup();
    with_contract(&ctx, || {
        let empty = String::from_str(&ctx.env, "");
        ctx.env
            .storage()
            .persistent()
            .set(&DataKey::Metadata(1), &empty);

        assert_eq!(
            deserialize_metadata(&ctx.env, 1),
            Err(Error::CorruptedStorage)
        );
    });
}

#[test]
fn test_deserialize_royalty_rejects_invalid_bps() {
    let ctx = setup();
    with_contract(&ctx, || {
        let royalty = Royalty {
            recipient: Address::generate(&ctx.env),
            basis_points: MAX_ROYALTY_BPS + 1,
            asset_address: None,
        };
        ctx.env
            .storage()
            .persistent()
            .set(&DataKey::Royalty(1), &royalty);

        assert!(matches!(
            deserialize_royalty(&ctx.env, 1),
            Err(Error::CorruptedStorage)
        ));
    });
}

#[test]
fn test_deserialize_royalty_accepts_valid_bps() {
    let ctx = setup();
    with_contract(&ctx, || {
        let royalty = Royalty {
            recipient: Address::generate(&ctx.env),
            basis_points: DEFAULT_ROYALTY_BPS,
            asset_address: None,
        };
        ctx.env
            .storage()
            .persistent()
            .set(&DataKey::Royalty(1), &royalty);

        let loaded = deserialize_royalty(&ctx.env, 1).expect("valid royalty");
        assert_eq!(loaded.basis_points, DEFAULT_ROYALTY_BPS);
    });
}

#[test]
fn test_version_constant_matches_storage_constants() {
    assert_eq!(VERSION, CONTRACT_VERSION);
}
