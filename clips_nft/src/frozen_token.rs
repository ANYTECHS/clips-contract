//! Frozen token storage — tracks which NFTs are frozen.
//!
//! # Storage
//! Key: `DataKey::FrozenToken(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, TokenId};

/// Mark a token as frozen and report whether the frozen marker was newly set.
pub fn freeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if env.storage().persistent().has(&key) {
        return false;
    }
    env.storage().persistent().set(&key, &true);
    true
}

/// Unfreeze a token and report whether a frozen marker was removed.
pub fn unfreeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if !env.storage().persistent().has(&key) {
        return false;
    }
    env.storage().persistent().remove(&key);
    true
}

/// Return `true` if the token is currently frozen.
pub fn is_frozen(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::FrozenToken(token_id))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn setup() -> (Env, TokenId) {
        let env = Env::default();
        let token_id = TokenId::from(1u32);
        (env, token_id)
    }

    #[test]
    fn test_token_not_frozen_by_default() {
        let (env, token_id) = setup();
        assert!(!is_frozen(&env, token_id));
    }

    #[test]
    fn test_freeze_token_sets_state() {
        let (env, token_id) = setup();
        assert!(freeze_token(&env, token_id));
        assert!(is_frozen(&env, token_id));
    }

    #[test]
    fn test_freeze_token_is_idempotent() {
        let (env, token_id) = setup();
        assert!(freeze_token(&env, token_id));
        // Second freeze should report no new marker was set.
        assert!(!freeze_token(&env, token_id));
        assert!(is_frozen(&env, token_id));
    }

    #[test]
    fn test_unfreeze_token_clears_state() {
        let (env, token_id) = setup();
        freeze_token(&env, token_id);
        assert!(unfreeze_token(&env, token_id));
        assert!(!is_frozen(&env, token_id));
    }

    #[test]
    fn test_unfreeze_token_when_not_frozen() {
        let (env, token_id) = setup();
        // Unfreezing a token that was never frozen reports no marker removed.
        assert!(!unfreeze_token(&env, token_id));
        assert!(!is_frozen(&env, token_id));
    }

    #[test]
    fn test_freeze_unfreeze_cycle() {
        let (env, token_id) = setup();
        assert!(freeze_token(&env, token_id));
        assert!(is_frozen(&env, token_id));
        assert!(unfreeze_token(&env, token_id));
        assert!(!is_frozen(&env, token_id));
        // Can be re-frozen after unfreezing.
        assert!(freeze_token(&env, token_id));
        assert!(is_frozen(&env, token_id));
    }

    #[test]
    fn test_freeze_state_is_per_token() {
        let env = Env::default();
        let token_a = TokenId::from(1u32);
        let token_b = TokenId::from(2u32);

        assert!(freeze_token(&env, token_a));
        assert!(is_frozen(&env, token_a));
        assert!(!is_frozen(&env, token_b));

        assert!(freeze_token(&env, token_b));
        assert!(is_frozen(&env, token_a));
        assert!(is_frozen(&env, token_b));

        assert!(unfreeze_token(&env, token_a));
        assert!(!is_frozen(&env, token_a));
        assert!(is_frozen(&env, token_b));
    }
}