use soroban_sdk::{symbol_short, Address, Env};

use crate::{config_guard, types::{DataKey, Error, RoyaltyPaymentsDisabledEvent}};

pub fn set_payments_disabled(
    env: &Env,
    caller: &Address,
    disabled: bool,
) -> Result<(), Error> {
    config_guard::require_config_admin(env, caller)?;
    env.storage()
        .instance()
        .set(&DataKey::RoyaltyPaymentsDisabled, &disabled);
    env.events().publish(
        (symbol_short!("ryl_emg"),),
        RoyaltyPaymentsDisabledEvent {
            disabled,
            caller: caller.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
    Ok(())
}

pub fn is_payments_disabled(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::RoyaltyPaymentsDisabled)
        .unwrap_or(false)
}

pub fn require_payments_enabled(env: &Env) -> Result<(), Error> {
    if is_payments_disabled(env) {
        return Err(Error::RoyaltyPaymentsDisabled);
    }
    Ok(())
}