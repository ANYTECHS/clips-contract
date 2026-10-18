//! NFT transfer recipient validation.
//!
//! Soroban's [`Address`] type guarantees addresses are structurally valid. This
//! module enforces the contract's recipient policy before a transfer changes
//! ownership: the recipient cannot be this contract, cannot be blacklisted,
//! and (for transfers) must differ from the current owner.

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::types::Error;

/// Validate an address as an eligible NFT recipient.
///
/// Returns [`Error::InvalidRecipient`] for the contract's own address and
/// [`Error::Unauthorized`] for a blacklisted address. Malformed addresses
/// cannot be constructed through Soroban's typed [`Address`] API.
pub fn validate_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    if *recipient == env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }
    if blacklist::is_blacklisted(env, recipient) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
/// Validate a transfer recipient and reject transfers to the current owner.
pub fn validate_transfer_recipient(
    env: &Env,
    current_owner: &Address,
    recipient: &Address,
) -> Result<(), Error> {
    validate_recipient(env, recipient)?;
    if current_owner == recipient {
        return Err(Error::InvalidRecipient);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn accepts_valid_recipient() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert_eq!(validate_recipient(env, &recipient), Ok(()));
        });
    }

    #[test]
    fn rejects_contract_as_recipient() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                validate_recipient(env, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn rejects_blacklisted_recipient() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            blacklist::add_wallet(env, &recipient);
            assert_eq!(
                validate_recipient(env, &recipient),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn rejects_self_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(
                validate_transfer_recipient(env, &owner, &owner),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn accepts_transfer_to_distinct_valid_recipient() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let recipient = Address::generate(env);
            assert_eq!(validate_transfer_recipient(env, &owner, &recipient), Ok(()));
        });
    }
}
