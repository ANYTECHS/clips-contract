//! Caller validation guard (issue #1409).
//!
//! Centralized entry point for validating the identity of a caller before a
//! protected contract function runs. Retrieves the caller, checks that it is
//! structurally valid (not the contract itself, not blacklisted), and returns
//! errors from the contract's centralized [`crate::types::Error`] catalog.

use crate::caller_validator;
use crate::types::Error;
use soroban_sdk::{Address, Env};

/// Retrieve the caller address unchanged (#1409).
pub fn get_caller(env: &Env, caller: &Address) -> Address {
    caller_validator::get_caller(env, caller)
}

/// Validate caller presence and structural eligibility (#1409).
///
/// Returns `Error::InvalidAddress` for self-calls and `Error::Unauthorized`
/// for blacklisted callers.
pub fn require_valid_caller(env: &Env, caller: &Address) -> Result<(), Error> {
    caller_validator::validate_caller(env, caller)
}

/// Non-panicking probe version of [`require_valid_caller`].
pub fn is_valid_caller(env: &Env, caller: &Address) -> bool {
    require_valid_caller(env, caller).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::testutils::Address as _;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn get_caller_returns_the_supplied_address() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(get_caller(env, &caller), caller);
        });
    }

    #[test]
    fn valid_caller_is_accepted() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(is_valid_caller(env, &caller));
            assert_eq!(require_valid_caller(env, &caller), Ok(()));
        });
    }

    #[test]
    fn contract_self_call_is_rejected_with_centralized_error() {
        with_contract(|env| {
            let self_address = env.current_contract_address();
            assert!(!is_valid_caller(env, &self_address));
            assert_eq!(
                require_valid_caller(env, &self_address),
                Err(Error::InvalidAddress)
            );
        });
    }
}
