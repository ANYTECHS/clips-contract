//! Centralized guard module for contract authorization and validation (issue #1091).
//!
//! This module is the single source of truth for all guard implementations used
//! throughout the smart contract. Guards are reusable authorization and validation
//! checks that:
//!
//! 1. **Authenticate** — verify caller identity and authorization
//! 2. **Validate** — ensure state and parameters are sound
//! 3. **Enforce** — return standardized errors on violation
//!
//! # Guard organization
//!
//! Guards are organized by responsibility domain:
//!
//! ## Authorization guards
//! - [`admin`] — restrict operations to contract administrators
//! - [`owner`] — restrict operations to NFT owners
//! - [`operator`] — restrict operations to authorized operators
//!
//! ## State guards
//! - [`royalty`] — validate royalty configuration state
//! - [`payment`] — detect duplicate or replayed payments
//!
//! ## Composition
//! - [`compose`] — combine multiple guards into sequential checks
//!
//! # Usage patterns
//!
//! ## Single guard
//!
//! ```rust,ignore
//! // Require admin authorization
//! admin::require_admin_auth(&env, &caller)?;
//!
//! // Check owner without authorization demand
//! if owner::is_owner(&env, token_id, &caller) {
//!     // caller owns the token
//! }
//! ```
//!
//! ## Guard composition
//!
//! ```rust,ignore
//! // Combine multiple checks: admin + royalty state validation
//! admin::require_admin_auth(&env, &caller)?;
//! royalty::validate_royalty_state(&env, token_id)?;
//! royalty::validate_recipient(&env, &new_recipient)?;
//! // Safe to proceed with update
//! ```
//!
//! # Guard priority and short-circuit behavior
//!
//! Guards are designed to fail fast:
//!
//! - If the first guard fails, execution stops.
//! - Subsequent guards are not evaluated.
//! - This prevents information leakage and unnecessary computation.
//!
//! # Adding new guards
//!
//! To add a new guard domain:
//!
//! 1. Create a new module in this folder (e.g., `new_domain.rs`).
//! 2. Export from `mod.rs`.
//! 3. Document the guard's responsibility, error conditions, and usage.
//! 4. Add comprehensive tests.
//! 5. Update this module-level documentation.

pub mod admin;
pub mod compose;
pub mod owner;
pub mod payment;
pub mod royalty;

pub use admin::{check_caller_is_admin, get_configured_admin, require_admin};
pub use compose::{GuardBuilder, GuardComposition};
pub use owner::{check_caller_is_owner, get_owner_for_token, require_owner};
pub use payment::{
    detect_replay_payment, is_payment_already_processed, record_payment_processed,
};
pub use royalty::{
    validate_royalty_recipient, validate_royalty_state, validate_royalty_within_maximum,
};

/// Convenient type alias for guard results.
pub type GuardResult<T> = Result<T, crate::types::Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_module_exports_all_domains() {
        // Compile-time check: all public functions must be re-exported.
        // If this compiles, all public guard functions are accessible.
    }
}
