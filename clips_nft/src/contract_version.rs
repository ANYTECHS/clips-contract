//! Contract version storage — persists migration version and upgrade timestamp.
//!
//! # Storage
//! Key: `DataKey::ContractVersion` (instance storage)
//!
//! The stored [`ContractVersionRecord`] tracks which migration steps have been
//! applied and when the contract was last upgraded.

use soroban_sdk::{contracttype, Env};

use crate::storage_constants::{DEFAULT_UPGRADE_TIMESTAMP, INITIAL_MIGRATION_VERSION};
use crate::types::DataKey;

/// Metadata persisted across contract upgrades.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractVersionRecord {
    /// Highest migration step that has been successfully applied.
    pub migration_version: u32,
    /// Ledger timestamp of the most recent upgrade / migration.
    pub upgrade_timestamp: u64,
}

impl ContractVersionRecord {
    /// Default record for freshly deployed contracts (pre-migration).
    pub fn initial() -> Self {
        Self {
            migration_version: INITIAL_MIGRATION_VERSION,
            upgrade_timestamp: DEFAULT_UPGRADE_TIMESTAMP,
        }
    }
}

/// Load the stored version record, falling back to [`ContractVersionRecord::initial`].
pub fn get_version_record(env: &Env) -> ContractVersionRecord {
    env.storage()
        .instance()
        .get(&DataKey::ContractVersion)
        .unwrap_or_else(ContractVersionRecord::initial)
}

/// Return the stored migration version (0 before any migration runs).
pub fn get_migration_version(env: &Env) -> u32 {
    get_version_record(env).migration_version
}

/// Return the ledger timestamp recorded at the last upgrade.
pub fn get_upgrade_timestamp(env: &Env) -> u64 {
    get_version_record(env).upgrade_timestamp
}

/// Persist a full version record.
pub fn set_version_record(env: &Env, record: &ContractVersionRecord) {
    env.storage()
        .instance()
        .set(&DataKey::ContractVersion, record);
}

/// Record a successful migration step with the current ledger timestamp.
pub fn record_upgrade(env: &Env, migration_version: u32, timestamp: u64) {
    set_version_record(
        env,
        &ContractVersionRecord {
            migration_version,
            upgrade_timestamp: timestamp,
        },
    );
}
