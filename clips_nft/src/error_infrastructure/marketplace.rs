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

use soroban_sdk::contracterror;

/// Error type for marketplace and payment operations.
///
/// These errors cover marketplace-specific failures:
/// - Payment asset is not supported.
/// - Duplicate active listings for the same NFT.
/// - Buyer payment amount is insufficient.
/// - Marketplace listing or offer has expired.
///
/// # Error Codes
///
/// Same as the `260–263` block in the centralized error registry.
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
        }
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        match self {
            MarketplaceError::UnsupportedPaymentAsset => "UnsupportedPaymentAsset",
            MarketplaceError::DuplicateListing => "DuplicateListing",
            MarketplaceError::InsufficientPayment => "InsufficientPayment",
            MarketplaceError::ExpiredListing => "ExpiredListing",
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
        assert!(MarketplaceError::from_code(264).is_none());
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

    // ── Documentation (acceptance: "Add tests") ───────────────────────────────

    #[test]
    fn all_marketplace_errors_have_numeric_codes() {
        let errors = [
            MarketplaceError::UnsupportedPaymentAsset,
            MarketplaceError::DuplicateListing,
            MarketplaceError::InsufficientPayment,
            MarketplaceError::ExpiredListing,
        ];
        for error in errors {
            assert!(error.code() >= 260);
            assert!(error.code() <= 263);
            assert!(!error.name().is_empty());
        }
    }
}
