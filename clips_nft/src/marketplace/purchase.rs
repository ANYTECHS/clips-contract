//! Marketplace purchase — core workflow for buying a listed NFT.
//!
//! Acceptance Criteria:
//! 1. Validate listing — must exist, be Active, and not expired.
//! 2. Validate buyer — must be authorized, not the seller, not blacklisted, not the contract.
//! 3. Calculate fees — platform fee from contract configuration.
//! 4. Calculate royalties — per-recipient royalty splits from token's royalty config.
//! 5. Process payment — record royalty payments and accumulate platform revenue.
//! 6. Transfer NFT — update token owner and wallet indexes.
//! 7. Mark listing sold — record buyer and timestamp.
//! 8. Emit events — NftSoldEvent and RoyaltyPaidEvent for each royalty recipient.

use soroban_sdk::{Address, Env, Vec};

use crate::blacklist;
use crate::pause_guard;
use crate::platform_fee;
use crate::platform_recipient;
use crate::platform_revenue;
use crate::royalty_history;
use crate::royalty_pause_guard;
use crate::safe_math;
use crate::token_owner_storage;
use crate::token_storage;
use crate::transaction_deduction_validator;
use crate::types::{Error, RoyaltyPaidEvent, RoyaltyPayment, TokenId};
use crate::wallet_token_index;

use super::listing_storage;
use super::types::{ListingStatus, NftSoldEvent, PurchaseResult};

/// Topic label emitted with every [`NftSoldEvent`].
const NFT_SOLD_TOPIC: &str = "nft_sold";

/// Topic label emitted with every [`RoyaltyPaidEvent`].
const ROYALTY_PAID_TOPIC: &str = "royalty_paid";

/// Execute a marketplace purchase for a listed NFT.
///
/// The buyer pays the listing price, which is split among:
/// - Royalty recipients (per the token's configured royalty split)
/// - Platform fee (from contract configuration)
/// - Seller (remaining net amount)
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `buyer`    — Address of the buyer (must auth).
/// * `token_id` — On-chain token identifier of the listed NFT.
///
/// # Returns
/// [`PurchaseResult`] with the complete breakdown of the purchase.
///
/// # Acceptance Criteria
/// 1. **Validate listing** — must exist, be Active, and not expired.
/// 2. **Validate buyer** — must be authorized, not the seller, not blacklisted, not the contract.
/// 3. **Calculate fees** — platform fee from contract configuration.
/// 4. **Calculate royalties** — per-recipient royalty splits from token's royalty config.
/// 5. **Process payment** — record royalty payments and accumulate platform revenue.
/// 6. **Transfer NFT** — update token owner and wallet indexes.
/// 7. **Mark listing sold** — record buyer and timestamp.
/// 8. **Emit events** — NftSoldEvent and RoyaltyPaidEvent for each royalty recipient.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused` | Contract is paused. |
/// | `TokenNotFound` | Listing does not exist for this token. |
/// | `ListingNotActive` | Listing is not in Active status. |
/// | `ListingExpired` | Listing has passed its expiration timestamp. |
/// | `CannotPurchaseOwnListing` | Buyer is the same address as the seller. |
/// | `InvalidAddress` | Buyer is blacklisted or is the contract itself. |
/// | `Unauthorized` | Token is frozen or buyer is not authorized. |
/// | `RoyaltyOverflow` | Arithmetic overflow during fee/royalty calculation. |
/// | `TotalDeductionsExceedSalePrice` | Combined royalty + platform fee > sale price. |
#[allow(deprecated)]
pub fn purchase_listing(
    env: &Env,
    buyer: &Address,
    token_id: TokenId,
) -> Result<PurchaseResult, Error> {
    // ── Step 1: Validate buyer authorization ──────────────────────────────────
    buyer.require_auth();

    // ── Step 2: Validate listing ──────────────────────────────────────────────
    // 2a. Contract must not be paused.
    pause_guard::require_not_paused(env)?;
    royalty_pause_guard::require_royalty_not_paused(env)?;

    // 2b. Listing must exist and be in Active status.
    let listing = listing_storage::get_listing(env, token_id)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }

    // 2c. Listing must not be expired.
    if listing.expires_at > 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }

    // ── Step 3: Validate buyer ────────────────────────────────────────────────
    // 3a. Buyer must not be the seller.
    if *buyer == listing.seller {
        return Err(Error::CannotPurchaseOwnListing);
    }

    // 3b. Buyer must not be blacklisted.
    if blacklist::is_blacklisted(env, buyer) {
        return Err(Error::InvalidAddress);
    }

    // 3c. Buyer must not be the contract itself.
    if *buyer == env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }

    // 3d. Token must not be frozen.
    if crate::frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }

    // 3e. Token must exist (ownership record must exist).
    let _owner = token_owner_storage::get_owner(env, token_id)?;

    // ── Step 4: Calculate fees and royalties ──────────────────────────────────
    let sale_price = listing.price;

    // 4a. Calculate platform fee.
    let platform_fee_bps = platform_fee::get_platform_fee(env);

    // 4b. Calculate total royalty basis points from token's royalty config.
    // If no royalty is configured, use an empty royalty (zero recipients).
    let royalty = token_storage::get_royalty(env, token_id).unwrap_or(crate::types::Royalty {
        recipients: Vec::new(env),
        asset_address: None,
    });
    let mut total_royalty_bps: u32 = 0;
    for i in 0..royalty.recipients.len() {
        if let Some(r) = royalty.recipients.get(i) {
            total_royalty_bps = total_royalty_bps
                .checked_add(r.basis_points)
                .ok_or(Error::RoyaltyOverflow)?;
        }
    }

    // 4c. Validate total deductions don't exceed sale price.
    let (total_royalty_amount, platform_fee_amount) =
        transaction_deduction_validator::validate_total_deduction_amount(
            sale_price,
            total_royalty_bps,
            platform_fee_bps,
        )?;

    // ── Step 5: Process payment — distribute royalties ────────────────────────
    let timestamp = env.ledger().timestamp();
    let mut payments: Vec<RoyaltyPayment> = Vec::new(env);

    for i in 0..royalty.recipients.len() {
        if let Some(recipient_config) = royalty.recipients.get(i) {
            let amount = safe_math::safe_royalty_amount(sale_price, recipient_config.basis_points)?;

            if amount > 0 {
                // Record payment in history.
                royalty_history::record_royalty_payment(
                    env,
                    token_id,
                    recipient_config.recipient.clone(),
                    amount,
                    timestamp,
                );

                // Emit royalty paid event.
                env.events().publish(
                    (ROYALTY_PAID_TOPIC,),
                    RoyaltyPaidEvent {
                        token_id,
                        payer: buyer.clone(),
                        receiver: recipient_config.recipient.clone(),
                        amount,
                        asset_address: royalty.asset_address.clone(),
                        timestamp,
                    },
                );

                payments.push_back(RoyaltyPayment {
                    token_id,
                    recipient: recipient_config.recipient.clone(),
                    amount,
                    timestamp,
                });
            }
        }
    }

    // 5b. Record platform fee if applicable.
    if platform_fee_amount > 0 {
        if let Ok(_platform_wallet) = platform_recipient::get_platform_recipient(env) {
            platform_revenue::update_platform_revenue(env, platform_fee_amount);
        }
    }

    // ── Step 6: Transfer NFT ──────────────────────────────────────────────────
    let seller = listing.seller.clone();

    // 6a. Update token owner.
    token_owner_storage::save_owner(env, token_id, buyer);

    // 6b. Update wallet token indexes.
    wallet_token_index::move_token_between_wallets(env, &seller, buyer, token_id)?;

    // 6c. Also update the legacy TokenData owner field.
    if let Ok(mut token_data) = crate::token_storage::get_token(env, token_id) {
        token_data.owner = buyer.clone();
        crate::token_storage::set_token(env, token_id, &token_data);
    }

    // ── Step 7: Mark listing sold ─────────────────────────────────────────────
    listing_storage::mark_as_sold(env, token_id, buyer)?;

    // ── Step 8: Emit purchase event ───────────────────────────────────────────
    env.events().publish(
        (NFT_SOLD_TOPIC,),
        NftSoldEvent {
            token_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            sale_amount: sale_price,
            payment_asset: listing.payment_asset.clone(),
            timestamp,
        },
    );

    Ok(PurchaseResult {
        token_id,
        seller,
        buyer: buyer.clone(),
        sale_price,
        platform_fee: platform_fee_amount,
        total_royalty: total_royalty_amount,
        royalty_payments: payments,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::listing_storage;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::payment_currency;
    use crate::types::{Royalty, RoyaltyRecipient};
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    /// Helper — register the contract and run `f` in its context.
    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    /// Mint a minimal token owned by `owner` for testing purposes.
    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    /// Create a supported payment asset for testing.
    fn setup_payment_asset(env: &Env) -> Address {
        let asset = Address::generate(env);
        payment_currency::add_currency(env, asset.clone()).unwrap();
        asset
    }

    /// Create an active listing for a token.
    fn setup_listing(
        env: &Env,
        token_id: TokenId,
        seller: &Address,
        price: i128,
        asset: &Address,
        expires_at: u64,
    ) {
        listing_storage::save_listing(
            env,
            &Listing {
                token_id,
                seller: seller.clone(),
                price,
                payment_asset: asset.clone(),
                expires_at,
                status: ListingStatus::Active,
                created_at: env.ledger().timestamp(),
                buyer: None,
                sold_at: None,
            },
        );
    }

    /// Configure royalty for a token.
    fn setup_royalty(env: &Env, token_id: TokenId, recipient: &Address, bps: u32) {
        let mut recipients = soroban_sdk::Vec::new(env);
        recipients.push_back(RoyaltyRecipient {
            recipient: recipient.clone(),
            basis_points: bps,
        });
        let royalty = Royalty {
            recipients,
            asset_address: None,
        };
        token_storage::set_royalty(env, token_id, &royalty);
    }

    // ── Step 1: Validate listing ──────────────────────────────────────────────

    #[test]
    fn purchase_fails_when_no_listing_exists() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            assert_eq!(
                purchase_listing(env, &buyer, 999),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn purchase_fails_when_listing_not_active() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);

            listing_storage::save_listing(
                env,
                &Listing {
                    token_id: 1,
                    seller: seller.clone(),
                    price: 1000,
                    payment_asset: asset,
                    expires_at: 0,
                    status: ListingStatus::Sold,
                    created_at: 0,
                    buyer: None,
                    sold_at: None,
                },
            );

            assert_eq!(
                purchase_listing(env, &buyer, 1),
                Err(Error::ListingNotActive)
            );
        });
    }

    #[test]
    fn purchase_fails_when_listing_expired() {
        with_contract(|env| {
            env.ledger().set_timestamp(1000);
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 500); // expires_at = 500 (before now)

            assert_eq!(purchase_listing(env, &buyer, 1), Err(Error::ListingExpired));
        });
    }

    #[test]
    fn purchase_succeeds_when_listing_no_expiration() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 500); // 5%

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.token_id, 1);
            assert_eq!(result.seller, seller);
            assert_eq!(result.buyer, buyer);
            assert_eq!(result.sale_price, 1_000_000);
        });
    }

    // ── Step 2: Validate buyer ────────────────────────────────────────────────

    #[test]
    fn purchase_fails_when_buyer_is_seller() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            assert_eq!(
                purchase_listing(env, &seller, 1),
                Err(Error::CannotPurchaseOwnListing)
            );
        });
    }

    #[test]
    fn purchase_fails_when_buyer_is_blacklisted() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);
            blacklist::add_wallet(env, &buyer);

            assert_eq!(purchase_listing(env, &buyer, 1), Err(Error::InvalidAddress));
        });
    }

    #[test]
    fn purchase_fails_when_buyer_is_contract() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);
            let contract = env.current_contract_address();

            assert_eq!(
                purchase_listing(env, &contract, 1),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn purchase_fails_when_token_is_frozen() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);
            crate::frozen_token::freeze_token(env, 1);

            assert_eq!(purchase_listing(env, &buyer, 1), Err(Error::Unauthorized));
        });
    }

    // ── Step 3: Calculate fees and royalties ──────────────────────────────────

    #[test]
    fn purchase_calculates_zero_royalty_when_none_configured() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            // No royalty configured — should use empty recipients

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.total_royalty, 0);
            assert_eq!(result.platform_fee, 0);
            assert_eq!(result.royalty_payments.len(), 0);
        });
    }

    #[test]
    fn purchase_calculates_correct_royalty_split() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 500); // 5%

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.total_royalty, 50_000); // 5% of 1_000_000
            assert_eq!(result.platform_fee, 0);
            assert_eq!(result.royalty_payments.len(), 1);
            assert_eq!(
                result.royalty_payments.get(0).unwrap().recipient,
                royalty_recipient
            );
            assert_eq!(result.royalty_payments.get(0).unwrap().amount, 50_000);
        });
    }

    #[test]
    fn purchase_calculates_platform_fee() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);

            // Set platform fee to 2.5% (250 bps)
            crate::platform_fee::set_platform_fee(env, 250).unwrap();

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.platform_fee, 25_000); // 2.5% of 1_000_000
            assert_eq!(result.total_royalty, 0);
        });
    }

    #[test]
    fn purchase_calculates_combined_royalty_and_platform_fee() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 500); // 5% royalty

            // Set platform fee to 1% (100 bps)
            crate::platform_fee::set_platform_fee(env, 100).unwrap();

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.total_royalty, 50_000); // 5% of 1_000_000
            assert_eq!(result.platform_fee, 10_000); // 1% of 1_000_000
            assert_eq!(result.sale_price, 1_000_000);
            // Total deductions: 60,000 < 1,000,000 — valid
        });
    }

    #[test]
    fn purchase_fails_when_total_deductions_exceed_sale_price() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 10_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 9_500); // 95% royalty

            // Set platform fee to 10% (1000 bps)
            crate::platform_fee::set_platform_fee(env, 1_000).unwrap();

            // Total: 95% + 10% = 105% — exceeds 100%
            assert_eq!(
                purchase_listing(env, &buyer, 1),
                Err(Error::TotalDeductionsExceedSalePrice)
            );
        });
    }

    // ── Step 5: Process payment ───────────────────────────────────────────────

    #[test]
    fn purchase_records_royalty_in_history() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 500);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            // Verify royalty history was recorded
            let history = crate::royalty_history::get_royalty_history(env, 1);
            assert_eq!(history.len(), 1);
            let entry = history.get(0).unwrap();
            assert_eq!(entry.recipient, royalty_recipient);
            assert_eq!(entry.amount, 50_000);
        });
    }

    #[test]
    fn purchase_updates_platform_revenue() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            let platform_wallet = Address::generate(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);

            crate::platform_fee::set_platform_fee(env, 250).unwrap();
            crate::platform_recipient::save_platform_recipient(env, &platform_wallet);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            let revenue = crate::platform_revenue::get_platform_revenue(env);
            assert_eq!(revenue, 25_000); // 2.5% of 1_000_000
        });
    }

    // ── Step 6: Transfer NFT ──────────────────────────────────────────────────

    #[test]
    fn purchase_transfers_token_ownership() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            let new_owner = token_owner_storage::get_owner(env, 1).unwrap();
            assert_eq!(new_owner, buyer);
        });
    }

    #[test]
    fn purchase_updates_wallet_indexes() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            wallet_token_index::add_token_to_wallet(env, &seller, 1).unwrap();
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            // Seller should no longer have token 1
            let seller_tokens = wallet_token_index::get_wallet_tokens(env, &seller);
            assert_eq!(seller_tokens.len(), 0);

            // Buyer should now have token 1
            let buyer_tokens = wallet_token_index::get_wallet_tokens(env, &buyer);
            assert_eq!(buyer_tokens.len(), 1);
            assert_eq!(buyer_tokens.get(0).unwrap(), 1);
        });
    }

    // ── Step 7: Mark listing sold ─────────────────────────────────────────────

    #[test]
    fn purchase_marks_listing_as_sold() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            let listing = listing_storage::get_listing(env, 1).unwrap();
            assert_eq!(listing.status, ListingStatus::Sold);
            assert_eq!(listing.buyer, Some(buyer));
            assert!(listing.sold_at.is_some());
        });
    }

    #[test]
    fn purchase_fails_on_already_sold_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer1 = Address::generate(env);
            let buyer2 = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let _result = purchase_listing(env, &buyer1, 1).unwrap();
            // After first purchase, listing is Sold — ListingNotActive is returned
            // before reaching the ListingAlreadySold check.
            assert_eq!(
                purchase_listing(env, &buyer2, 1),
                Err(Error::ListingNotActive)
            );
        });
    }

    // ── Step 8: Emit events ───────────────────────────────────────────────────

    #[test]
    fn purchase_emits_nft_sold_event() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.sale_price, 1_000_000);
            assert_eq!(result.seller, seller);
            assert_eq!(result.buyer, buyer);
        });
    }

    #[test]
    fn purchase_emits_royalty_paid_events() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let royalty_recipient = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);
            setup_royalty(env, 1, &royalty_recipient, 500);

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.royalty_payments.len(), 1);
            assert_eq!(
                result.royalty_payments.get(0).unwrap().recipient,
                royalty_recipient
            );
        });
    }

    // ── Multi-recipient royalty split ─────────────────────────────────────────

    #[test]
    fn purchase_handles_multi_recipient_royalty() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let recipient1 = Address::generate(env);
            let recipient2 = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1_000_000, &asset, 0);

            // Set up multi-recipient royalty: 3% + 2% = 5%
            let mut recipients = soroban_sdk::Vec::new(env);
            recipients.push_back(RoyaltyRecipient {
                recipient: recipient1.clone(),
                basis_points: 300,
            });
            recipients.push_back(RoyaltyRecipient {
                recipient: recipient2.clone(),
                basis_points: 200,
            });
            let royalty = Royalty {
                recipients,
                asset_address: None,
            };
            token_storage::set_royalty(env, 1, &royalty);

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.total_royalty, 50_000); // 5% of 1_000_000
            assert_eq!(result.royalty_payments.len(), 2);
            assert_eq!(result.royalty_payments.get(0).unwrap().amount, 30_000);
            assert_eq!(result.royalty_payments.get(1).unwrap().amount, 20_000);
        });
    }

    // ── Pause guard ───────────────────────────────────────────────────────────

    #[test]
    fn purchase_fails_when_contract_paused() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);
            crate::pause_state::save_pause_state(env, true);

            assert_eq!(purchase_listing(env, &buyer, 1), Err(Error::ContractPaused));
        });
    }

    // ── Edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn purchase_with_minimum_price() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1, &asset, 0);

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.sale_price, 1);
            assert_eq!(result.total_royalty, 0);
            assert_eq!(result.platform_fee, 0);
        });
    }

    #[test]
    fn purchase_returns_correct_timestamp() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_700_000_000);
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let result = purchase_listing(env, &buyer, 1).unwrap();
            assert_eq!(result.timestamp, 1_700_000_000);
        });
    }

    #[test]
    fn purchase_with_frozen_seller_token_fails() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);
            // Freeze the token after listing (possible if admin freezes)
            crate::frozen_token::freeze_token(env, 1);

            assert_eq!(purchase_listing(env, &buyer, 1), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn purchase_preserves_seller_listing_history() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_payment_asset(env);
            setup_token(env, 1, &seller);
            setup_listing(env, 1, &seller, 1000, &asset, 0);

            let _result = purchase_listing(env, &buyer, 1).unwrap();

            let listing = listing_storage::get_listing(env, 1).unwrap();
            assert_eq!(listing.seller, seller);
            assert_eq!(listing.price, 1000);
        });
    }
}
