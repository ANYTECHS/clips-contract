use soroban_sdk::Env;

use crate::types::{DataKey, Error, ListingId};

/// Return the next marketplace listing ID without consuming it.
pub fn peek_next_listing_id(env: &Env) -> ListingId {
    env.storage()
        .instance()
        .get(&DataKey::NextListingId)
        .unwrap_or(1)
}

/// Generate a unique marketplace listing ID.
pub fn generate_listing_id(env: &Env) -> Result<ListingId, Error> {
    let current_id = peek_next_listing_id(env);
    let next_id = current_id.checked_add(1).ok_or(Error::InvalidLimit)?;
    env.storage()
        .instance()
        .set(&DataKey::NextListingId, &next_id);
    Ok(current_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;

    fn with_contract<F>(test: F)
    where
        F: FnOnce(&Env),
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || test(&env));
    }

    #[test]
    fn generates_sequential_listing_ids() {
        with_contract(|env| {
            assert_eq!(generate_listing_id(env).unwrap(), 1);
            assert_eq!(generate_listing_id(env).unwrap(), 2);
            assert_eq!(peek_next_listing_id(env), 3);
        });
    }

    #[test]
    fn prevents_listing_id_overflow() {
        with_contract(|env| {
            env.storage()
                .instance()
                .set(&DataKey::NextListingId, &ListingId::MAX);

            assert_eq!(generate_listing_id(env), Err(Error::InvalidLimit));
            assert_eq!(peek_next_listing_id(env), ListingId::MAX);
        });
    }
}
