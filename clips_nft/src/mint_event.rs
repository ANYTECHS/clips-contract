//! Mint event — emits the standard `"mint"` event after a successful NFT mint.
//!
//! Resolves issue #432: emit event after successful mint.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{MintEvent, TokenId};

/// Emit the `"mint"` event.
///
/// Call this once per successful mint, after all state writes are complete.
///
/// # Arguments
/// * `env`          — Contract execution environment.
/// * `to`           — Address that received the NFT.
/// * `clip_id`      — Off-chain clip identifier.
/// * `token_id`     — Newly assigned on-chain token ID.
/// * `metadata_uri` — Metadata URI stored for this token.
pub fn emit_mint(env: &Env, to: &Address, clip_id: u32, token_id: TokenId, metadata_uri: &String) {
    env.events().publish(
        (symbol_short!("mint"),),
        MintEvent {
            to: to.clone(),
            clip_id,
            token_id,
            metadata_uri: metadata_uri.clone(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, String,
    };

    #[test]
    fn emit_mint_publishes_event() {
        let env = Env::default();
        let to = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");

        emit_mint(&env, &to, 1, 0, &uri);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
    }
}
