use soroban_sdk::{contracterror, contracttype, Address, String};

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
    Token(u32),
    Metadata(u32),
    Royalty(u32),
    ClipIdMinted(u32),
    PlatformFee,
    DefaultRoyaltyBps,
    Config,
    /// List of supported payment currency addresses.
    SupportedCurrencies,
    /// Migration version and upgrade timestamp metadata.
    ContractVersion,
    /// Per-token approval address.
    Approval(u32),
    /// Operator approval for all tokens of an owner.
    OperatorApproval(Address, Address),
    /// Wallet blacklist flag.
    Blacklisted(Address),
    /// Accumulated platform revenue.
    PlatformRevenue,
    /// Per-collection supply counter.
    CollectionSupply(u32),
    /// Royalty payment history per token.
    RoyaltyHistory(u32),
    /// Separate royalty recipient lookup.
    RoyaltyRecipient(u32),
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
    // ── Configuration Errors (Issue #486) ────────────────────────────────
    /// Fee value is outside the allowed range.
    InvalidFee = 11,
    /// Address is invalid or empty.
    InvalidAddress = 12,
    /// Metadata URI is empty or malformed.
    InvalidURI = 13,
    /// Collection limit is zero or exceeds the maximum.
    InvalidLimit = 14,
    /// Caller is not authorized to update configuration.
    UnauthorizedConfigurationUpdate = 15,
    /// Currency already exists in the supported list.
    DuplicateCurrency = 16,
    /// Currency not found in the supported list.
    CurrencyNotFound = 17,
    /// Configuration value is outside the allowed range.
    InvalidConfig = 18,
    /// Stored data failed validation or is malformed.
    CorruptedStorage = 19,
}
