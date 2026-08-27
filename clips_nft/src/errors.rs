use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    TokenNotFound = 4,
    AlreadyMinted = 5,
    InvalidBasisPoints = 6,
    InvalidRecipient = 7,
    Paused = 8,
    NotPaused = 9,
    MintCooldown = 10,
    /// Returned when an attempt is made to mint a clip that has already been
    /// minted. Each `clip_id` may only be minted once; subsequent mint calls
    /// for the same `clip_id` must return this error.
    ClipAlreadyMinted = 11,
    /// Returned when an invalid wallet address is provided.
    InvalidAddress = 12,
    /// Metadata URI is already associated with another NFT.
    DuplicateMetadata = 21,
    /// Caller is not authorized to mint NFTs.
    UnauthorizedMinter = 22,
    /// URL protocol is not supported (must be https://, ipfs://, or ar://).
    UnsupportedProtocol = 23,
    /// URL is malformed or invalid.
    MalformedUrl = 24,
    /// Metadata update not allowed
    MetadataUpdateNotAllowed = 27,
    /// Batch mint size exceeds the configured maximum limit.
    BatchLimitExceeded = 45,
    /// Batch transfer size exceeds the configured maximum limit.
    BatchTransferLimitExceeded = 46,
    /// Royalty configuration cannot be changed after it is frozen.
    RoyaltyFrozen = 47,
}
