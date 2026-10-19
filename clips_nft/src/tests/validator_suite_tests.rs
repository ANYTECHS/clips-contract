use crate::config_validator::validate_fee;
use crate::royalty_validator::{validate_royalty, validate_royalty_bps};
use crate::token_id_validator::{validate_token_id_format, validate_token_id_formats};
use crate::types::{Error, Royalty, RoyaltyRecipient};
use crate::validators::{standardize, validate_all};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn royalty_validator_accepts_boundaries_and_rejects_invalid_totals() {
    let env = Env::default();
    let recipient = Address::generate(&env);

    let maximum_royalty = Royalty {
        recipients: soroban_sdk::vec![
            &env,
            RoyaltyRecipient {
                recipient: recipient.clone(),
                basis_points: 10_000,
            }
        ],
        asset_address: None,
    };
    assert_eq!(validate_royalty(&maximum_royalty), Ok(()));
    assert_eq!(validate_royalty_bps(0), Ok(()));
    assert_eq!(validate_royalty_bps(10_000), Ok(()));
    assert_eq!(validate_royalty_bps(10_001), Err(Error::InvalidBasisPoints));

    let excessive_total = Royalty {
        recipients: soroban_sdk::vec![
            &env,
            RoyaltyRecipient {
                recipient: recipient.clone(),
                basis_points: 6_000,
            },
            RoyaltyRecipient {
                recipient,
                basis_points: 6_000,
            }
        ],
        asset_address: None,
    };
    assert_eq!(validate_royalty(&excessive_total), Err(Error::InvalidBasisPoints));
}

#[test]
fn token_id_validator_covers_sentinel_and_numeric_boundaries() {
    assert_eq!(validate_token_id_format(0), Err(Error::InvalidTokenId));
    assert_eq!(validate_token_id_format(1), Ok(()));
    assert_eq!(validate_token_id_format(u32::MAX), Ok(()));
    assert_eq!(validate_token_id_formats(&[1, u32::MAX]), Ok(()));
    assert_eq!(
        validate_token_id_formats(&[1, 0, u32::MAX]),
        Err(Error::InvalidTokenId)
    );
}

#[test]
fn composed_domain_validators_preserve_centralized_errors() {
    let valid_results = [
        standardize(validate_fee(1_000)),
        standardize(validate_royalty_bps(10_000)),
        standardize(validate_token_id_format(u32::MAX)),
    ];
    assert_eq!(validate_all(&valid_results), Ok(()));

    let invalid_results = [
        standardize(validate_fee(1_001)),
        standardize(validate_royalty_bps(10_001)),
        standardize(validate_token_id_format(0)),
    ];
    assert_eq!(validate_all(&invalid_results), Err(Error::InvalidFee));
    assert_eq!(
        validate_all(&invalid_results[1..]),
        Err(Error::InvalidBasisPoints)
    );
    assert_eq!(
        validate_all(&invalid_results[2..]),
        Err(Error::InvalidTokenId)
    );
}