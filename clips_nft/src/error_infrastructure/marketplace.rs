//! Marketplace and payment error types (issue #???).
//!
//! Defines the standardized errors returned by marketplace operations and
//! payment processing. Each error carries a unique code from the `marketplace`
//! module block (`260–263`) and is documented in the central
//! [`crate::error_infrastructure::registry`].
//!
//! # Error Codes
//!
//! * `260` — `UnsupportedPaymentAsset` — Payment asset not supported for marketplace/royalty operations.
//! * `261` — `DuplicateListing` — Duplicate active listing already exists for the same NFT.
//! * `262` — `InsufficientPayment` — Buyer has not provided sufficient funds for the purchase.
//! * `263` — `ExpiredListing` — Marketplace listing or offer has expired.
//! * `264` — `ListingNotFound` — Referenced marketplace listing does not exist.
//! * `265` — `ListingAlreadyExists` — Duplicate active listing already exists for the same NFT.
//! * `266` — `ListingExpired` — Marketplace listing has expired and can no longer be interacted with.

use soroban_sdk::contracterror;

/// Error type for marketplace and payment operations.
///
/// These errors cover marketplace-specific failures:
/// - Payment asset is not supported.
/// - Duplicate active listings for the same NFT.
/// - Buyer payment amount is insufficient.
/// - Marketplace listing or offer has expired.
/// - Referenced listing does not exist.
/// - Listing already exists for the same NFT.
/// - Listing has expired.
///
/// # Error Codes
///
/// Same as the `260–266` block in the centralized error registry.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    /// Payment asset is not supported by the marketplace or for royalty operations.
    UnsupportedPaymentAsset = 260,

    /// Duplicate active listing already exists for the same NFT.
    DuplicateListing = 261,

    /// Buyer has not provided sufficient funds to complete the purchase at the requested price.
    InsufficientPayment = 262,

    /// Marketplace listing or offer has expired and is no longer valid.
    ExpiredListing = 263,

    /// Referenced marketplace listing does not exist.
    ListingNotFound = 264,

    /// Duplicate active listing already exists for the same NFT.
    ListingAlreadyExists = 265,

    /// Marketplace listing has expired and can no longer be interacted with.
    ListingExpired = 266,
}

impl MarketplaceError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "marketplace";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        match self {
            MarketplaceError::UnsupportedPaymentAsset => 260,
            MarketplaceError::DuplicateListing => 261,
            MarketplaceError::InsufficientPayment => 262,
            MarketplaceError::ExpiredListing => 263,
            MarketplaceError::ListingNotFound => 264,
            MarketplaceError::ListingAlreadyExists => 265,
            MarketplaceError::ListingExpired => 266,
        }
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        match self {
            MarketplaceError::UnsupportedPaymentAsset => "UnsupportedPaymentAsset",
            MarketplaceError::DuplicateListing => "DuplicateListing",
            MarketplaceError::InsufficientPayment => "InsufficientPayment",
            MarketplaceError::ExpiredListing => "ExpiredListing",
            MarketplaceError::ListingNotFound => "ListingNotFound",
            MarketplaceError::ListingAlreadyExists => "ListingAlreadyExists",
            MarketplaceError::ListingExpired => "ListingExpired",
        }
    }

    /// Decode a `MarketplaceError` from a numeric code.
    ///
    /// Returns `None` for codes that do not map to a marketplace error,
    /// allowing callers to distinguish unknown codes without panicking.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            260 => Some(MarketplaceError::UnsupportedPaymentAsset),
            261 => Some(MarketplaceError::DuplicateListing),
            262 => Some(MarketplaceError::InsufficientPayment),
            263 => Some(MarketplaceError::ExpiredListing),
            264 => Some(MarketplaceError::ListingNotFound),
            265 => Some(MarketplaceError::ListingAlreadyExists),
            266 => Some(MarketplaceError::ListingExpired),
            _ => None,
        }
    }
}

// ── Reusable helpers returning standardized marketplace errors ──────────────

/// Standardized error for an unsupported payment asset.
pub const fn unsupported_payment_asset() -> MarketplaceError {
    MarketplaceError::UnsupportedPaymentAsset
}

/// Standardized error for a duplicate active listing.
pub const fn duplicate_listing() -> MarketplaceError {
    MarketplaceError::DuplicateListing
}

/// Standardized error for insufficient payment.
pub const fn insufficient_payment() -> MarketplaceError {
    MarketplaceError::InsufficientPayment
}

/// Standardized error for an expired listing.
pub const fn expired_listing() -> MarketplaceError {
    MarketplaceError::ExpiredListing
}

/// Standardized error for a listing that does not exist.
pub const fn listing_not_found() -> MarketplaceError {
    MarketplaceError::ListingNotFound
}

/// Standardized error for a listing that already exists for the same NFT.
pub const fn listing_already_exists() -> MarketplaceError {
    MarketplaceError::ListingAlreadyExists
}

/// Standardized error for a listing that has expired.
pub const fn listing_expired() -> MarketplaceError {
    MarketplaceError::ListingExpired
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unique codes (acceptance: "Assign unique codes") ────────────────────

    #[test]
    fn marketplace_error_codes_are_unique() {
        let codes = [
            MarketplaceError::UnsupportedPaymentAsset.code(),
            MarketplaceError::DuplicateListing.code(),
            MarketplaceError::InsufficientPayment.code(),
            MarketplaceError::ExpiredListing.code(),
            MarketplaceError::ListingNotFound.code(),
            MarketplaceError::ListingAlreadyExists.code(),
            MarketplaceError::ListingExpired.code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len());
    }

    #[test]
    fn marketplace_error_names_are_unique() {
        let names = [
            MarketplaceError::UnsupportedPaymentAsset.name(),
            MarketplaceError::DuplicateListing.name(),
            MarketplaceError::InsufficientPayment.name(),
            MarketplaceError::ExpiredListing.name(),
            MarketplaceError::ListingNotFound.name(),
            MarketplaceError::ListingAlreadyExists.name(),
            MarketplaceError::ListingExpired.name(),
        ];
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    // ── Correct codes (acceptance: "Assign unique codes") ─────────────────────

    #[test]
    fn marketplace_error_codes_match_registry_block() {
        assert_eq!(MarketplaceError::UnsupportedPaymentAsset.code(), 260);
        assert_eq!(MarketplaceError::DuplicateListing.code(), 261);
        assert_eq!(MarketplaceError::InsufficientPayment.code(), 262);
        assert_eq!(MarketplaceError::ExpiredListing.code(), 263);
        assert_eq!(MarketplaceError::ListingNotFound.code(), 264);
        assert_eq!(MarketplaceError::ListingAlreadyExists.code(), 265);
        assert_eq!(MarketplaceError::ListingExpired.code(), 266);
    }

    // ── Correct names (acceptance: "Define error name") ────────────────────────

    #[test]
    fn marketplace_error_names_are_correct() {
        assert_eq!(
            MarketplaceError::UnsupportedPaymentAsset.name(),
            "UnsupportedPaymentAsset"
        );
        assert_eq!(MarketplaceError::DuplicateListing.name(), "DuplicateListing");
        assert_eq!(
            MarketplaceError::InsufficientPayment.name(),
            "InsufficientPayment"
        );
        assert_eq!(MarketplaceError::ExpiredListing.name(), "ExpiredListing");
        assert_eq!(MarketplaceError::ListingNotFound.name(), "ListingNotFound");
        assert_eq!(
            MarketplaceError::ListingAlreadyExists.name(),
            "ListingAlreadyExists"
        );
        assert_eq!(MarketplaceError::ListingExpired.name(), "ListingExpired");
    }

    // ── Module ownership (acceptance: "Return standardized error") ─────────────

    #[test]
    fn all_marketplace_errors_belong_to_marketplace_module() {
        assert_eq!(MarketplaceError::MODULE, "marketplace");
    }

    // ── Serialization round-trips (acceptance: "Use across payment modules") ──

    #[test]
    fn marketplace_error_serialization_roundtrip() {
        let all = [
            MarketplaceError::UnsupportedPaymentAsset,
            MarketplaceError::DuplicateListing,
            MarketplaceError::InsufficientPayment,
            MarketplaceError::ExpiredListing,
            MarketplaceError::ListingNotFound,
            MarketplaceError::ListingAlreadyExists,
            MarketplaceError::ListingExpired,
        ];
        for error in all {
            let code = error.code();
            let decoded = MarketplaceError::from_code(code);
            assert_eq!(
                decoded,
                Some(error),
                "round-trip failed for code {}",
                code
            );
        }
    }

    #[test]
    fn marketplace_error_from_code_rejects_unknown_codes() {
        assert!(MarketplaceError::from_code(0).is_none());
        assert!(MarketplaceError::from_code(259).is_none());
        assert!(MarketplaceError::from_code(267).is_none());
        assert!(MarketplaceError::from_code(9999).is_none());
    }

    // ── Helper functions (acceptance: "Return standardized error") ─────────────

    #[test]
    fn unsupported_payment_asset_helper_returns_correct_error() {
        let error = unsupported_payment_asset();
        assert_eq!(error, MarketplaceError::UnsupportedPaymentAsset);
        assert_eq!(error.code(), 260);
    }

    #[test]
    fn duplicate_listing_helper_returns_correct_error() {
        let error = duplicate_listing();
        assert_eq!(error, MarketplaceError::DuplicateListing);
        assert_eq!(error.code(), 261);
    }

    #[test]
    fn insufficient_payment_helper_returns_correct_error() {
        let error = insufficient_payment();
        assert_eq!(error, MarketplaceError::InsufficientPayment);
        assert_eq!(error.code(), 262);
    }

    #[test]
    fn expired_listing_helper_returns_correct_error() {
        let error = expired_listing();
        assert_eq!(error, MarketplaceError::ExpiredListing);
        assert_eq!(error.code(), 263);
    }

    #[test]
    fn listing_not_found_helper_returns_correct_error() {
        let error = listing_not_found();
        assert_eq!(error, MarketplaceError::ListingNotFound);
        assert_eq!(error.code(), 264);
    }

    #[test]
    fn listing_already_exists_helper_returns_correct_error() {
        let error = listing_already_exists();
        assert_eq!(error, MarketplaceError::ListingAlreadyExists);
        assert_eq!(error.code(), 265);
    }

    #[test]
    fn listing_expired_helper_returns_correct_error() {
        let error = listing_expired();
        assert_eq!(error, MarketplaceError::ListingExpired);
        assert_eq!(error.code(), 266);
    }

    // ── Documentation (acceptance: "Add tests") ───────────────────────────────

    #[test]
    fn all_marketplace_errors_have_numeric_codes() {
        let errors = [
            MarketplaceError::UnsupportedPaymentAsset,
            MarketplaceError::DuplicateListing,
            MarketplaceError::InsufficientPayment,
            MarketplaceError::ExpiredListing,
            MarketplaceError::ListingNotFound,
            MarketplaceError::ListingAlreadyExists,
            MarketplaceError::ListingExpired,
        ];
        for error in errors {
            assert!(error.code() >= 260);
            assert!(error.code() <= 266);
            assert!(!error.name().is_empty());
        }
    }
}
