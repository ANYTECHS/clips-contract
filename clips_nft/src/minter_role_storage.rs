//! Approved minter role storage.
//!
//! Maintains the set of wallet addresses that are authorised to mint NFTs on
//! behalf of creators.  Each address is stored independently so approvals can
//! be granted or revoked per-account without affecting others.
//!
//! # Storage
//! Key: `DataKey::ApprovedMinter(minter)` (persistent storage)
//! Value: `bool` — `true` means the address is currently approved.

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Approve `minter` to mint NFTs on behalf of creators.
///
/// Calling this function when `minter` is already approved is a no-op
/// (idempotent).
pub fn add_approved_minter(env: &Env, minter: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::ApprovedMinter(minter.clone()), &true);
}

/// Revoke the minting approval for `minter`.
///
/// If `minter` was never approved this function is a no-op — it does **not**
/// panic or return an error.
pub fn remove_approved_minter(env: &Env, minter: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::ApprovedMinter(minter.clone()));
}

/// Return `true` if `minter` is currently approved to mint NFTs.
///
/// Returns `false` for any address that has not been approved, or whose
/// approval has been revoked.
pub fn is_approved_minter(env: &Env, minter: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ApprovedMinter(minter.clone()))
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn test_env() -> Env {
        Env::default()
    }

    // ── test cases ───────────────────────────────────────────────────────────

    /// Adding a minter and then querying it should return `true`.
    #[test]
    fn add_then_query_returns_true() {
        let env = test_env();
        let minter = Address::generate(&env);

        add_approved_minter(&env, &minter);

        assert!(is_approved_minter(&env, &minter));
    }

    /// Removing a minter after adding it should return `false`.
    #[test]
    fn remove_then_query_returns_false() {
        let env = test_env();
        let minter = Address::generate(&env);

        add_approved_minter(&env, &minter);
        remove_approved_minter(&env, &minter);

        assert!(!is_approved_minter(&env, &minter));
    }

    /// Querying before any approval has been granted should return `false`.
    #[test]
    fn query_before_add_returns_false() {
        let env = test_env();
        let minter = Address::generate(&env);

        assert!(!is_approved_minter(&env, &minter));
    }

    /// Adding the same minter twice is idempotent — the second call must not
    /// panic and the status must still be `true`.
    #[test]
    fn add_same_minter_twice_is_idempotent() {
        let env = test_env();
        let minter = Address::generate(&env);

        add_approved_minter(&env, &minter);
        add_approved_minter(&env, &minter); // second call — must not panic

        assert!(is_approved_minter(&env, &minter));
    }

    /// Removing a minter that was never added must not panic.
    #[test]
    fn remove_non_existent_minter_is_noop() {
        let env = test_env();
        let minter = Address::generate(&env);

        // Must not panic:
        remove_approved_minter(&env, &minter);

        assert!(!is_approved_minter(&env, &minter));
    }

    /// Multiple distinct minters are stored independently; approving one must
    /// not affect the others.
    #[test]
    fn multiple_distinct_minters_stored_independently() {
        let env = test_env();
        let minter_a = Address::generate(&env);
        let minter_b = Address::generate(&env);
        let minter_c = Address::generate(&env);

        // Approve only A and B.
        add_approved_minter(&env, &minter_a);
        add_approved_minter(&env, &minter_b);

        assert!(is_approved_minter(&env, &minter_a), "A should be approved");
        assert!(is_approved_minter(&env, &minter_b), "B should be approved");
        assert!(!is_approved_minter(&env, &minter_c), "C was never approved");

        // Revoke B — must not affect A or C.
        remove_approved_minter(&env, &minter_b);

        assert!(is_approved_minter(&env, &minter_a), "A still approved");
        assert!(!is_approved_minter(&env, &minter_b), "B now revoked");
        assert!(
            !is_approved_minter(&env, &minter_c),
            "C still never approved"
        );
    }

    /// Approve → revoke → approve again should restore the approved status.
    #[test]
    fn re_approve_after_revoke_works() {
        let env = test_env();
        let minter = Address::generate(&env);

        add_approved_minter(&env, &minter);
        remove_approved_minter(&env, &minter);
        add_approved_minter(&env, &minter);

        assert!(is_approved_minter(&env, &minter));
    }
}
