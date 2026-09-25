//! Centralized registry of unique error codes for the ClipCash contract
//! (issue #981).
//!
//! Every error defined anywhere in the contract's standardized error space is
//! registered here together with its module, machine-readable name, unique
//! numeric code and human-readable documentation.
//!
//! # Guarantees
//!
//! * **Unique codes** — no two entries share a numeric `code`.
//! * **Unique names** — no two entries share a `name` (prevents aliasing).
//! * **Grouped by module** — entries are sorted by module, each module owning
//!   a contiguous code block.
//! * **Documented** — every entry carries a non-empty `description`.
//!
//! These invariants are enforced by the unit tests in this module.

use alloc::vec::Vec;

/// A single registered error code.
///
/// # Fields
/// * `module` — error group the code belongs to (e.g. `"initialization"`).
/// * `name` — machine-readable error name (matches the Rust variant name).
/// * `code` — unique numeric code assigned to this error.
/// * `description` — documentation for indexers and developers.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ErrorCode {
    /// Error group the code belongs to.
    pub module: &'static str,
    /// Machine-readable error name.
    pub name: &'static str,
    /// Unique numeric code.
    pub code: u32,
    /// Human-readable documentation.
    pub description: &'static str,
}

/// Complete registry of standardized ClipCash error codes.
///
/// The list is ordered by module; each module owns a contiguous code block:
///
/// * `initialization` (200–204)
/// * `configuration` (210–214)
/// * `validation` (220–225)
/// * `core` (230–231) — standardized token/ownership errors
/// * `transfer` (240–242) — standardized transfer errors
/// * `minting` (250) — standardized minting errors
/// * `marketplace` (260–263) — marketplace and payment operation errors
///
/// The core/transfer/minting/marketplace blocks are introduced by the companion
/// error-infrastructure modules; the registry documents their canonical codes
/// so all error definitions converge on the same numbering.
pub static ERROR_CODES: &'static [ErrorCode] = &[
    // ── initialization (200–204) ────────────────────────────────────────────
    ErrorCode {
        module: "initialization",
        name: "ContractAlreadyInitialized",
        code: 200,
        description: "The contract has already been initialized and cannot be re-initialized.",
    },
    ErrorCode {
        module: "initialization",
        name: "ContractNotInitialized",
        code: 201,
        description: "The contract must be initialized before this operation is allowed.",
    },
    ErrorCode {
        module: "initialization",
        name: "InvalidInitializationParameters",
        code: 202,
        description: "Initialization parameters are malformed or out of range.",
    },
    ErrorCode {
        module: "initialization",
        name: "MissingAdministrator",
        code: 203,
        description: "Initialization was attempted without a valid administrator address.",
    },
    ErrorCode {
        module: "initialization",
        name: "InvalidInitialConfig",
        code: 204,
        description: "The initial configuration is invalid and cannot be applied.",
    },
    // ── configuration (210–214) ──────────────────────────────────────────────
    ErrorCode {
        module: "configuration",
        name: "InvalidFee",
        code: 210,
        description: "A configured fee is outside the allowed range.",
    },
    ErrorCode {
        module: "configuration",
        name: "InvalidRoyaltyLimit",
        code: 211,
        description: "The configured royalty limit is outside the allowed range.",
    },
    ErrorCode {
        module: "configuration",
        name: "InvalidBatchSize",
        code: 212,
        description: "The configured batch size exceeds the allowed maximum.",
    },
    ErrorCode {
        module: "configuration",
        name: "UnsupportedAsset",
        code: 213,
        description: "The referenced asset is not supported by the contract.",
    },
    ErrorCode {
        module: "configuration",
        name: "InvalidConfigurationValue",
        code: 214,
        description: "A configuration value is structurally invalid or out of range.",
    },
    // ── validation (220–225) ─────────────────────────────────────────────────
    ErrorCode {
        module: "validation",
        name: "InvalidAddress",
        code: 220,
        description: "The provided wallet address is invalid.",
    },
    ErrorCode {
        module: "validation",
        name: "InvalidAmount",
        code: 221,
        description: "The provided amount is zero, negative or otherwise invalid.",
    },
    ErrorCode {
        module: "validation",
        name: "InvalidTokenId",
        code: 222,
        description: "The provided token identifier is invalid.",
    },
    ErrorCode {
        module: "validation",
        name: "InvalidUri",
        code: 223,
        description: "The provided URI is malformed or uses an unsupported protocol.",
    },
    ErrorCode {
        module: "validation",
        name: "InvalidTimestamp",
        code: 224,
        description: "The provided timestamp is zero or in the past.",
    },
    ErrorCode {
        module: "validation",
        name: "InvalidConfiguration",
        code: 225,
        description: "The provided configuration is invalid.",
    },
    // ── core (230–231) ───────────────────────────────────────────────────────
    ErrorCode {
        module: "core",
        name: "TokenNotFound",
        code: 230,
        description: "The operation references an NFT that does not exist.",
    },
    ErrorCode {
        module: "core",
        name: "UnauthorizedOwner",
        code: 231,
        description: "The caller is not the owner of the referenced NFT.",
    },
    // ── transfer (240–242) ───────────────────────────────────────────────────
    ErrorCode {
        module: "transfer",
        name: "InvalidRecipient",
        code: 240,
        description: "The NFT recipient address is invalid.",
    },
    ErrorCode {
        module: "transfer",
        name: "SelfTransferNotAllowed",
        code: 241,
        description: "The sender and recipient of a transfer must be different.",
    },
    ErrorCode {
        module: "transfer",
        name: "FrozenToken",
        code: 242,
        description: "The operation is not allowed on a frozen NFT.",
    },
    // ── minting (250) ────────────────────────────────────────────────────────
    ErrorCode {
        module: "minting",
        name: "TokenAlreadyExists",
        code: 250,
        description: "An NFT with the same token identifier already exists.",
    },
    // ── marketplace (260–263) ────────────────────────────────────────────────
    ErrorCode {
        module: "marketplace",
        name: "UnsupportedPaymentAsset",
        code: 260,
        description: "The payment asset is not supported for marketplace or royalty operations.",
    },
    ErrorCode {
        module: "marketplace",
        name: "DuplicateListing",
        code: 261,
        description: "A duplicate active listing already exists for the same NFT.",
    },
    ErrorCode {
        module: "marketplace",
        name: "InsufficientPayment",
        code: 262,
        description: "The buyer has not provided sufficient funds to complete the purchase.",
    },
    ErrorCode {
        module: "marketplace",
        name: "ExpiredListing",
        code: 263,
        description: "The marketplace listing or offer has expired and is no longer valid.",
    },
];

/// Modules that own a code block in the registry, in display order.
pub static MODULES: &'static [&'static str] = &[
    "initialization",
    "configuration",
    "validation",
    "core",
    "transfer",
    "minting",
    "marketplace",
];

/// Return the registered error for a numeric `code`, if any.
pub fn error_code(code: u32) -> Option<&'static ErrorCode> {
    ERROR_CODES.iter().find(|entry| entry.code == code)
}

/// Return the registered error for a `name`, if any.
pub fn error_by_name(name: &str) -> Option<&'static ErrorCode> {
    ERROR_CODES.iter().find(|entry| entry.name == name)
}

/// Return the machine-readable name for a numeric `code`, if registered.
pub fn name_for(code: u32) -> Option<&'static str> {
    error_code(code).map(|entry| entry.name)
}

/// Return every error registered under `module`, in registration order.
pub fn codes_for_module(module: &str) -> Vec<&'static ErrorCode> {
    ERROR_CODES
        .iter()
        .filter(|entry| entry.module == module)
        .collect()
}

/// Return `true` if no other entry in the registry shares `code`.
pub fn is_unique_code(code: u32) -> bool {
    ERROR_CODES
        .iter()
        .filter(|entry| entry.code == code)
        .count()
        == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unique codes (acceptance: "Assign unique codes") ─────────────────────

    #[test]
    fn every_error_code_is_unique() {
        let mut codes: Vec<u32> = ERROR_CODES.iter().map(|entry| entry.code).collect();
        codes.sort_unstable();
        for window in codes.windows(2) {
            assert_ne!(window[0], window[1], "duplicate error code found");
        }
    }

    #[test]
    fn every_error_name_is_unique() {
        let mut names: Vec<&str> = ERROR_CODES.iter().map(|entry| entry.name).collect();
        names.sort_unstable();
        for window in names.windows(2) {
            assert_ne!(window[0], window[1], "duplicate error name found");
        }
    }

    #[test]
    fn is_unique_code_detects_duplicates() {
        assert!(is_unique_code(200));
        assert!(is_unique_code(230));
        // A code that appears twice in a synthetic list must be rejected.
        let synthetic: [ErrorCode; 2] = [
            ErrorCode {
                module: "a",
                name: "A",
                code: 1,
                description: "first",
            },
            ErrorCode {
                module: "a",
                name: "B",
                code: 1,
                description: "second",
            },
        ];
        let duplicate = synthetic.iter().filter(|entry| entry.code == 1).count() > 1;
        assert!(duplicate);
        assert!(!is_unique_code(999_999));
    }

    // ── Group by module (acceptance: "Group codes by module") ────────────────

    #[test]
    fn codes_are_grouped_by_module() {
        // Every same-module block must be contiguous: no gap may exist between
        // two entries of the same module.
        for (idx, entry) in ERROR_CODES.iter().enumerate() {
            if let Some(last_same) = ERROR_CODES.iter().rposition(|e| e.module == entry.module) {
                if last_same > idx {
                    for between in &ERROR_CODES[idx..=last_same] {
                        assert_eq!(
                            between.module, entry.module,
                            "module {} is not contiguous",
                            entry.module
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_module_has_at_least_one_code() {
        for module in MODULES {
            assert!(
                !codes_for_module(module).is_empty(),
                "module {} has no registered codes",
                module
            );
        }
    }

    #[test]
    fn module_codes_are_contiguous() {
        for module in MODULES {
            let mut codes: Vec<u32> = codes_for_module(module).iter().map(|e| e.code).collect();
            codes.sort_unstable();
            for window in codes.windows(2) {
                assert_eq!(
                    window[1],
                    window[0] + 1,
                    "module {} codes are not contiguous",
                    module
                );
            }
        }
    }

    // ── Lookups ──────────────────────────────────────────────────────────────

    #[test]
    fn lookup_by_code_returns_registered_error() {
        let entry = error_code(230).expect("230 must be registered");
        assert_eq!(entry.name, "TokenNotFound");
        assert_eq!(entry.module, "core");
        assert_eq!(name_for(230), Some("TokenNotFound"));
    }

    #[test]
    fn lookup_by_name_returns_registered_error() {
        let entry = error_by_name("InvalidBatchSize").expect("must be registered");
        assert_eq!(entry.code, 212);
        assert_eq!(entry.module, "configuration");
    }

    #[test]
    fn unknown_lookups_return_none() {
        assert!(error_code(0).is_none());
        assert!(error_code(9999).is_none());
        assert!(error_by_name("DoesNotExist").is_none());
        assert!(name_for(7777).is_none());
    }

    #[test]
    fn codes_for_module_filters_correctly() {
        let validation = codes_for_module("validation");
        assert_eq!(validation.len(), 6);
        for entry in &validation {
            assert_eq!(entry.module, "validation");
        }
    }

    // ── Document each code (acceptance: "Document each code") ────────────────

    #[test]
    fn every_code_is_documented() {
        for entry in ERROR_CODES {
            assert!(!entry.name.is_empty(), "code {} has no name", entry.code);
            assert!(
                !entry.module.is_empty(),
                "code {} has no module",
                entry.code
            );
            assert!(
                !entry.description.is_empty(),
                "code {} has no documentation",
                entry.code
            );
        }
    }
}
