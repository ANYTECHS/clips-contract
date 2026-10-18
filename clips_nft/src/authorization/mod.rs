//! Authorization module — consistent allow/deny outcomes for access checks.
//!
//! Centralizes how the contract represents authorization decisions so guards
//! never invent ad-hoc shapes:
//!
//! * [`result`] — `AuthorizationResult::{Allowed, Denied}` plus conversion
//!   into the centralized contract [`Error`](crate::types::Error).
//!
//! Helpers that *perform* the checks (caller/resource utilities) live in
//! [`helpers`] (issue #1246) and return this result type.

pub mod helpers;
pub mod result;

pub use helpers::{check_caller, check_owner_operator_or_admin, check_resource_owner};
pub use result::{AuthorizationDecision, AuthorizationResult};
