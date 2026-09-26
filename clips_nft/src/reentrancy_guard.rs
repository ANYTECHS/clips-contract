//! Reentrancy protection guard (Issue #1074).
//!
//! Prevents nested execution of sensitive contract operations (payments,
//! transfers, purchases) that make cross-contract calls.
//!
//! # Design
//!
//! A single lock flag is kept in temporary storage under [`REENTRANCY_LOCK`].
//! [`enter`] sets the flag and fails with [`Error::ReentrantCall`] if it is
//! already set; [`exit`] clears it. [`with_guard`] wraps both so the lock is
//! always released after the protected operation, whether it succeeds or
//! returns an error.
//!
//! # Usage
//!
//! ```rust,ignore
//! reentrancy_guard::with_guard(&env, || {
//!     // sensitive operation that may call external contracts
//!     Ok(())
//! })?;
//! ```

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::types::Error;

/// Storage key of the reentrancy lock flag.
pub const REENTRANCY_LOCK: Symbol = symbol_short!("REENTRY");

/// Return `true` while a protected operation is executing.
pub fn is_locked(env: &Env) -> bool {
    env.storage()
        .temporary()
        .get::<Symbol, bool>(&REENTRANCY_LOCK)
        .unwrap_or(false)
}

/// Acquire the lock before entering a protected operation.
///
/// # Errors
/// - [`Error::ReentrantCall`] if a protected operation is already executing.
pub fn enter(env: &Env) -> Result<(), Error> {
    if is_locked(env) {
        return Err(Error::ReentrantCall);
    }
    env.storage().temporary().set(&REENTRANCY_LOCK, &true);
    Ok(())
}

/// Release the lock after a protected operation completes.
pub fn exit(env: &Env) {
    env.storage().temporary().remove(&REENTRANCY_LOCK);
}

/// Run `op` under the reentrancy lock, releasing it afterwards.
///
/// The lock is released on both `Ok` and `Err` results. A panic aborts the
/// whole invocation, which rolls back the lock with every other write.
///
/// # Errors
/// - [`Error::ReentrantCall`] if called while already inside a protected
///   operation.
/// - Any error returned by `op`.
pub fn with_guard<T, F>(env: &Env, op: F) -> Result<T, Error>
where
    F: FnOnce() -> Result<T, Error>,
{
    enter(env)?;
    let result = op();
    exit(env);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(crate::ClipCashNFT, ());
        (env, contract_id)
    }

    #[test]
    fn unlocked_by_default() {
        let (env, id) = setup();
        env.as_contract(&id, || assert!(!is_locked(&env)));
    }

    #[test]
    fn enter_sets_lock_and_exit_clears_it() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            assert_eq!(enter(&env), Ok(()));
            assert!(is_locked(&env));
            exit(&env);
            assert!(!is_locked(&env));
        });
    }

    #[test]
    fn nested_enter_is_rejected() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            enter(&env).unwrap();
            assert_eq!(enter(&env), Err(Error::ReentrantCall));
        });
    }

    #[test]
    fn with_guard_runs_operation_and_releases_lock() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let result = with_guard(&env, || {
                assert!(is_locked(&env));
                Ok(7u32)
            });
            assert_eq!(result, Ok(7));
            assert!(!is_locked(&env));
        });
    }

    #[test]
    fn with_guard_blocks_nested_protected_call() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let result = with_guard(&env, || with_guard(&env, || Ok(())));
            assert_eq!(result, Err(Error::ReentrantCall));
            assert!(!is_locked(&env));
        });
    }

    #[test]
    fn with_guard_releases_lock_on_error() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let result: Result<(), Error> = with_guard(&env, || Err(Error::InvalidFee));
            assert_eq!(result, Err(Error::InvalidFee));
            assert!(!is_locked(&env));
            // A fresh protected call succeeds after the failed one.
            assert_eq!(with_guard(&env, || Ok(())), Ok(()));
        });
    }
}
