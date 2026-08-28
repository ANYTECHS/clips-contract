//! Royalty admin guard — reusable authorization check for all royalty
//! administrative operations.
//!
//! Only addresses registered as administrators (via [`administrator_storage`])
//! may invoke state-changing royalty functions. This guard centralises the
//! auth check so individual royalty entry-points stay thin.
//!
//! # Usage
//!
//! ```rust,ignore
//! royalty_admin_guard::require_royalty_admin(env, &caller)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::types::Error;

/// Reject the call if `caller` is not a registered administrator.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if the caller is not an admin.
pub fn require_royalty_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    caller.require_auth();

    if !administrator_storage::is_admin(env, caller) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn admin_passes_guard() {
        let env = Env::default();
        let admin = Address::generate(&env);
        administrator_storage::add_admin(&env, &admin);

        assert!(require_royalty_admin(&env, &admin).is_ok());
    }

    #[test]
    fn non_admin_is_rejected() {
        let env = Env::default();
        let user = Address::generate(&env);

        assert_eq!(require_royalty_admin(&env, &user), Err(Error::Unauthorized));
    }

    #[test]
    fn revoked_admin_is_rejected() {
        let env = Env::default();
        let admin = Address::generate(&env);
        administrator_storage::add_admin(&env, &admin);
        administrator_storage::remove_admin(&env, &admin);

        assert_eq!(
            require_royalty_admin(&env, &admin),
            Err(Error::Unauthorized)
        );
    }
}
