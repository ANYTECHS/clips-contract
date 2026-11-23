//! Mint request validation domain.
//!
//! Validates every required field of an NFT mint request before the request is
//! accepted for minting:
//!
//! | Domain | Check | Failure error |
//! |--------|-------|---------------|
//! | Token data | clip id is non-zero and not already minted | `InvalidTokenId` / `ClipAlreadyMinted` |
//! | Creator information | creator address is valid (not the contract itself) | `InvalidAddress` |
//! | Metadata | metadata URI is present and uses a supported scheme | `InvalidURI` |
//! | Recipient information | owner/recipient address is valid and not blacklisted | `InvalidRecipient` / `Unauthorized` |
//!
//! All failures return a centralized [`crate::types::Error`] so callers can
//! propagate them directly.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::validators::minting::{validate_mint_request, MintRequestValidator};
//! use crate::validators::{ValidationContext, Validator};
//!
//! let ctx = ValidationContext::new(&env);
//! assert!(MintRequestValidator.validate(&ctx, &request).is_valid());
//! assert!(validate_mint_request(&env, &request).is_ok());
//! ```

pub mod request;

pub use request::{
    validate_creator, validate_metadata, validate_mint_request, validate_recipient,
    validate_token_data, MintRequestInput, MintRequestValidator,
};
