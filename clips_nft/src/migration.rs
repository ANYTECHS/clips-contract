//! Storage migration helper — versioned, idempotent migration runner.
//!
//! Migrations are applied sequentially from the stored migration version up to
//! the requested target. Each step is safe to re-run: already-applied steps are
//! skipped and individual step functions are idempotent.

use soroban_sdk::{contracttype, Env};

use crate::contract_version::{get_migration_version, record_upgrade};
use crate::storage_constants::{event_keys, CURRENT_MIGRATION_VERSION};
use crate::types::{DataKey, Error};

/// A single migration step identified by its target version number.
pub struct MigrationStep {
    pub version: u32,
    pub migrate: fn(&Env) -> Result<(), Error>,
}

/// Event emitted after each migration step completes.
#[contracttype]
#[derive(Clone)]
pub struct MigrationEvent {
    pub from_version: u32,
    pub to_version: u32,
    pub timestamp: u64,
}

/// Run all pending migrations up to `target_version`.
///
/// Returns the migration version after execution. Safe to call multiple times —
/// already-applied steps are skipped.
pub fn run_migrations(env: &Env, target_version: u32) -> Result<u32, Error> {
    let current = get_migration_version(env);
    if current >= target_version {
        return Ok(current);
    }

    for step in migration_steps() {
        if step.version <= current {
            continue;
        }
        if step.version > target_version {
            break;
        }
        (step.migrate)(env)?;
        let ts = env.ledger().timestamp();
        record_upgrade(env, step.version, ts);
        env.events().publish(
            (event_keys::MIGRATION,),
            MigrationEvent {
                from_version: step.version.saturating_sub(1),
                to_version: step.version,
                timestamp: ts,
            },
        );
    }

    Ok(get_migration_version(env))
}

/// Migrate to the current release's target migration version.
pub fn migrate_to_current(env: &Env) -> Result<u32, Error> {
    run_migrations(env, CURRENT_MIGRATION_VERSION)
}

/// Returns true when no further migrations are pending.
pub fn is_fully_migrated(env: &Env) -> bool {
    get_migration_version(env) >= CURRENT_MIGRATION_VERSION
}

fn migration_steps() -> &'static [MigrationStep] {
    &[MigrationStep {
        version: 1,
        migrate: migrate_v0_to_v1,
    }]
}

/// v0 → v1: seed `TotalSupply` from `NextTokenId` when the key is absent.
fn migrate_v0_to_v1(env: &Env) -> Result<(), Error> {
    if !env.storage().instance().has(&DataKey::TotalSupply) {
        let next: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &next);
    }
    Ok(())
}
