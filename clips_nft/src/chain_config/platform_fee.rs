use soroban_sdk::Env;

use super::keys::ConfigKey;

/// Maximum allowed platform fee: 10 % (1_000 basis points).
pub const MAX_PLATFORM_FEE_BPS: u32 = 1_000;

/// Returns the current platform fee in basis points, defaulting to 0.
pub fn get_platform_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&ConfigKey::PlatformFee)
        .unwrap_or(0)
}

/// Sets the platform fee.
///
/// # Errors
/// Returns `Err(())` if `fee_bps` exceeds [`MAX_PLATFORM_FEE_BPS`].
pub fn set_platform_fee(env: &Env, fee_bps: u32) -> Result<(), ()> {
    if fee_bps > MAX_PLATFORM_FEE_BPS {
        return Err(());
    }
    env.storage()
        .instance()
        .set(&ConfigKey::PlatformFee, &fee_bps);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_fee_get_and_set() {
        let env = Env::default();
        assert_eq!(get_platform_fee(&env), 0);

        assert!(set_platform_fee(&env, 500).is_ok());
        assert_eq!(get_platform_fee(&env), 500);

        assert!(set_platform_fee(&env, MAX_PLATFORM_FEE_BPS).is_ok());
        assert_eq!(get_platform_fee(&env), MAX_PLATFORM_FEE_BPS);

        assert!(set_platform_fee(&env, MAX_PLATFORM_FEE_BPS + 1).is_err());
    }
}
