//! Error category constants (issue #985).
//!
//! Categories make contract failures easier for developers and indexers to
//! classify. Every standardized error code from the centralized registry is
//! assigned to one or more categories, and [`categorize_by_code`] maps a raw
//! code to its primary category.

/// High-level classification for a contract failure.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub enum ErrorCategory {
    /// Input did not pass validation (codes 220–225, 240–241, 290–293).
    Validation,
    /// Caller lacked the required permission (codes 270–273).
    /// Input did not pass validation (codes 220–225, 240–241, 290–293, 294–296).
    Validation,
    /// Caller lacked the required permission (codes 231, 270–273, 297).
    Authorization,
    /// Caller did not own the referenced asset (codes 231, 281–282).
    Ownership,
    /// Payment or fee processing failed (codes 213, 260, 262).
    Payment,
    /// Marketplace operation failed (codes 260–263).
    Marketplace,
    /// Royalty configuration or payment failed (codes 211, 260).
    Royalty,
    /// Storage read/write failed (code 230).
    Storage,
    /// Security-sensitive restriction triggered (codes 242, 250, 260–263, 280, 283–284).
    /// Security-sensitive restriction triggered (codes 242, 250, 298–302).
    Security,
    /// Contract/configuration state is invalid (codes 200–204, 210–214).
    Configuration,
}

/// Every category in declaration order.
pub const ALL_CATEGORIES: [ErrorCategory; 9] = [
    ErrorCategory::Validation,
    ErrorCategory::Authorization,
    ErrorCategory::Ownership,
    ErrorCategory::Payment,
    ErrorCategory::Marketplace,
    ErrorCategory::Royalty,
    ErrorCategory::Storage,
    ErrorCategory::Security,
    ErrorCategory::Configuration,
];

impl ErrorCategory {
    /// Stable, lowercase machine-readable name for the category.
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::Validation => "validation",
            ErrorCategory::Authorization => "authorization",
            ErrorCategory::Ownership => "ownership",
            ErrorCategory::Payment => "payment",
            ErrorCategory::Marketplace => "marketplace",
            ErrorCategory::Royalty => "royalty",
            ErrorCategory::Storage => "storage",
            ErrorCategory::Security => "security",
            ErrorCategory::Configuration => "configuration",
        }
    }

    /// Short description of the category for indexers and loggers.
    pub const fn description(self) -> &'static str {
        match self {
            ErrorCategory::Validation => "Invalid input failed validation",
            ErrorCategory::Authorization => "Caller is not authorized to perform the operation",
            ErrorCategory::Ownership => "Caller does not own the referenced asset",
            ErrorCategory::Payment => "Payment or fee processing failed",
            ErrorCategory::Marketplace => "Marketplace listing or offer operation failed",
            ErrorCategory::Royalty => "Royalty configuration or payment failed",
            ErrorCategory::Storage => "Storage read or write failed",
            ErrorCategory::Security => "Security-sensitive restriction was triggered",
            ErrorCategory::Configuration => "Contract or configuration state is invalid",
        }
    }

    /// Numeric category id used for compact serialization.
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Error codes assigned to this category (from the centralized registry).
    pub const fn error_codes(self) -> &'static [u32] {
        match self {
            ErrorCategory::Validation => &[220, 221, 222, 223, 224, 225, 240, 241, 290, 291, 292, 293],
            ErrorCategory::Authorization => &[231, 270, 271, 272, 273],
            ErrorCategory::Ownership => &[231, 281, 282],
            ErrorCategory::Payment => &[213, 260, 262],
            ErrorCategory::Marketplace => &[260, 261, 262, 263],
            ErrorCategory::Royalty => &[211, 260],
            ErrorCategory::Authorization => &[231, 270, 271, 272, 273],
            ErrorCategory::Ownership => &[231, 281, 282],
            ErrorCategory::Payment => &[213],
            ErrorCategory::Marketplace => &[213, 260, 261, 262, 263],
            ErrorCategory::Royalty => &[211, 260],
            ErrorCategory::Validation => &[220, 221, 222, 223, 224, 225, 240, 241],
            ErrorCategory::Authorization => &[231],
            ErrorCategory::Ownership => &[231],
            ErrorCategory::Payment => &[213, 260, 262],
            ErrorCategory::Marketplace => &[260, 261, 262, 263],
            ErrorCategory::Royalty => &[211, 260],
            ErrorCategory::Validation => &[
                220, 221, 222, 223, 224, 225, 240, 241, 290, 291, 292, 293, 294, 295, 296,
            ],
            ErrorCategory::Authorization => &[231, 270, 271, 272, 273, 297],
            ErrorCategory::Validation => &[
                220, 221, 222, 223, 224, 225, 240, 241, 290, 291, 292, 293, 294, 295, 296, 297,
                298, 299,
            ],
            ErrorCategory::Authorization => &[231, 270, 271, 272, 273],
            ErrorCategory::Ownership => &[231, 281, 282],
            ErrorCategory::Payment => &[213],
            ErrorCategory::Marketplace => &[213, 300, 301, 302],
            ErrorCategory::Royalty => &[211],
            ErrorCategory::Storage => &[230],
            ErrorCategory::Security => &[
                242, 250, 260, 261, 262, 263, 280, 283, 284, 298, 299, 300, 301, 302,
            ],
            ErrorCategory::Configuration => &[200, 201, 202, 203, 204, 210, 211, 212, 213, 214],
        }
    }

    /// Decode a category from its numeric [`Self::code`].
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(ErrorCategory::Validation),
            1 => Some(ErrorCategory::Authorization),
            2 => Some(ErrorCategory::Ownership),
            3 => Some(ErrorCategory::Payment),
            4 => Some(ErrorCategory::Marketplace),
            5 => Some(ErrorCategory::Royalty),
            6 => Some(ErrorCategory::Storage),
            7 => Some(ErrorCategory::Security),
            8 => Some(ErrorCategory::Configuration),
            _ => None,
        }
    }
}

/// Return the primary category for a standardized error `code`.
///
/// Returns `None` for codes that are not part of the centralized error space.
pub const fn categorize_by_code(code: u32) -> Option<ErrorCategory> {
    match code {
        220..=225 | 240 | 241 | 290..=296 => Some(ErrorCategory::Validation),
        200..=204 | 210..=214 => Some(ErrorCategory::Configuration),
        230 => Some(ErrorCategory::Storage),
        231 | 281..=282 => Some(ErrorCategory::Ownership),
        270..=273 | 297 => Some(ErrorCategory::Authorization),
        242 | 250 | 260..=263 | 280 | 283..=284 | 298..=302 => Some(ErrorCategory::Security),
        220..=225 | 240 | 241 | 290..=299 => Some(ErrorCategory::Validation),
        200..=204 | 210..=214 => Some(ErrorCategory::Configuration),
        230 => Some(ErrorCategory::Storage),
        231 | 281..=282 => Some(ErrorCategory::Ownership),
        270..=273 => Some(ErrorCategory::Authorization),
        242 | 250 | 260..=263 | 280 | 283..=284 => Some(ErrorCategory::Security),
        // InsufficientPayment (#1399) and UnsupportedAsset (#1400).
        260 | 262 => Some(ErrorCategory::Payment),
        300..=302 => Some(ErrorCategory::Marketplace),
        _ => None,
    }
}

/// Return every category that lists `code` in [`ErrorCategory::error_codes`].
///
/// Unlike [`categorize_by_code`], which returns the single primary category,
/// this reports all categories a code belongs to (codes may overlap).
pub fn categories_for_code(code: u32) -> alloc::vec::Vec<ErrorCategory> {
    ALL_CATEGORIES
        .iter()
        .copied()
        .filter(|category| category.error_codes().contains(&code))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn every_category_has_a_unique_str_and_code() {
        let mut names: Vec<&str> = ALL_CATEGORIES.iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL_CATEGORIES.len());

        let mut codes: Vec<u8> = ALL_CATEGORIES.iter().map(|c| c.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), ALL_CATEGORIES.len());
    }

    #[test]
    fn category_str_names_match_acceptance_criteria() {
        assert_eq!(ErrorCategory::Validation.as_str(), "validation");
        assert_eq!(ErrorCategory::Authorization.as_str(), "authorization");
        assert_eq!(ErrorCategory::Ownership.as_str(), "ownership");
        assert_eq!(ErrorCategory::Payment.as_str(), "payment");
        assert_eq!(ErrorCategory::Marketplace.as_str(), "marketplace");
        assert_eq!(ErrorCategory::Royalty.as_str(), "royalty");
        assert_eq!(ErrorCategory::Storage.as_str(), "storage");
        assert_eq!(ErrorCategory::Security.as_str(), "security");
        assert_eq!(ErrorCategory::Configuration.as_str(), "configuration");
    }

    #[test]
    fn every_category_is_documented() {
        for category in ALL_CATEGORIES {
            assert!(!category.description().is_empty());
        }
    }

    #[test]
    fn every_category_has_at_least_one_error_code() {
        for category in ALL_CATEGORIES {
            assert!(
                !category.error_codes().is_empty(),
                "{} has no assigned codes",
                category.as_str()
            );
        }
    }

    #[test]
    fn categorize_by_code_primary_mappings() {
        assert_eq!(categorize_by_code(220), Some(ErrorCategory::Validation));
        assert_eq!(categorize_by_code(240), Some(ErrorCategory::Validation));
        assert_eq!(categorize_by_code(230), Some(ErrorCategory::Storage));
        assert_eq!(categorize_by_code(231), Some(ErrorCategory::Ownership));
        assert_eq!(categorize_by_code(211), Some(ErrorCategory::Configuration));
        assert_eq!(categorize_by_code(242), Some(ErrorCategory::Security));
        assert_eq!(categorize_by_code(250), Some(ErrorCategory::Security));
    }

    #[test]
    fn payment_errors_are_categorized_as_payment() {
        // InsufficientPayment (#1399).
        assert_eq!(categorize_by_code(260), Some(ErrorCategory::Payment));
        // UnsupportedAsset (#1400).
        assert_eq!(categorize_by_code(262), Some(ErrorCategory::Payment));
        assert!(ErrorCategory::Payment.error_codes().contains(&260));
        assert!(ErrorCategory::Payment.error_codes().contains(&262));
    fn new_transfer_and_approval_codes_are_categorized() {
        // InvalidRecipient (294–296) → Validation
        for code in 294..=296 {
            assert_eq!(categorize_by_code(code), Some(ErrorCategory::Validation));
        }
        // ApprovalAlreadyExists (297) → Authorization
        assert_eq!(categorize_by_code(297), Some(ErrorCategory::Authorization));
        // InvalidTransferState reasons (298–302) → Security
        for code in 298..=302 {
            assert_eq!(categorize_by_code(code), Some(ErrorCategory::Security));
    fn batch_transfer_codes_are_validation() {
        for code in 294..=299 {
            assert_eq!(categorize_by_code(code), Some(ErrorCategory::Validation));
            assert!(
                ErrorCategory::Validation.error_codes().contains(&code),
                "validation category is missing batch transfer code {code}"
            );
        }
    }

    #[test]
    fn listing_codes_are_marketplace() {
        for code in 300..=302 {
            assert_eq!(categorize_by_code(code), Some(ErrorCategory::Marketplace));
            assert!(
                ErrorCategory::Marketplace.error_codes().contains(&code),
                "marketplace category is missing listing code {code}"
            );
        }
    }

    #[test]
    fn unknown_codes_are_not_categorized() {
        assert_eq!(categorize_by_code(0), None);
        assert_eq!(categorize_by_code(9998), None);
        assert_eq!(categorize_by_code(i32::MAX as u32), None);
    }

    #[test]
    fn configuration_codes_are_classified_as_configuration() {
        for code in 210..=214u32 {
            assert_eq!(
                categorize_by_code(code),
                Some(ErrorCategory::Configuration),
                "code {code} should be Configuration"
            );
        }
    }

    #[test]
    fn validation_codes_are_classified_as_validation() {
        for code in 220..=225u32 {
            assert_eq!(
                categorize_by_code(code),
                Some(ErrorCategory::Validation),
                "code {code} should be Validation"
            );
        }
    }

    #[test]
    fn token_not_found_maps_to_storage() {
        assert_eq!(categorize_by_code(230), Some(ErrorCategory::Storage));
        assert!(ErrorCategory::Storage.error_codes().contains(&230));
    }

    #[test]
    fn every_assigned_code_has_a_primary_category() {
        for category in ALL_CATEGORIES {
            for &code in category.error_codes() {
                assert!(
                    categorize_by_code(code).is_some(),
                    "code {code} listed under {} has no primary category",
                    category.as_str()
                );
            }
        }
    }

    #[test]
    fn categories_for_code_reports_all_memberships() {
        let storage = categories_for_code(230);
        assert!(storage.contains(&ErrorCategory::Storage));

        let validation = categories_for_code(220);
        assert!(validation.contains(&ErrorCategory::Validation));

        assert!(categories_for_code(9998).is_empty());
    }

    #[test]
    fn category_codes_roundtrip() {
        for category in ALL_CATEGORIES {
            assert_eq!(ErrorCategory::from_code(category.code()), Some(category));
        }
        assert_eq!(ErrorCategory::from_code(9), None);
        assert_eq!(ErrorCategory::from_code(255), None);
    }
}
