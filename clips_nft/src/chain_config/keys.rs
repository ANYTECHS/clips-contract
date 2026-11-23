use soroban_sdk::contracttype;

/// Centralized storage keys for all configuration entries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigKey {
    /// Global contract configuration parameters.
    GlobalConfig,
    /// Fee charged by the platform, in basis points (0–10_000).
    PlatformFee,
    /// Fee charged by the marketplace, in basis points (0–10_000).
    MarketplaceFee,
    /// Default metadata base URI prepended to token IDs. Issue #476.
    MetadataBaseUri,
    /// Wallet address that receives platform and protocol fees.
    TreasuryWallet,
    /// Fee charged on NFT minting operations.
    MintingFee,
    /// Fee charged on NFT burning/destruction operations.
    BurningFee,
    /// Maximum number of NFTs mintable in a single batch transaction.
    MaxBatchMintSize,
    /// Maximum total collection size / NFT capacity.
    MaxCollectionSize,
    /// Contract pause state flag.
    Paused,
    /// Default IPFS gateway URL.
    IpfsGateway,
    /// List of supported Stellar assets.
    SupportedAssets,
    /// List of supported payment currencies.
    SupportedCurrencies,
    /// Emergency administrator address.
    /// Maximum allowed metadata size in bytes. Issue #1139.
    MaxMetadataSize,
    /// Emergency administrator address. Issue #1135.
    EmergencyAdmin,
}
