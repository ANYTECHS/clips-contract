//! Creator assignment event — emitted when a creator is bound to a minted NFT.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{CreatorAssignedEvent, TokenId};

/// Emit the `"creator"` assignment event after a successful creator write.
pub fn emit_creator_assigned(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    clip_id: u32,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("creator"),),
        CreatorAssignedEvent {
            token_id,
            creator: creator.clone(),
            clip_id,
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    #[test]
    fn emit_creator_assigned_publishes_event() {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            let creator = Address::generate(&env);
            emit_creator_assigned(&env, 7, &creator, 42, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
