//! NFT Offer Struct — dedicated representation of a buyer's offer for an NFT.
//!
//! This module provides the canonical [`NftOffer`] struct used across the
//! contract to represent a buyer-originated bid on a specific NFT.  It
//! deliberately mirrors the field semantics of [`crate::marketplace::types::Offer`]
//! while exposing names that match the acceptance criteria exactly:
//!
//! | Acceptance-criteria field | [`NftOffer`] field  | Notes                               |
//! |---------------------------|---------------------|-------------------------------------|
//! | Offer ID                  | `offer_id`          | Auto-incremented unique identifier  |
//! | Token ID                  | `token_id`          | On-chain token targeted by the offer|
//! | Buyer                     | `buyer`             | Address of the bidder               |
//! | Offer amount              | `offer_amount`      | Price in payment asset's base unit  |
//! | Payment asset             | `payment_asset`     | Token contract address for payment  |
//! | Expiration                | `expiration`        | Unix timestamp; `0` = never expires |
//! | Status                    | `status`            | [`NftOfferStatus`] lifecycle state  |
//!
//! # Design
//!
//! An additional `created_at` field is included to give indexers a reliable
//! on-chain creation timestamp without requiring additional storage reads.
//!
//! # Example
//!
//! ```rust,ignore
//! use clips_nft::nft_offer::{NftOffer, NftOfferStatus};
//!
//! let offer = NftOffer {
//!     offer_id:      1,
//!     token_id:      42,
//!     buyer:         buyer_address,
//!     offer_amount:  5_000_000,
//!     payment_asset: xlm_contract,
//!     expiration:    env.ledger().timestamp() + 86_400,
//!     status:        NftOfferStatus::Active,
//!     created_at:    env.ledger().timestamp(),
//! };
//! ```
//!
//! # Conversions
//!
//! Bidirectional conversion helpers are provided between [`NftOffer`] and the
//! internal [`crate::marketplace::types::Offer`] type so that existing
//! marketplace storage and guard modules can be used unchanged:
//!
//! - [`NftOffer::from_marketplace_offer`] — wrap an internal `Offer`
//! - [`NftOffer::into_marketplace_offer`] — unwrap to the internal `Offer`

use soroban_sdk::{contracttype, Address};

use crate::marketplace::types::{Offer, OfferStatus};
use crate::types::TokenId;

// ── Status ────────────────────────────────────────────────────────────────────

/// Lifecycle state of an [`NftOffer`].
///
/// A freshly created offer starts as [`Active`][NftOfferStatus::Active] and
/// transitions to one of the three terminal states.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NftOfferStatus {
    /// Offer is open and can be accepted by the current token owner.
    Active,
    /// Offer was accepted; the token has been transferred to the buyer.
    Accepted,
    /// Offer was withdrawn by the buyer or cancelled by an authorized party.
    Cancelled,
    /// Offer's expiration timestamp has passed without acceptance.
    Expired,
}

impl NftOfferStatus {
    /// Convert from the internal marketplace [`OfferStatus`].
    pub fn from_offer_status(s: &OfferStatus) -> Self {
        match s {
            OfferStatus::Active => NftOfferStatus::Active,
            OfferStatus::Accepted => NftOfferStatus::Accepted,
            OfferStatus::Cancelled => NftOfferStatus::Cancelled,
            OfferStatus::Expired => NftOfferStatus::Expired,
        }
    }

    /// Convert to the internal marketplace [`OfferStatus`].
    pub fn to_offer_status(&self) -> OfferStatus {
        match self {
            NftOfferStatus::Active => OfferStatus::Active,
            NftOfferStatus::Accepted => OfferStatus::Accepted,
            NftOfferStatus::Cancelled => OfferStatus::Cancelled,
            NftOfferStatus::Expired => OfferStatus::Expired,
        }
    }
}

// ── Struct ────────────────────────────────────────────────────────────────────

/// A buyer's offer to purchase a specific NFT.
///
/// # Fields
///
/// - `offer_id`      — Unique, auto-incremented identifier for this offer.
/// - `token_id`      — On-chain token identifier targeted by this offer.
/// - `buyer`         — Address of the account that placed the offer.
/// - `offer_amount`  — The proposed purchase price in the payment asset's
///                     smallest denomination (e.g. stroops for XLM).
/// - `payment_asset` — Contract address of the token used for payment.
/// - `expiration`    — Unix timestamp after which the offer is no longer valid.
///                     A value of `0` means the offer never expires.
/// - `status`        — Current lifecycle state (see [`NftOfferStatus`]).
/// - `created_at`    — Ledger timestamp (seconds since Unix epoch) at creation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftOffer {
    /// Unique offer identifier (auto-incremented).
    pub offer_id: u64,
    /// On-chain token identifier targeted by this offer.
    pub token_id: TokenId,
    /// Address of the account that placed the offer.
    pub buyer: Address,
    /// Proposed purchase price in the payment asset's base unit.
    pub offer_amount: i128,
    /// Contract address of the token accepted as payment.
    pub payment_asset: Address,
    /// Unix expiration timestamp; `0` means the offer never expires.
    pub expiration: u64,
    /// Current lifecycle state of the offer.
    pub status: NftOfferStatus,
    /// Ledger timestamp (seconds since Unix epoch) when the offer was created.
    pub created_at: u64,
}

impl NftOffer {
    /// Build an [`NftOffer`] from an internal marketplace [`Offer`].
    ///
    /// Field mapping:
    ///
    /// | `Offer` field     | `NftOffer` field   |
    /// |-------------------|--------------------|
    /// | `offer_id`        | `offer_id`         |
    /// | `token_id`        | `token_id`         |
    /// | `buyer`           | `buyer`            |
    /// | `price`           | `offer_amount`     |
    /// | `payment_asset`   | `payment_asset`    |
    /// | `expires_at`      | `expiration`       |
    /// | `status`          | `status`           |
    /// | `created_at`      | `created_at`       |
    pub fn from_marketplace_offer(offer: &Offer) -> Self {
        Self {
            offer_id: offer.offer_id,
            token_id: offer.token_id,
            buyer: offer.buyer.clone(),
            offer_amount: offer.price,
            payment_asset: offer.payment_asset.clone(),
            expiration: offer.expires_at,
            status: NftOfferStatus::from_offer_status(&offer.status),
            created_at: offer.created_at,
        }
    }

    /// Consume this [`NftOffer`] and produce the internal marketplace [`Offer`].
    ///
    /// The reverse of [`from_marketplace_offer`][NftOffer::from_marketplace_offer].
    pub fn into_marketplace_offer(self) -> Offer {
        Offer {
            offer_id: self.offer_id,
            token_id: self.token_id,
            buyer: self.buyer,
            price: self.offer_amount,
            payment_asset: self.payment_asset,
            expires_at: self.expiration,
            status: self.status.to_offer_status(),
            created_at: self.created_at,
        }
    }

    /// Return `true` if the offer is currently [`Active`][NftOfferStatus::Active].
    pub fn is_active(&self) -> bool {
        matches!(self.status, NftOfferStatus::Active)
    }

    /// Return `true` if the offer has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        !self.is_active()
    }

    /// Return `true` if `timestamp` is past the offer's expiration.
    ///
    /// Always returns `false` when `expiration == 0` (i.e. never-expires).
    pub fn is_expired_at(&self, timestamp: u64) -> bool {
        self.expiration != 0 && timestamp > self.expiration
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::OfferStatus;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    fn sample_offer(env: &Env, buyer: &Address, asset: &Address) -> Offer {
        Offer {
            offer_id: 1,
            token_id: 42,
            buyer: buyer.clone(),
            price: 1_000_000,
            payment_asset: asset.clone(),
            expires_at: 9_999_999,
            status: OfferStatus::Active,
            created_at: 1_000,
        }
    }

    // ── NftOfferStatus conversion ─────────────────────────────────────────────

    #[test]
    fn status_round_trips_active() {
        let s = NftOfferStatus::from_offer_status(&OfferStatus::Active);
        assert_eq!(s, NftOfferStatus::Active);
        assert_eq!(s.to_offer_status(), OfferStatus::Active);
    }

    #[test]
    fn status_round_trips_accepted() {
        let s = NftOfferStatus::from_offer_status(&OfferStatus::Accepted);
        assert_eq!(s, NftOfferStatus::Accepted);
        assert_eq!(s.to_offer_status(), OfferStatus::Accepted);
    }

    #[test]
    fn status_round_trips_cancelled() {
        let s = NftOfferStatus::from_offer_status(&OfferStatus::Cancelled);
        assert_eq!(s, NftOfferStatus::Cancelled);
        assert_eq!(s.to_offer_status(), OfferStatus::Cancelled);
    }

    #[test]
    fn status_round_trips_expired() {
        let s = NftOfferStatus::from_offer_status(&OfferStatus::Expired);
        assert_eq!(s, NftOfferStatus::Expired);
        assert_eq!(s.to_offer_status(), OfferStatus::Expired);
    }

    // ── NftOffer fields ───────────────────────────────────────────────────────

    #[test]
    fn from_marketplace_offer_maps_all_fields() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let internal = sample_offer(&env, &buyer, &asset);

        let nft_offer = NftOffer::from_marketplace_offer(&internal);

        assert_eq!(nft_offer.offer_id, 1);
        assert_eq!(nft_offer.token_id, 42);
        assert_eq!(nft_offer.buyer, buyer);
        assert_eq!(nft_offer.offer_amount, 1_000_000);
        assert_eq!(nft_offer.payment_asset, asset);
        assert_eq!(nft_offer.expiration, 9_999_999);
        assert_eq!(nft_offer.status, NftOfferStatus::Active);
        assert_eq!(nft_offer.created_at, 1_000);
    }

    #[test]
    fn into_marketplace_offer_round_trips_fields() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let internal = sample_offer(&env, &buyer, &asset);

        let nft_offer = NftOffer::from_marketplace_offer(&internal);
        let back = nft_offer.into_marketplace_offer();

        assert_eq!(back.offer_id, internal.offer_id);
        assert_eq!(back.token_id, internal.token_id);
        assert_eq!(back.buyer, internal.buyer);
        assert_eq!(back.price, internal.price);
        assert_eq!(back.payment_asset, internal.payment_asset);
        assert_eq!(back.expires_at, internal.expires_at);
        assert_eq!(back.created_at, internal.created_at);
    }

    // ── Helper predicates ─────────────────────────────────────────────────────

    #[test]
    fn is_active_returns_true_for_active_status() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let offer = NftOffer::from_marketplace_offer(&sample_offer(&env, &buyer, &asset));
        assert!(offer.is_active());
        assert!(!offer.is_terminal());
    }

    #[test]
    fn is_active_returns_false_for_accepted_status() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let mut internal = sample_offer(&env, &buyer, &asset);
        internal.status = OfferStatus::Accepted;
        let offer = NftOffer::from_marketplace_offer(&internal);
        assert!(!offer.is_active());
        assert!(offer.is_terminal());
    }

    #[test]
    fn is_expired_at_returns_false_when_zero_expiration() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let mut internal = sample_offer(&env, &buyer, &asset);
        internal.expires_at = 0;
        let offer = NftOffer::from_marketplace_offer(&internal);
        // timestamp is well past 0 but offer never expires
        assert!(!offer.is_expired_at(99_999_999));
    }

    #[test]
    fn is_expired_at_returns_true_when_past_expiration() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let mut internal = sample_offer(&env, &buyer, &asset);
        internal.expires_at = 1_000;
        let offer = NftOffer::from_marketplace_offer(&internal);
        assert!(offer.is_expired_at(1_001));
        assert!(!offer.is_expired_at(1_000)); // equal timestamp is NOT expired
        assert!(!offer.is_expired_at(999));
    }

    // ── Acceptance criteria field presence (compile-time) ─────────────────────

    #[test]
    fn all_acceptance_criteria_fields_are_accessible() {
        let (env, _) = make_env();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let offer = NftOffer {
            offer_id: 7,
            token_id: 3,
            buyer: buyer.clone(),
            offer_amount: 500,
            payment_asset: asset.clone(),
            expiration: 0,
            status: NftOfferStatus::Active,
            created_at: 100,
        };
        // All seven acceptance-criteria fields exist and are readable:
        let _ = offer.offer_id;       // Offer ID
        let _ = offer.token_id;       // Token ID
        let _ = offer.buyer;          // Buyer
        let _ = offer.offer_amount;   // Offer amount
        let _ = offer.payment_asset;  // Payment asset
        let _ = offer.expiration;     // Expiration
        let _ = offer.status;         // Status
    }
}
