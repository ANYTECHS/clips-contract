//! Authorization result structure (issue #1245).
//!
//! A consistent representation for successful and failed authorization
//! checks, integrated with the centralized contract [`Error`](crate::types::Error):
//!
//! * success → [`AuthorizationResult::Allowed`]
//! * failure → [`AuthorizationResult::Denied(Error)`] carrying the exact
//!   contract error (usually `Error::Unauthorized`)
//!
//! The type converts both ways with `Result<(), Error>` so guards compose
//! with `?` and entry points stay on the standard error type.

use crate::types::Error;

/// Outcome of an authorization check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationResult {
    /// Caller is authorized; the operation may proceed.
    Allowed,
    /// Caller is not authorized; the operation must be rejected.
    Denied(Error),
}

/// Machine-readable authorization decision for indexers and tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationDecision {
    Allow,
    Deny,
}

impl AuthorizationResult {
    /// `true` when the check passed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, AuthorizationResult::Allowed)
    }

    /// `true` when the check failed.
    pub fn is_denied(&self) -> bool {
        !self.is_allowed()
    }

    /// The denial error, or `None` when allowed.
    pub fn error(&self) -> Option<Error> {
        match self {
            AuthorizationResult::Allowed => None,
            AuthorizationResult::Denied(err) => Some(*err),
        }
    }

    /// Coarse decision without the error payload.
    pub fn decision(&self) -> AuthorizationDecision {
        match self {
            AuthorizationResult::Allowed => AuthorizationDecision::Allow,
            AuthorizationResult::Denied(_) => AuthorizationDecision::Deny,
        }
    }

    /// Convert into the contract result type (`Allowed` → `Ok(())`).
    pub fn into_contract_result(self) -> Result<(), Error> {
        match self {
            AuthorizationResult::Allowed => Ok(()),
            AuthorizationResult::Denied(err) => Err(err),
        }
    }
}

impl From<AuthorizationResult> for Result<(), Error> {
    fn from(result: AuthorizationResult) -> Self {
        result.into_contract_result()
    }
}

impl From<Result<(), Error>> for AuthorizationResult {
    fn from(result: Result<(), Error>) -> Self {
        match result {
            Ok(()) => AuthorizationResult::Allowed,
            Err(err) => AuthorizationResult::Denied(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_converts_to_ok() {
        let result = AuthorizationResult::Allowed;
        assert!(result.is_allowed());
        assert!(!result.is_denied());
        assert_eq!(result.error(), None);
        assert_eq!(result.decision(), AuthorizationDecision::Allow);
        assert_eq!(result.into_contract_result(), Ok(()));
    }

    #[test]
    fn denied_carries_unauthorized_error() {
        let result = AuthorizationResult::Denied(Error::Unauthorized);
        assert!(!result.is_allowed());
        assert!(result.is_denied());
        assert_eq!(result.error(), Some(Error::Unauthorized));
        assert_eq!(result.decision(), AuthorizationDecision::Deny);
        assert_eq!(result.into_contract_result(), Err(Error::Unauthorized));
    }

    #[test]
    fn denied_preserves_non_unauthorized_errors() {
        for err in [Error::TokenNotFound, Error::InvalidAddress] {
            let result = AuthorizationResult::Denied(err);
            assert_eq!(result.into_contract_result(), Err(err));
        }
    }

    #[test]
    fn round_trips_through_result() {
        let ok: AuthorizationResult = Ok(()).into();
        assert_eq!(ok, AuthorizationResult::Allowed);
        let denied: AuthorizationResult = Err(Error::Unauthorized).into();
        assert_eq!(denied, AuthorizationResult::Denied(Error::Unauthorized));
        let back: Result<(), Error> = denied.into();
        assert_eq!(back, Err(Error::Unauthorized));
    }
}
