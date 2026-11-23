pub use crate::error_infrastructure::ConfigurationError;
pub use crate::types::Error;

pub type StateError = Error;
pub type ApprovalError = Error;
pub type TransferError = Error;
pub type BatchTransferError = Error;

// Marketplace / payment errors (#1399, #1400)
//
// The central `Error` enum (defined in `crate::types`) is the single source of
// truth for these standardized errors. The variants below are documented here
// for reference and are expected to be present on `crate::types::Error`:
//
// - `Error::InsufficientPayment { expected: i128, provided: i128 }`
//     Returned when a buyer does not provide sufficient funds for an NFT
//     purchase. Carries the `expected` amount (and the `provided` amount) so
//     callers can surface the shortfall.
//
// - `Error::UnsupportedAsset { asset: Address }`
//     Returned when marketplace or royalty operations reference an unsupported
//     payment asset. Used across payment modules for standardized validation.
//
// Discriminants: `InsufficientPayment = 264`, `UnsupportedAsset = 265`
// (avoiding the 260–263 range used by RoyaltyValidationError and
// MarketplaceError).

pub type PaymentError = Error;
pub type AssetError = Error;
pub type ConfigError = Error;
