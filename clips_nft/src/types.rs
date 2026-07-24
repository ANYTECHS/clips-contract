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
    /// Backend signature has already been used for a prior mint.
    SignatureAlreadyUsed = 21,
    /// Token is already present in the wallet ownership index.
    DuplicateWalletEntry = 22,
}
