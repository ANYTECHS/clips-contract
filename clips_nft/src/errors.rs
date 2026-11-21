pub use crate::types::Error;

pub type StateError = Error;
pub type ApprovalError = Error;
pub type TransferError = Error;

/// Standardized errors for invalid batch transfer operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchTransferError {
    /// The batch contained no tokens.
    EmptyBatch,
    /// The batch exceeded the maximum allowed size.
    OversizedBatch,
    /// The request was malformed or otherwise invalid.
    InvalidRequest,
    /// The batch contained a duplicate token.
    DuplicateToken,
    /// A recipient in the batch was invalid.
    InvalidRecipient,
    /// A transfer within the batch failed.
    FailedTransfer,
}

impl BatchTransferError {
    /// Returns the unique numeric code for this error.
    pub fn code(&self) -> u32 {
        match self {
            BatchTransferError::EmptyBatch => 1,
            BatchTransferError::OversizedBatch => 2,
            BatchTransferError::InvalidRequest => 3,
            BatchTransferError::DuplicateToken => 4,
            BatchTransferError::InvalidRecipient => 5,
            BatchTransferError::FailedTransfer => 6,
        }
    }

    /// Returns the human-readable name for this error.
    pub fn name(&self) -> &'static str {
        match self {
            BatchTransferError::EmptyBatch => "EmptyBatch",
            BatchTransferError::OversizedBatch => "OversizedBatch",
            BatchTransferError::InvalidRequest => "InvalidRequest",
            BatchTransferError::DuplicateToken => "DuplicateToken",
            BatchTransferError::InvalidRecipient => "InvalidRecipient",
            BatchTransferError::FailedTransfer => "FailedTransfer",
        }
    }

    /// Reconstructs an error from its numeric code.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(BatchTransferError::EmptyBatch),
            2 => Some(BatchTransferError::OversizedBatch),
            3 => Some(BatchTransferError::InvalidRequest),
            4 => Some(BatchTransferError::DuplicateToken),
            5 => Some(BatchTransferError::InvalidRecipient),
            6 => Some(BatchTransferError::FailedTransfer),
            _ => None,
        }
    }

    /// Helper constructor for an empty batch error.
    pub fn empty_batch() -> Self {
        BatchTransferError::EmptyBatch
    }

    /// Helper constructor for an oversized batch error.
    pub fn oversized_batch() -> Self {
        BatchTransferError::OversizedBatch
    }

    /// Helper constructor for an invalid request error.
    pub fn invalid_request() -> Self {
        BatchTransferError::InvalidRequest
    }

    /// Helper constructor for a duplicate token error.
    pub fn duplicate_token() -> Self {
        BatchTransferError::DuplicateToken
    }

    /// Helper constructor for an invalid recipient error.
    pub fn invalid_recipient() -> Self {
        BatchTransferError::InvalidRecipient
    }

    /// Helper constructor for a failed transfer error.
    pub fn failed_transfer() -> Self {
        BatchTransferError::FailedTransfer
    }
}

/// Marketplace-related standardized errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceError {
    /// The referenced listing does not exist.
    ListingNotFound,
    /// An active listing already exists for the NFT.
    ListingAlreadyExists,
    /// The listing has expired.
    ListingExpired,
}

impl MarketplaceError {
    /// Returns the unique numeric code for this error.
    pub fn code(&self) -> u32 {
        match self {
            MarketplaceError::ListingNotFound => 1,
            MarketplaceError::ListingAlreadyExists => 2,
            MarketplaceError::ListingExpired => 3,
        }
    }

    /// Returns the human-readable name for this error.
    pub fn name(&self) -> &'static str {
        match self {
            MarketplaceError::ListingNotFound => "ListingNotFound",
            MarketplaceError::ListingAlreadyExists => "ListingAlreadyExists",
            MarketplaceError::ListingExpired => "ListingExpired",
        }
    }

    /// Reconstructs an error from its numeric code.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(MarketplaceError::ListingNotFound),
            2 => Some(MarketplaceError::ListingAlreadyExists),
            3 => Some(MarketplaceError::ListingExpired),
            _ => None,
        }
    }

    /// Helper constructor for a listing-not-found error.
    pub fn listing_not_found() -> Self {
        MarketplaceError::ListingNotFound
    }

    /// Helper constructor for a listing-already-exists error.
    pub fn listing_already_exists() -> Self {
        MarketplaceError::ListingAlreadyExists
    }

    /// Helper constructor for a listing-expired error.
    pub fn listing_expired() -> Self {
        MarketplaceError::ListingExpired
    }
}

/// Re-exported marketplace error variants for use across marketplace functions.
pub use MarketplaceError::{
    ListingAlreadyExists, ListingExpired, ListingNotFound,
};