#![cfg(test)]

use clips_nft::{storage::keys::StorageKey, AtomicMintContract};
use soroban_sdk::Env;

#[test]
fn instance_storage_supports_read_write_update_and_delete() {
    let env = Env::default();
    let contract_id = env.register_contract(None, AtomicMintContract);

    env.as_contract(&contract_id, || {
        let key = StorageKey::NextTokenId;
        let storage = env.storage().instance();

        assert!(!storage.has(&key));

        storage.set(&key, &1u32);
        assert_eq!(storage.get::<_, u32>(&key), Some(1));

        storage.set(&key, &2u32);
        assert_eq!(storage.get::<_, u32>(&key), Some(2));

        storage.remove(&key);
        assert!(!storage.has(&key));
        assert_eq!(storage.get::<_, u32>(&key), None);
    });
}
