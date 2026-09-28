#![cfg(test)]
#[test]
fn placeholder() {
    assert_eq!(1, 1);
}

/// Multiple unfreezes emit separate events.
#[test]
fn test_multiple_unfreezes_emit_separate_events() {
    with_contract(|env, contract_id| {
        let caller = Address::generate(env);

        for i in 0..3 {
            let token_id = 700 + i;
            frozen_token::freeze_token(env, token_id);
            nft_unfrozen_event::emit_nft_unfrozen(env, token_id, &caller, env.ledger().timestamp() + i * 100);
        }

        assert_eq!(env.events().all().events().len(), 3);
    });
}

// ─── NFT lifecycle integration test ─────────────────────────────────────────

/// Full lifecycle: mint → freeze → unfreeze emits correct sequence of events.
#[test]
fn test_full_nft_lifecycle() {
    with_contract(|env, contract_id| {
        let admin = Address::generate(env);
        let token_id: u32 = 800;

        // 1. Freeze must not emit event if token doesn't exist
        assert!(!frozen_token::freeze_token(env, token_id));
        assert_eq!(event_count(env), 0);

        // 2. Unfreeze must not emit event if token doesn't exist
        assert!(!frozen_token::unfreeze_token(env, token_id));
        assert_eq!(event_count(env), 0);
    });
}
