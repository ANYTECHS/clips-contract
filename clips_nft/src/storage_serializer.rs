//! Storage serializer (Task 4).
//!
//! Provides typed encode/decode wrappers around Soroban's XDR-based
//! serialization (`to_xdr` / `from_xdr`) for the contract's core structs.
//! Using these helpers ensures a single, consistent serialization path and
//! makes future format migrations easier to audit.

use soroban_sdk::{xdr::ToXdr, Bytes, Env};

use crate::types::{Error, Royalty, TokenData};

// ─── TokenData ───────────────────────────────────────────────────────────────

/// Serialize [`TokenData`] to raw XDR bytes.
pub fn serialize_token_data(env: &Env, data: &TokenData) -> Bytes {
    data.to_xdr(env)
}

/// Deserialize [`TokenData`] from raw XDR bytes.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] when the bytes cannot be decoded.
pub fn deserialize_token_data(env: &Env, bytes: &Bytes) -> Result<TokenData, Error> {
    TokenData::from_xdr(env, bytes).ok_or(Error::TokenNotFound)
}

// ─── Royalty ─────────────────────────────────────────────────────────────────

/// Serialize a [`Royalty`] struct to raw XDR bytes.
pub fn serialize_royalty(env: &Env, royalty: &Royalty) -> Bytes {
    royalty.to_xdr(env)
}

/// Deserialize a [`Royalty`] struct from raw XDR bytes.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] when the bytes cannot be decoded.
pub fn deserialize_royalty(env: &Env, bytes: &Bytes) -> Result<Royalty, Error> {
    Royalty::from_xdr(env, bytes).ok_or(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};
    use crate::types::{Royalty, TokenData};

    #[test]
    fn test_token_data_round_trip() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let original = TokenData { owner, clip_id: 42 };
        let bytes = serialize_token_data(&env, &original);
        let decoded = deserialize_token_data(&env, &bytes).expect("decode failed");
        assert_eq!(decoded.clip_id, original.clip_id);
        assert_eq!(decoded.owner, original.owner);
    }

    #[test]
    fn test_royalty_round_trip() {
        let env = Env::default();
        let recipient = Address::generate(&env);
        let original = Royalty {
            recipient: recipient.clone(),
            basis_points: 500,
            asset_address: None,
        };
        let bytes = serialize_royalty(&env, &original);
        let decoded = deserialize_royalty(&env, &bytes).expect("decode failed");
        assert_eq!(decoded.basis_points, 500);
        assert_eq!(decoded.recipient, recipient);
        assert!(decoded.asset_address.is_none());
    }

    #[test]
    fn test_royalty_round_trip_with_asset() {
        let env = Env::default();
        let recipient = Address::generate(&env);
        let asset = Address::generate(&env);
        let original = Royalty {
            recipient: recipient.clone(),
            basis_points: 1_000,
            asset_address: Some(asset.clone()),
        };
        let bytes = serialize_royalty(&env, &original);
        let decoded = deserialize_royalty(&env, &bytes).expect("decode failed");
        assert_eq!(decoded.basis_points, 1_000);
        assert_eq!(decoded.asset_address, Some(asset));
    }

    #[test]
    fn test_deserialize_token_data_invalid_bytes() {
        let env = Env::default();
        let bad = Bytes::from_slice(&env, &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(
            deserialize_token_data(&env, &bad),
            Err(Error::TokenNotFound)
        );
    }

    #[test]
    fn test_deserialize_royalty_invalid_bytes() {
        let env = Env::default();
        let bad = Bytes::from_slice(&env, &[0xff, 0x00]);
        assert_eq!(
            deserialize_royalty(&env, &bad),
            Err(Error::TokenNotFound)
        );
    }
}
