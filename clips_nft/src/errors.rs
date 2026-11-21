pub use crate::types::Error;

pub use crate::error_infrastructure::InitializationError;
pub use crate::error_catalog::{ErrorCategory, UnauthorizedOwnerError};

pub type StateError = Error;
pub type ApprovalError = Error;
pub type TransferError = Error;
pub type BatchTransferError = Error;
pub type InitializationResult = Result<(), InitializationError>;
pub type OwnershipResult = Result<(), UnauthorizedOwnerError>;