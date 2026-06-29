#![cfg(test)]

use clips_nft::errors::Error;

#[test]
fn test_clip_already_minted_is_distinct_variant() {
    // ClipAlreadyMinted must be a distinct, matchable error variant.
    let err = Error::ClipAlreadyMinted;
    assert_eq!(err, Error::ClipAlreadyMinted);
    assert_ne!(err, Error::AlreadyMinted);
    assert_ne!(err, Error::Unauthorized);
}

#[test]
fn test_clip_already_minted_error_code() {
    // Discriminant must be 11 to remain stable across versions.
    assert_eq!(Error::ClipAlreadyMinted as u32, 11);
}
