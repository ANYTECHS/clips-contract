use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Vec};

pub type TokenId = u32;

#[contracttype]
#[derive(Clone)]
pub struct TokenData {
    pub owner: Address,
    pub clip_id: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Royalty {
    pub recipient: Address,
    pub basis_points: u32,
    pub asset_address: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltyRecipient {
    pub recipient: Address,
    pub basis_points: u32,
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

/// Rich event emitted immediately after a successful NFT mint.
///
/// Carries every field an indexer, wallet, or marketplace needs to track
/// a newly created ClipCash NFT without any additional storage reads.
///
/// # Fields
/// - `token_id`     — On-chain token identifier assigned during this mint.
/// - `clip_id`      — Off-chain video-clip identifier linked to the token.
/// - `creator`      — Address of the clip creator (may differ from owner on
///                    secondary mints or gifted tokens).
/// - `owner`        — Address that received ownership of the token.
/// - `metadata_uri` — URI pointing to the token's metadata JSON.
/// - `timestamp`    — Ledger timestamp (seconds since Unix epoch) at mint time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTMintedEvent {
    /// Newly assigned on-chain token ID.
    pub token_id: TokenId,
    /// Off-chain clip identifier (unique per collection).
    pub clip_id: u32,
    /// Address of the original clip creator.
    pub creator: Address,
    /// Address of the initial token owner.
    pub owner: Address,
    /// Metadata URI stored for this token (IPFS, Arweave, or HTTPS).
    pub metadata_uri: String,
    /// Ledger timestamp in seconds since the Unix epoch.
    pub timestamp: u64,
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

/// Event emitted when royalty information is successfully assigned during minting (issue #695).
///
/// Carries every field an indexer needs to track royalty configuration at
/// mint time, without requiring additional storage reads.
///
/// # Fields
/// - `token_id`     — On-chain token identifier the royalty is assigned to.
/// - `recipient`    — Address that will receive royalty payments.
/// - `basis_points` — Royalty percentage in basis points (100 bps = 1 %).
/// - `timestamp`    — Ledger timestamp (seconds since Unix epoch) when assigned.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyAssignedEvent {
    /// On-chain token ID the royalty is assigned to.
    pub token_id: TokenId,
    /// Address that will receive royalty payments on secondary sales.
    pub recipient: Address,
    /// Royalty percentage in basis points (0–10 000).
    pub basis_points: u32,
    /// Ledger timestamp in seconds since the Unix epoch.
    pub timestamp: u64,
}

/// Event emitted when a creator is assigned to a newly minted NFT.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorAssignedEvent {
    pub token_id: TokenId,
    pub creator: Address,
    pub clip_id: u32,
    pub timestamp: u64,
}

/// Status of a mint transaction returned to callers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failed,
}

/// Standardized response returned after a successful NFT mint.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintSuccessResponse {
    pub token_id: TokenId,
    pub owner: Address,
    pub metadata_uri: String,
    pub clip_id: u32,
    pub mint_timestamp: u64,
    pub status: TransactionStatus,
}

/// Standardized response returned after a successful NFT transfer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferResult {
    pub token_id: TokenId,
    pub previous_owner: Address,
    pub new_owner: Address,
    pub transfer_timestamp: u64,
    pub status: TransactionStatus,
}

/// Type alias used for batch identifiers — monotonically increasing counter
/// assigned to every invocation of `execute_batch_mint`.  Batch IDs are
/// never re-used even across failed batches.
pub type BatchId = u64;

/// Event emitted once after a fully successful batch mint operation (issue #697).
///
/// Summarises the outcome of an `execute_batch_mint` call into a single,
/// indexed event so off-chain systems can track batch completions without
/// scanning individual per-token mint events.
///
/// # Fields
/// - `batch_id`      — Monotonically increasing identifier for the batch.
/// - `minted_count`  — Number of NFTs successfully created in this batch.
/// - `recipient`     — Address that received ownership of all minted tokens.
/// - `timestamp`     — Ledger timestamp (seconds since Unix epoch) when the
///                     batch completed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchMintCompletedEvent {
    /// Monotonically increasing identifier assigned to this batch.
    pub batch_id: BatchId,
    /// Number of NFTs minted in this batch.
    pub minted_count: u32,
    /// Address that received ownership of all minted tokens.
    pub recipient: Address,
    /// Ledger timestamp in seconds since the Unix epoch.
    pub timestamp: u64,
}

/// Reusable response object returned after every batch mint operation.
///
/// Aggregates the outcome of a `BatchMintRequest` into a single struct with
/// enough information for off-chain indexers and clients to reconcile state
/// without re-scanning storage.
///
/// # Fields (per acceptance criteria)
/// * `batch_id`          — monotonically increasing identifier for this invocation
/// * `minted_token_ids`  — on-chain token IDs of every successfully minted NFT
/// * `success_count`     — number of NFTs created (matches `minted_token_ids.len()`)
/// * `failure_count`     — number of NFTs that failed (0 on atomic-all-or-nothing
///                         current implementation; reserved for future partial modes)
/// * `processed_at`      — ledger timestamp when the batch completed
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchMintResponse {
    pub batch_id: BatchId,
    pub minted_token_ids: Vec<TokenId>,
    pub success_count: u32,
    pub failure_count: u32,
    pub processed_at: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    NextTokenId,
    /// Monotonically increasing batch identifier bumped on every
    /// `execute_batch_mint` invocation (even failed ones).
    NextBatchId,
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
    /// Maps metadata URI to token ID to prevent duplicate metadata.
    MetadataIndex(String),
    /// Approved minter address (single minter role).
    ApprovedMinter(Address),

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
    /// Per-address nonce counter for signature replay prevention.
    Nonce(Address),

    // ── Minting fields (issues #665, #668, #669, #672) ────────────────────────
    /// Thumbnail image URI associated with a minted NFT (issue #668).
    ThumbnailUri(TokenId),
    /// Preview video URI associated with a minted NFT (issue #669).
    PreviewVideoUri(TokenId),
    /// Legacy thumbnail URI key alias.
    Thumbnail(TokenId),
    /// Legacy preview URI key alias.
    PreviewUri(TokenId),

    // ── Minting storage tasks (issues #673–#676) ───────────────────────────────
    /// Per-token royalty percentage in basis points (issue #673).
    RoyaltyPercentage(TokenId),
    /// Portfolio index of tokens created by a creator (issue #674).
    CreatorTokens(Address),
    /// Portfolio index of tokens owned by an address (issue #675).
    OwnerTokens(Address),
    /// Collection a token is associated with (issue #676).
    TokenCollection(TokenId),
    /// Existence marker for a registered collection (issue #676).
    CollectionRegistered(u32),
    /// Membership list of tokens in a collection (issue #676).
    CollectionMembers(u32),

    // ── Minting royalty / metadata tasks (issues #666, #667, #670, #671) ───────
    /// Registered metadata record existence marker keyed by URI (issue #666).
    MetadataRecord(String),

    // ── Token counter (issue #504) ────────────────────────────────────────────
    /// Total number of NFTs minted (monotonically increasing counter).
    TokenCounter,
    // ── Token ownership (issue #505) ──────────────────────────────────────────
    /// Direct owner address for a token (dedicated ownership record).
    TokenOwner(TokenId),
    /// Persistent storage key for checking multiple administrators (issue #494).
    Administrator(Address),

    // ── Royalty recipient index (issue #785) ──────────────────────────────────
    /// Ordered list of token IDs whose royalty is assigned to this recipient.
    RecipientTokens(Address),
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
    /// The referenced collection has not been registered (issue #676).
    CollectionNotFound = 40,
    /// Royalty recipient is not a valid Stellar wallet address (issue #671).
    InvalidRecipient = 41,
    /// Referenced metadata record does not exist (issue #666).
    MetadataNotFound = 42,
    /// Caller is not an approved minter.
    UnauthorizedMinter = 43,
    /// Duplicate metadata URI detected.
    DuplicateMetadata = 44,
    /// Duplicate entry in wallet token index.
    DuplicateWalletEntry = 45,
    /// Ed25519 signature has already been used (replay protection).
    SignatureAlreadyUsed = 46,
    /// Number of mint requests in batch exceeds the configured limit.
    BatchLimitExceeded = 47,

    SupplyOverflow = 48,
    /// Sender and recipient must be different wallets for a transfer.
    SelfTransferNotAllowed = 49,
}
