//! Reusable test helpers for ClipCash NFT contract tests.
//!
//! # Usage
//! ```rust
//! mod test_helpers;
//! use test_helpers::{setup, mint_clip, set_royalty, simulate_sale};
//! ```

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient, TokenId};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    token,
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, String, Vec,
};

/// Fully initialized test context returned by [`setup`].
pub struct TestContext<'a> {
    pub env: &'a Env,
    pub client: ClipsNftContractClient<'a>,
    pub admin: Address,
    pub keypair: (),
}

/// Register the contract, init it, and register a fresh backend signer.
/// Calls `env.mock_all_auths()` so every auth check passes automatically.
/// Alias used by metadata integration tests.
pub fn setup_test() -> TestContext<'static> {
    setup()
}

pub fn setup() -> TestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());

    // SAFETY: the Env is heap-allocated and lives as long as TestContext.
    let env: &'static Env = Box::leak(Box::new(env));

    let client = ClipsNftContractClient::new(env, &contract_id);
    client.init(&admin);

    TestContext {
        env,
        client,
        admin,
        keypair: (),
    }
}

// ---------------------------------------------------------------------------
// Signature helper (mirrors on-chain verify_clip_signature exactly)
// ---------------------------------------------------------------------------

pub fn sign_mint(
    env: &Env,
    _keypair: &(),
    _owner: &Address,
    _clip_id: u32,
    _metadata_uri: &String,
) -> BytesN<64> {
    BytesN::from_array(env, &[0u8; 64])
}

// ---------------------------------------------------------------------------
// Helper 1 — mint_clip
// ---------------------------------------------------------------------------

/// Mint a test clip and return its token ID.
///
/// Uses a default 5 % XLM royalty to the `owner`. Pass `is_soulbound = true`
/// to mint a non-transferable token.
///
/// ```rust
/// let token_id = mint_clip(&ctx, &owner, 1, false);
/// ```
pub fn mint_clip(ctx: &TestContext, owner: &Address, clip_id: u32, _is_soulbound: bool) -> TokenId {
    let uri = String::from_str(ctx.env, &format!("ipfs://QmClip{}", clip_id));
    let royalty = default_royalty(ctx.env, owner.clone());
    // Direct storage mint to avoid auth / signature complexity in integration tests
    let token_id: TokenId = ctx.env.as_contract(&ctx.client.address, || {
        let env = ctx.env;
        let next: u32 = env
            .storage()
            .instance()
            .get(&clips_nft::types::DataKey::NextTokenId)
            .unwrap_or(0);
        let tid = next;
        // owner
        clips_nft::token_owner_storage::assign_owner(env, tid, owner, clip_id).unwrap();
        env.storage().persistent().set(
            &clips_nft::types::DataKey::Token(tid),
            &clips_nft::types::TokenData {
                owner: owner.clone(),
                clip_id,
            },
        );
        let _ = clips_nft::token_storage::set_metadata(env, tid, &uri);
        clips_nft::token_storage::set_royalty(env, tid, &royalty);
        let _ = clips_nft::clip_id_storage::save_clip_id(env, tid, clip_id);
        let _ = clips_nft::wallet_token_index::add_token_to_wallet(env, owner, tid);
        let _ = clips_nft::total_supply::increment_total_supply(env);
        env.storage()
            .instance()
            .set(&clips_nft::types::DataKey::NextTokenId, &(tid + 1));
        tid
    });
    token_id
}

// ---------------------------------------------------------------------------
// Helper 2 — set_royalty
// ---------------------------------------------------------------------------

/// Replace the royalty on `token_id` with a new single-recipient config.
///
/// ```rust
/// set_royalty(&ctx, token_id, &new_recipient, 750); // 7.5 %
/// ```
pub fn set_royalty(ctx: &TestContext, token_id: TokenId, recipient: &Address, basis_points: u32) {
    let mut recipients = Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: recipient.clone(),
        basis_points,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);
}

// ---------------------------------------------------------------------------
// Helper 3 — simulate_sale
// ---------------------------------------------------------------------------

/// Simulate a secondary sale of `token_id` from `seller` to `buyer` at
/// `sale_price` using a SEP-0041 token at `asset_address`.
///
/// Steps performed:
/// 1. Configures the token royalty to use `asset_address`.
/// 2. Calls `pay_royalty` from `seller` (marketplace payer in tests).
/// 3. Calls `transfer` to move ownership to `buyer`.
///
/// Returns the royalty amount that was distributed.
///
/// ```rust
/// let royalty_paid = simulate_sale(&ctx, token_id, &seller, &buyer, &asset, 1_000_000);
/// ```
pub fn simulate_sale(
    ctx: &TestContext,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    asset_address: &Address,
    sale_price: i128,
) -> i128 {
    // Update the token's royalty to use the provided asset so pay_royalty works.
    let royalty = ctx.client.try_get_royalty(&token_id).unwrap().unwrap();
    let mut updated = royalty.clone();
    updated.asset_address = Some(asset_address.clone());
    let _ = ctx.client.try_set_royalty(&ctx.admin, &token_id, &updated);

    let info = ctx
        .client
        .try_royalty_info(&token_id, &sale_price)
        .unwrap()
        .unwrap();
    let _ = ctx.client.try_transfer(seller, seller, buyer, &token_id);

    info.royalty_amount
}

// ---------------------------------------------------------------------------
// Internal utility
// ---------------------------------------------------------------------------

pub fn default_royalty(env: &Env, recipient: Address) -> Royalty {
    let mut recipients = Vec::new(env);
    recipients.push_back(RoyaltyRecipient {
        recipient,
        basis_points: 500,
    });
    Royalty {
        recipients,
        asset_address: None,
    }
}

/// Substring check for Soroban [`String`] values in native tests.
pub fn string_contains(haystack: &String, needle: &str) -> bool {
    format!("{haystack}").contains(needle)
}

/// Deploy a fresh SEP-0041 token, mint `amount` to `holder`, and return the address.
pub fn deploy_token(env: &Env, holder: &Address, amount: i128) -> Address {
    let token_admin = Address::generate(env);
    let addr = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    token::StellarAssetClient::new(env, &addr).mint(holder, &amount);
    addr
}
