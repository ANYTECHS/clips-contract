//! Mint events — emitted after a successful NFT mint.
//!
//! Resolves issue #914: emit an event whenever a new ClipCash NFT is
//! successfully minted, including all required fields:
//! token ID, creator, owner, clip ID, metadata reference, and timestamp.
//!
//! This module exposes two event emitters:
//! - [`emit_mint`]        — legacy lightweight `"mint"` event (owner + clip + token + URI).
//! - [`emit_nft_minted`]  — rich `"nft_mntd"` event with all 6 acceptance-criteria fields.

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

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_minted_event(
    env: &Env,
    token_id: TokenId,
    clip_id: u32,
    creator: &Address,
    owner: &Address,
    metadata_uri: &String,
    timestamp: u64,
) -> NFTMintedEvent {
    let _ = env; // env kept for API symmetry with emit_nft_minted
    NFTMintedEvent {
        token_id,
        clip_id,
        creator: creator.clone(),
        owner: owner.clone(),
        metadata_uri: metadata_uri.clone(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, String,
    };

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── emit_mint (legacy) ────────────────────────────────────────────────────

    #[test]
    fn emit_mint_publishes_event() {
        with_contract(|env| {
            let to = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            emit_mint(env, &to, 1, 0, &uri);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    // ── emit_nft_minted ───────────────────────────────────────────────────────

    #[test]
    fn emit_nft_minted_publishes_exactly_one_event() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmClip42");
            emit_nft_minted(env, 7, 42, &creator, &owner, &uri, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn no_event_emitted_when_not_called() {
        with_contract(|env| {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }

    #[test]
    fn multiple_mints_emit_separate_events() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            emit_nft_minted(env, 1, 10, &creator, &owner, &uri, 100);
            emit_nft_minted(env, 2, 20, &creator, &owner, &uri, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    // ── payload field coverage (acceptance criteria) ──────────────────────────

    #[test]
    fn payload_contains_token_id() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let payload = build_nft_minted_event(env, 99, 1, &creator, &owner, &uri, 0);
            assert_eq!(payload.token_id, 99);
        });
    }

    #[test]
    fn payload_contains_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let payload = build_nft_minted_event(env, 1, 1, &creator, &owner, &uri, 0);
            assert_eq!(payload.creator, creator);
        });
    }

    #[test]
    fn payload_contains_owner() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let payload = build_nft_minted_event(env, 1, 1, &creator, &owner, &uri, 0);
            assert_eq!(payload.owner, owner);
        });
    }

    #[test]
    fn payload_contains_clip_id() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let payload = build_nft_minted_event(env, 1, 777, &creator, &owner, &uri, 0);
            assert_eq!(payload.clip_id, 777);
        });
    }

    #[test]
    fn payload_contains_metadata_uri() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmClipMetadata");
            let payload = build_nft_minted_event(env, 1, 1, &creator, &owner, &uri, 0);
            assert_eq!(payload.metadata_uri, uri);
        });
    }

    #[test]
    fn payload_contains_timestamp() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let ts: u64 = 1_720_000_000;
            let payload = build_nft_minted_event(env, 1, 1, &creator, &owner, &uri, ts);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn creator_can_differ_from_owner() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            let payload = build_nft_minted_event(env, 1, 1, &creator, &owner, &uri, 0);
            assert_ne!(payload.creator, payload.owner);
            assert_eq!(payload.creator, creator);
            assert_eq!(payload.owner, owner);
        });
    }

    #[test]
    fn all_six_fields_set_in_single_call() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmAll6Fields");
            let ts: u64 = 1_234_567_890;
            let payload = build_nft_minted_event(env, 42, 99, &creator, &owner, &uri, ts);
            assert_eq!(payload.token_id, 42);
            assert_eq!(payload.clip_id, 99);
            assert_eq!(payload.creator, creator);
            assert_eq!(payload.owner, owner);
            assert_eq!(payload.metadata_uri, uri);
            assert_eq!(payload.timestamp, ts);
        });
    }
}
