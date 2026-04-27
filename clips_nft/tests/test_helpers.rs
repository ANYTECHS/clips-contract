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
    token, Address, Bytes, BytesN, Env, String, Vec, xdr::ToXdr,
};

/// Fully initialized test context returned by [`setup`].
pub struct TestContext<'a> {
    pub env: Env,
    pub client: ClipsNftContractClient<'a>,
    pub admin: Address,
    pub keypair: ed25519_dalek::SigningKey,
}

/// Register the contract, init it, and register a fresh backend signer.
/// Calls `env.mock_all_auths()` so every auth check passes automatically.
pub fn setup() -> TestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());

    // SAFETY: the Env is heap-allocated and lives as long as TestContext.
    let env: &'static Env = Box::leak(Box::new(env));

    let client = ClipsNftContractClient::new(env, &contract_id);
    client.init(&admin);

    let keypair = {
        let sk_bytes = soroban_sdk::BytesN::<32>::random(env).to_array();
        ed25519_dalek::SigningKey::from_bytes(&sk_bytes)
    };
    let pubkey = BytesN::from_array(env, &keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey);

    TestContext { env, client, admin, keypair }
}

// ---------------------------------------------------------------------------
// Signature helper (mirrors on-chain verify_clip_signature exactly)
// ---------------------------------------------------------------------------

pub fn sign_mint(
    env: &Env,
    keypair: &ed25519_dalek::SigningKey,
    owner: &Address,
    clip_id: u32,
    metadata_uri: &String,
) -> BytesN<64> {
    let owner_hash: BytesN<32> = env.crypto().sha256(&owner.clone().to_xdr(env)).into();
    let uri_hash: BytesN<32> =
        env.crypto().sha256(&Bytes::from(metadata_uri.to_xdr(env))).into();

    let mut preimage = Bytes::new(env);
    preimage.extend_from_array(&clip_id.to_le_bytes());
    preimage.append(&Bytes::from(owner_hash));
    preimage.append(&Bytes::from(uri_hash));

    let message: BytesN<32> = env.crypto().sha256(&preimage).into();
    use ed25519_dalek::Signer as _;
    BytesN::from_array(env, &keypair.sign(&message.to_array()).to_bytes())
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
pub fn mint_clip(ctx: &TestContext, owner: &Address, clip_id: u32, is_soulbound: bool) -> TokenId {
    let uri = String::from_str(ctx.env, &format!("ipfs://QmClip{}", clip_id));
    let sig = sign_mint(ctx.env, &ctx.keypair, owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    ctx.client.mint(owner, &clip_id, &uri, &royalty, &is_soulbound, &sig)
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
    let royalty = Royalty { recipients, asset_address: None };
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);
}

// ---------------------------------------------------------------------------
// Helper 3 — simulate_sale
// ---------------------------------------------------------------------------

/// Simulate a secondary sale of `token_id` from `seller` to `buyer` at
/// `sale_price` using a SEP-0041 token at `asset_address`.
///
/// Steps performed:
/// 1. Calls `pay_royalty` so recipients receive their cut.
/// 2. Calls `transfer` to move ownership to `buyer`.
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
    let royalty = ctx.client.get_royalty(&token_id);
    let mut updated = royalty.clone();
    updated.asset_address = Some(asset_address.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &updated);

    let info = ctx.client.royalty_info(&token_id, &sale_price);
    ctx.client.pay_royalty(seller, &token_id, &sale_price);
    ctx.client.transfer(seller, buyer, &token_id);

    info.royalty_amount
}

// ---------------------------------------------------------------------------
// Internal utility
// ---------------------------------------------------------------------------

fn default_royalty(env: &Env, recipient: Address) -> Royalty {
    let mut recipients = Vec::new(env);
    recipients.push_back(RoyaltyRecipient { recipient, basis_points: 500 });
    Royalty { recipients, asset_address: None }
}

/// Deploy a fresh SEP-0041 token, mint `amount` to `holder`, and return the
/// token address. Useful for `simulate_sale` tests.
pub fn deploy_token(env: &Env, holder: &Address, amount: i128) -> Address {
    let token_admin = Address::generate(env);
    let addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    token::StellarAssetClient::new(env, &addr).mint(holder, &amount);
    addr
}
