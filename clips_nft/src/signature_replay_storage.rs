//! Signature replay protection storage.
//!
//! Persists consumed backend signature hashes so the same signed mint payload
//! cannot be replayed to mint additional NFTs.
//!
//! # Storage
//! Key: `DataKey::UsedSignature(hash)` → `bool` (persistent)

use soroban_sdk::{Bytes, BytesN, Env};

use crate::types::{DataKey, Error};

/// Derive the persistent storage key from raw signature bytes.
pub fn hash_signature(env: &Env, signature: &BytesN<64>) -> BytesN<32> {
    env.crypto()
        .sha256(&Bytes::from_array(env, &signature.to_array()))
        .into()
}

/// Return `true` if `signature_hash` has already been consumed.
pub fn is_signature_used(env: &Env, signature_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::UsedSignature(signature_hash.clone()))
}

/// Reject replayed signatures before any mint state is written.
pub fn ensure_signature_unused(env: &Env, signature_hash: &BytesN<32>) -> Result<(), Error> {
    if is_signature_used(env, signature_hash) {
        return Err(Error::SignatureAlreadyUsed);
    }
    Ok(())
}

/// Persist a signature hash after a successful mint.
pub fn mark_signature_used(env: &Env, signature_hash: &BytesN<32>) -> Result<(), Error> {
    if is_signature_used(env, signature_hash) {
        return Err(Error::SignatureAlreadyUsed);
    }
    env.storage()
        .persistent()
        .set(&DataKey::UsedSignature(signature_hash.clone()), &true);
    Ok(())
}

/// Remove a consumed signature marker (used by atomic mint rollback).
pub fn unmark_signature_used(env: &Env, signature_hash: &BytesN<32>) {
    env.storage()
        .persistent()
        .remove(&DataKey::UsedSignature(signature_hash.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::BytesN as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn stores_signature_hash_and_detects_duplicate() {
        with_contract(|env| {
            let hash = BytesN::<32>::random(env);
            assert!(!is_signature_used(env, &hash));
            mark_signature_used(env, &hash).unwrap();
            assert!(is_signature_used(env, &hash));
            assert_eq!(
                ensure_signature_unused(env, &hash),
                Err(Error::SignatureAlreadyUsed)
            );
        });
    }

    #[test]
    fn mark_signature_used_rejects_double_write() {
        with_contract(|env| {
            let hash = BytesN::<32>::random(env);
            mark_signature_used(env, &hash).unwrap();
            assert_eq!(
                mark_signature_used(env, &hash),
                Err(Error::SignatureAlreadyUsed)
            );
        });
    }

    #[test]
    fn hash_signature_is_deterministic() {
        let env = Env::default();
        let sig = BytesN::<64>::random(&env);
        let h1 = hash_signature(&env, &sig);
        let h2 = hash_signature(&env, &sig);
        assert_eq!(h1, h2);
    }
}
