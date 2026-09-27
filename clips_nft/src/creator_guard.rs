//! Creator authorization guard for creator-owned NFT operations.

use soroban_sdk::{Address, Env};

use crate::creator_storage;
use crate::types::{Error, TokenId};

/// Verify that `caller` is the registered creator of `token_id`.
///
/// The creator record is read from persistent storage. A missing record is
/// propagated as [`Error::TokenNotFound`], while a different caller is
/// rejected as [`Error::Unauthorized`].
pub fn require_creator(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    let creator = creator_storage::get_creator(env, token_id)?;
    if creator != *caller {
        return Err(Error::Unauthorized);
    }

    caller.require_auth();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn registered_creator_passes_guard() {
        let env = Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert!(require_creator(&env, &creator, 1).is_ok());
    }

    #[test]
    fn different_caller_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let caller = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert_eq!(
            require_creator(&env, &caller, 1),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn missing_creator_record_is_rejected() {
        let env = Env::default();
        let caller = Address::generate(&env);

        assert_eq!(
            require_creator(&env, &caller, 1),
            Err(Error::TokenNotFound)
        );
    }
}