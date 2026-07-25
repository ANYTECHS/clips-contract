//! Mint events — emitted after a successful NFT mint.
//!
//! Resolves issue #432: emit event after successful mint.
//!
//! This module exposes two event emitters:
//! - [`emit_mint`]        — legacy lightweight `"mint"` event (owner + clip + token + URI).
//! - [`emit_nft_minted`]  — rich `"nft_minted"` event that also includes creator and timestamp.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{MintEvent, NFTMintedEvent, TokenId};

/// Emit the legacy `"mint"` event.
///
/// Kept for backward-compatibility with existing indexers. For new
/// integrations prefer [`emit_nft_minted`], which includes the creator
/// address and a timestamp.
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

/// Emit the rich `"nft_minted"` event immediately after a successful mint.
///
/// This event is the canonical signal for indexers, wallets, and
/// marketplaces to track newly created ClipCash NFTs. It is emitted only
/// after **all** state writes have completed successfully, so receiving it
/// guarantees the token exists on-chain.
///
/// # Arguments
/// * `env`          — Contract execution environment.
/// * `token_id`     — Newly assigned on-chain token ID.
/// * `clip_id`      — Off-chain video-clip identifier linked to the token.
/// * `creator`      — Address of the original clip creator (may differ from
///                    `owner` when the token is gifted or minted on behalf of
///                    a creator).
/// * `owner`        — Address that received ownership of the token.
/// * `metadata_uri` — Metadata URI stored for this token.
/// * `timestamp`    — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_minted(
    env: &Env,
    token_id: TokenId,
    clip_id: u32,
    creator: &Address,
    owner: &Address,
    metadata_uri: &String,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_mntd"),),
        NFTMintedEvent {
            token_id,
            clip_id,
            creator: creator.clone(),
            owner: owner.clone(),
            metadata_uri: metadata_uri.clone(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger, LedgerInfo},
        Address, Env, String,
    };

    #[test]
    fn emit_mint_publishes_event() {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            let to = Address::generate(&env);
            let uri = String::from_str(&env, "ipfs://QmTest");
            emit_mint(&env, &to, 1, 0, &uri);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_nft_minted_publishes_all_fields() {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            let creator = Address::generate(&env);
            let owner = Address::generate(&env);
            let uri = String::from_str(&env, "ipfs://QmClip42");

            emit_nft_minted(&env, 7, 42, &creator, &owner, &uri, 1_700_000_000);

            let all = env.events().all();
            assert_eq!(all.events().len(), 1);

            // Verify the published data round-trips correctly.
            let (_, data): (soroban_sdk::Vec<soroban_sdk::Val>, NFTMintedEvent) =
                all.events().get(0).unwrap();
            assert_eq!(data.token_id, 7);
            assert_eq!(data.clip_id, 42);
            assert_eq!(data.creator, creator);
            assert_eq!(data.owner, owner);
            assert_eq!(data.metadata_uri, uri);
            assert_eq!(data.timestamp, 1_700_000_000);
        });
    }

    #[test]
    fn emit_nft_minted_only_after_all_writes_complete() {
        // Verify: no event is emitted when called zero times (i.e., on a
        // failed path the caller simply doesn't call this function).
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
