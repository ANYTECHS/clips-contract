use soroban_sdk::Env;

use super::keys::ConfigKey;

/// Maximum allowed marketplace fee: 10 % (1_000 basis points).
pub const MAX_MARKETPLACE_FEE_BPS: u32 = 1_000;

/// Returns the current marketplace fee in basis points, defaulting to 0.
pub fn get_marketplace_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&ConfigKey::MarketplaceFee)
        .unwrap_or(0)
}

/// Sets the marketplace fee.
///
/// # Errors
/// Returns `Err(())` if `fee_bps` exceeds [`MAX_MARKETPLACE_FEE_BPS`].
pub fn set_marketplace_fee(env: &Env, fee_bps: u32) -> Result<(), ()> {
    if fee_bps > MAX_MARKETPLACE_FEE_BPS {
        return Err(());
    }
    env.storage().instance().set(&ConfigKey::MarketplaceFee, &fee_bps);
    Ok(())
}
