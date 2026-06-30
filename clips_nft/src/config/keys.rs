use soroban_sdk::contracttype;

/// Centralized storage keys for all configuration entries.
#[contracttype]
#[derive(Clone)]
pub enum ConfigKey {
    /// Fee charged by the platform, in basis points (0–10_000).
    PlatformFee,
    /// Fee charged by the marketplace, in basis points (0–10_000).
    MarketplaceFee,
    /// Emergency administrator wallet address. Issue #474.
    EmergencyAdmin,
}
