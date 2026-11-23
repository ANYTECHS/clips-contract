use soroban_sdk::contracttype;

/// Centralized storage keys for all configuration entries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigKey {
    /// Fee charged by the platform, in basis points (0–10_000).
    PlatformFee,
    /// Fee charged by the marketplace, in basis points (0–10_000).
    MarketplaceFee,
    /// Default metadata base URI prepended to token IDs. Issue #476.
    MetadataBaseUri,
    /// Maximum allowed metadata size in bytes. Issue #1139.
    MaxMetadataSize,
}
