use soroban_sdk::String;

/// Maximum byte length for a metadata URI.
pub const MAX_URI_LEN: u32 = 256;

/// Validates a metadata URI before storage.
///
/// Rules:
/// - Must not be empty.
/// - Must not exceed [`MAX_URI_LEN`] bytes.
///
/// # Errors
/// Returns `Err(())` on any violation.
pub fn validate_metadata_uri(uri: &String) -> Result<(), ()> {
    let len = uri.len();
    if len == 0 || len > MAX_URI_LEN {
        return Err(());
    }
    Ok(())
}
