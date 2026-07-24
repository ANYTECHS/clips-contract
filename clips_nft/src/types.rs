use soroban_sdk::{contracterror, contracttype, Address, BytesN, String};

pub type TokenId = u32;

#[contracttype]
#[derive(Clone)]
pub struct TokenData {
    pub owner: Address,
    pub clip_id: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct Royalty {
    pub recipient: Address,
    pub basis_points: u32,
    pub asset_address: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltyInfo {
    pub receiver: Address,
    pub royalty_amount: i128,
    pub asset_address: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltyPayment {
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Minimal contract-wide config stored by the storage sub-module.
#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub max_royalty_bps: u32,
    pub mint_cooldown_secs: u64,
    pub platform_fee_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct MintEvent {
    pub to: Address,
    pub clip_id: u32,
    pub token_id: TokenId,
    pub metadata_uri: String,
}

#[contracttype]
#[derive(Clone)]
pub struct BurnEvent {
    pub owner: Address,
    pub token_id: TokenId,
}

/// Event emitted when NFT metadata is updated (Issue #563).
///
/// Includes the token ID, previous URI, new URI, and the updater address
/// so off-chain indexers can track every metadata change.
#[contracttype]
#[derive(Clone)]
pub struct MetadataUpdatedEvent {
    pub token_id: TokenId,
    pub previous_uri: String,
    pub new_uri: String,
    pub updater: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct TransferEvent {
    pub from: Address,
    pub to: Address,
    pub token_id: TokenId,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltyPaidEvent {
    pub token_id: TokenId,
    pub payer: Address,
    pub receiver: Address,
    pub amount: i128,
    pub asset_address: Option<Address>,
}

#[contracttype]
pub enum DataKey {
    Admin,
    NextTokenId,
    TotalSupply,
    Paused,
    Signer,
    Token(TokenId),
    Metadata(TokenId),
    Royalty(TokenId),
    /// Maps clip_id → token_id; also used as existence marker for a minted clip.
    ClipIdMinted(u32),
    PlatformFee,
    /// Treasury wallet that receives platform fees.
    PlatformRecipient,
    DefaultRoyaltyBps,
    Config,
    SupportedCurrencies,
    Blacklisted(Address),
    /// Single-token approval: address approved to transfer token_id.
    Approval(TokenId),
    /// Operator approval: (owner, operator) → approved.
    OperatorApproval(Address, Address),
    CollectionSupply(u32),
    /// Maps token_id → clip_id (reverse of ClipIdMinted).
    TokenClipId(TokenId),
    /// Existence marker for the minted-clip index (bool).
    ClipMinted(u32),
    /// AI-generated virality score for a token (issue #552).
    ViralityScore(u32),
    /// Originating social platform for a token (issue #553).
    SocialPlatform(u32),
    /// Original video source ID for a token (issue #554).
    VideoSourceId(u32),
    /// Original video source URL for a token (issue #554).
    VideoSourceUrl(u32),

    // ── Per-wallet index ──────────────────────────────────────────────────────
    /// Ordered list of token IDs owned by a wallet address.
    WalletTokens(Address),

    // ── Per-token auxiliary records ───────────────────────────────────────────
    /// Original creator address for a token.
    Creator(TokenId),
    /// Marks a token as non-transferable (soulbound / frozen).
    FrozenToken(TokenId),
    /// Historical royalty payment log for a token.
    RoyaltyHistory(TokenId),
    /// Standalone royalty recipient address (lightweight alternative to full Royalty struct).
    RoyaltyRecipient(TokenId),
    /// Metadata URI stored separately from the core token record.
    TokenUri(TokenId),
    /// Structured clip info metadata (clip title, description, etc.).
    ClipInfo(TokenId),
    /// Metadata refresh timestamps (creation + last update epoch seconds).
    MetadataTimestamps(TokenId),
    /// Flag indicating that the per-token metadata has been updated at least once.
    MetadataUpdated(TokenId),

    // ── Collection-level records ──────────────────────────────────────────────
    /// Human-readable collection name (e.g. "ClipCash Clips").
    Name,
    /// Collection ticker symbol (e.g. "CLIP").
    Symbol,
    /// Structured collection-level metadata blob.
    CollectionMetadata,

    // ── Contract-level bookkeeping ────────────────────────────────────────────
    /// Persisted contract version record (version + upgrade timestamp).
    ContractVersion,
    /// Per-event-type emission counter used for analytics.
    EventCounter(u32),
    /// Configurable maximum allowed size in bytes for a metadata URI.
    MaxMetadataSize,
    /// Global metadata schema version; bumped on schema migrations.
    MetadataVersion,
    /// Accumulated platform revenue (in the smallest asset unit).
    PlatformRevenue,
    /// Marks a backend signature hash as consumed to prevent replay.
    UsedSignature(BytesN<32>),
    /// Wallet ownership index: wallet → Vec<TokenId>.
    WalletTokens(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    NotPaused = 5,
    TokenNotFound = 6,
    ClipAlreadyMinted = 7,
    SignerNotSet = 8,
    InvalidSignature = 9,
    InvalidBasisPoints = 10,
    /// Fee value is outside the allowed range.
    InvalidFee = 11,
    InvalidAddress = 12,
    InvalidURI = 13,
    InvalidLimit = 14,
    UnauthorizedConfigurationUpdate = 15,
    DuplicateCurrency = 16,
    CurrencyNotFound = 17,
    /// Config values are out of range or structurally invalid.
    InvalidConfig = 18,
    /// Sale price must be positive.
    InvalidSalePrice = 19,
    /// Royalty amount calculation overflowed.
    RoyaltyOverflow = 20,
    /// Royalty basis points exceed the allowed maximum (10 000 = 100 %).
    RoyaltyTooHigh = 21,
    /// Mint cooldown period has not elapsed since the last mint.
    MintCooldown = 22,
    /// Storage record failed deserialization or integrity checks.
    CorruptedStorage = 23,
    /// A storage key is structurally invalid.
    InvalidStorageKey = 24,
    /// A storage conflict was detected (e.g. duplicate key in different namespace).
    StorageConflict = 25,
    /// Expected storage entry is missing.
    StorageNotFound = 26,
    /// Stored data could not be decoded into the expected type.
    MalformedData = 27,
    /// A URL or URI is malformed.
    MalformedUrl = 28,
    /// Record already exists where uniqueness is required.
    DuplicateRecord = 29,
    /// A URL protocol is not supported (e.g. non-IPFS/Arweave URI).
    UnsupportedProtocol = 30,
    /// Metadata title field is empty.
    EmptyTitle = 31,
    /// Metadata description field is empty.
    EmptyDescription = 32,
    /// Metadata image field is empty or invalid.
    InvalidImage = 33,
    /// Creator address is empty or missing.
    EmptyCreator = 34,
    /// Metadata has already been updated and cannot be updated again.
    MetadataAlreadyUpdated = 35,
    /// Metadata URI exceeds the configured maximum size limit.
    MetadataSizeTooLarge = 36,
    /// URI is invalid (alias for InvalidURI; used by some URI-validation modules).
    InvalidUri = 37,
    /// URL protocol is not supported (must be https://, ipfs://, or ar://).
    UnsupportedProtocol = 21,
    /// URL is malformed or invalid.
    MalformedUrl = 22,
}
