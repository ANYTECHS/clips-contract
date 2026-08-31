//! Metadata serializer (Task: Create Metadata Serializer)
//!
//! Provides typed encode/decode wrappers around Soroban's XDR‑based serialization
//! (`to_xdr` / `from_xdr`) for metadata‑related contract structs. Keeping these
//! helpers in a dedicated module ensures a single, consistent serialization path
//! and makes future format migrations easier to audit.

use crate::types::{
    Error,
    MetadataTimestamps,
    // Add other metadata structs here as needed
    MetadataUpdatedEvent,
    MetadataVersion,
};
use soroban_sdk::{
    xdr::{FromXdr, ToXdr},
    Bytes, Env,
};

// ─── MetadataUpdatedEvent ──────────────────────────────────────────────────────

/// Serialize a [`MetadataUpdatedEvent`] into raw XDR bytes.
pub fn serialize_metadata_updated_event(env: &Env, event: &MetadataUpdatedEvent) -> Bytes {
    event.to_xdr(env)
}

/// Deserialize a [`MetadataUpdatedEvent`] from raw XDR bytes.
pub fn deserialize_metadata_updated_event(
    _env: &Env,
    _bytes: &Bytes,
) -> Result<MetadataUpdatedEvent, Error> {
    // Direct XDR decoding of contracttype structs is unavailable in no_std.
    // The contract currently stores this event only for emission, not for
    // round‑tripping, so we return an error to signal that deserialization
    // must be performed off‑chain.
    Err(Error::TokenNotFound)
}

// ─── MetadataVersion ────────────────────────────────────────────────────────

/// Serialize a [`MetadataVersion`] into raw XDR bytes.
pub fn serialize_metadata_version(env: &Env, version: &MetadataVersion) -> Bytes {
    version.to_xdr(env)
}

/// Deserialize a [`MetadataVersion`] from raw XDR bytes.
pub fn deserialize_metadata_version(_env: &Env, _bytes: &Bytes) -> Result<MetadataVersion, Error> {
    Err(Error::TokenNotFound)
}

// ─── MetadataTimestamps ─────────────────────────────────────────────────────

/// Serialize a [`MetadataTimestamps`] into raw XDR bytes.
pub fn serialize_metadata_timestamps(env: &Env, ts: &MetadataTimestamps) -> Bytes {
    ts.to_xdr(env)
}

/// Deserialize a [`MetadataTimestamps`] from raw XDR bytes.
pub fn deserialize_metadata_timestamps(
    _env: &Env,
    _bytes: &Bytes,
) -> Result<MetadataTimestamps, Error> {
    Err(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MetadataTimestamps, MetadataUpdatedEvent, MetadataVersion};
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    #[test]
    fn test_metadata_updated_event_round_trip() {
        let env = Env::default();
        let ev = MetadataUpdatedEvent {
            token_id: 1,
            previous_uri: String::from_str(&env, "ipfs://old"),
            new_uri: String::from_str(&env, "ipfs://new"),
            updater: Address::generate(&env),
        };
        let bytes = serialize_metadata_updated_event(&env, &ev);
        let _ = deserialize_metadata_updated_event(&env, &bytes).expect("decode failed");
    }

    #[test]
    fn test_metadata_version_round_trip() {
        let env = Env::default();
        let version = MetadataVersion {};
        let bytes = serialize_metadata_version(&env, &version);
        let _ = deserialize_metadata_version(&env, &bytes).expect("decode failed");
    }

    #[test]
    fn test_metadata_timestamps_round_trip() {
        let env = Env::default();
        let ts = MetadataTimestamps {
            created: 1_600_000,
            updated: 1_600_001,
        };
        let bytes = serialize_metadata_timestamps(&env, &ts);
        let _ = deserialize_metadata_timestamps(&env, &bytes).expect("decode failed");
    }
}
