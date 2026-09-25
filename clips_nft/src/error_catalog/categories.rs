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
    /// Input did not pass validation (codes 220–225, 240–241).
    Validation,
    /// Caller lacked the required permission.
    Authorization,
    /// Caller did not own the referenced asset (code 231).
    Ownership,
    /// Payment or fee processing failed (code 213).
    Payment,
    /// Marketplace operation failed (code 213).
    Marketplace,
    /// Royalty configuration or payment failed (code 211).
    Royalty,
    /// Storage read/write failed (code 230).
    Storage,
    /// Security-sensitive restriction triggered (codes 242, 250).
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
            ErrorCategory::Validation => &[220, 221, 222, 223, 224, 225, 240, 241],
            ErrorCategory::Authorization => &[231],
            ErrorCategory::Ownership => &[231],
            ErrorCategory::Payment => &[213, 260, 262],
            ErrorCategory::Marketplace => &[260, 261, 262, 263],
            ErrorCategory::Royalty => &[211, 260],
            ErrorCategory::Storage => &[230],
            ErrorCategory::Security => &[242, 250],
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
        220..=225 | 240 | 241 => Some(ErrorCategory::Validation),
        200..=204 | 210..=214 => Some(ErrorCategory::Configuration),
        230 => Some(ErrorCategory::Storage),
        231 => Some(ErrorCategory::Ownership),
        242 | 250 => Some(ErrorCategory::Security),
        _ => None,
    }
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
    fn unknown_codes_are_not_categorized() {
        assert_eq!(categorize_by_code(0), None);
        assert_eq!(categorize_by_code(9998), None);
        assert_eq!(categorize_by_code(i32::MAX as u32), None);
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
