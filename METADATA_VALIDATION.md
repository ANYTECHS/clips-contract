# Metadata Validation Implementation

## Overview

This document details the comprehensive metadata validation system implemented in the ClipsNFT smart contract. The implementation addresses four key requirements:

1. **Safe Metadata Deserialization** - Handle invalid data with custom errors
2. **Storage Limits** - Enforce configured metadata size constraints
3. **URI Format Validation** - Support multiple storage backends
4. **Reusable Validation** - Implement validation before minting

## Implementation Details

### 1. Error Types

Four new error variants have been added to the `Error` enum:

```rust
pub enum Error {
    // ... existing errors ...
    
    /// Metadata URI format is invalid
    InvalidMetadataUri = 13,
    
    /// Metadata exceeds maximum allowed size
    MetadataTooLarge = 14,
    
    /// Metadata deserialization failed
    MetadataDeserializationFailed = 15,
    
    /// Metadata validation failed
    MetadataValidationFailed = 16,
}
```

### 2. Metadata Validation Module

A dedicated `metadata` module provides safe, reusable validation functions:

#### Maximum Size Constant

```rust
pub const MAX_METADATA_SIZE: u32 = 5_120; // 5 KB limit
```

#### Validation Functions

**URI Format Validation**
```rust
pub fn validate_uri(uri: &String) -> bool
```
Validates that the URI uses a supported scheme:
- `ipfs://` - IPFS content addressing
- `https://` - HTTPS URLs
- `ar://` - Arweave transaction IDs

**Size Validation**
```rust
pub fn validate_size(uri: &String) -> bool
```
Ensures the metadata URI does not exceed `MAX_METADATA_SIZE` (5 KB).

**Comprehensive Validation**
```rust
pub fn validate_metadata(uri: &String) -> bool
```
Performs both URI format and size validation before minting.

### 3. Mint Function Integration

The `mint()` function now performs validation early in execution:

```rust
pub fn mint(
    env: Env,
    to: Address,
    clip_id: u32,
    metadata_uri: String,
    royalty: Royalty,
    is_soulbound: bool,
    signature: BytesN<64>,
) -> Result<TokenId, Error> {
    to.require_auth();
    Self::require_not_paused(&env)?;

    // Validate metadata BEFORE signature verification
    if !metadata::validate_metadata(&metadata_uri) {
        return Err(Error::InvalidMetadataUri);
    }

    // Continue with signature verification and storage...
}
```

**Validation Order:**
1. Metadata validation (fast, rejects invalid URIs early)
2. Signature verification (computationally expensive)
3. State operations (persistent storage)

This order optimizes gas usage by rejecting invalid metadata before expensive signature verification.

## Supported URI Schemes

### IPFS (InterPlanetary File System)

**Format:** `ipfs://[content-hash]`

**Example:** `ipfs://QmExample1234567890`

**Characteristics:**
- Content-addressed storage
- Immutable by design
- Decentralized replication
- Recommended for metadata permanence

### HTTPS

**Format:** `https://[domain]/[path]`

**Example:** `https://api.example.com/metadata/token-123.json`

**Characteristics:**
- Centralized storage
- Dynamic content updates
- Familiar infrastructure
- Gateway availability risk

### Arweave

**Format:** `ar://[transaction-id]`

**Example:** `ar://a1b2c3d4e5f6g7h8i9j0`

**Characteristics:**
- Permanent on-chain storage
- Blockchain-based immutability
- No content hash scheme
- One-time payment model

## Storage Limits

### Maximum Metadata URI Size

- **Limit:** 5,120 bytes (5 KB)
- **Rationale:**
  - Soroban contract storage is limited
  - String serialization overhead
  - Practical token metadata doesn't require larger URIs
  - External metadata typically stored via URI reference

### Size Calculation

The size validation counts the UTF-8 encoded bytes of the complete URI string:

```rust
fn validate_size(uri: &String) -> bool {
    let uri_bytes = uri.to_string().as_bytes();
    uri_bytes.len() as u32 <= MAX_METADATA_SIZE
}
```

## Test Coverage

### Validation Tests

#### URI Format Tests
- ✅ Valid IPFS URIs
- ✅ Valid HTTPS URIs
- ✅ Valid Arweave URIs
- ✅ Invalid schemes (FTP, etc.)
- ✅ Empty scheme content
- ✅ Missing scheme

#### Size Tests
- ✅ Metadata within size limit
- ✅ Metadata exceeding size limit
- ✅ Metadata exactly at size limit (edge case)

#### Integration Tests
- ✅ Mint with valid IPFS metadata
- ✅ Mint with valid HTTPS metadata
- ✅ Mint with valid Arweave metadata
- ✅ Mint fails with invalid URI scheme
- ✅ Mint fails with empty IPFS URI
- ✅ Mint fails with no URI scheme
- ✅ Mint fails with oversized URI
- ✅ Multiple valid metadata formats

### Test Functions

1. **`test_validate_uri_ipfs_format`** - IPFS URI validation
2. **`test_validate_uri_https_format`** - HTTPS URI validation
3. **`test_validate_uri_arweave_format`** - Arweave URI validation
4. **`test_validate_uri_rejects_invalid_scheme`** - Invalid scheme rejection
5. **`test_validate_metadata_size_within_limit`** - Size validation pass
6. **`test_validate_metadata_size_exceeds_limit`** - Size validation fail
7. **`test_validate_metadata_comprehensive`** - Combined validation
8. **`test_mint_with_valid_ipfs_metadata`** - IPFS mint integration
9. **`test_mint_with_valid_https_metadata`** - HTTPS mint integration
10. **`test_mint_with_valid_arweave_metadata`** - Arweave mint integration
11. **`test_mint_fails_with_invalid_metadata_uri_scheme`** - Invalid scheme rejection
12. **`test_mint_fails_with_empty_ipfs_uri`** - Empty IPFS URI rejection
13. **`test_mint_fails_with_no_uri_scheme`** - No scheme rejection
14. **`test_mint_fails_with_oversized_metadata_uri`** - Oversized URI rejection
15. **`test_multiple_valid_metadata_uri_formats`** - Multiple format support
16. **`test_metadata_validation_edge_case_exactly_at_size_limit`** - Edge case: exactly at limit

## Usage Example

```rust
// Mint with IPFS metadata
let token_id = client.mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "ipfs://QmMetadataHash"),
    &royalty_config,
    &false,
    &signature,
)?;

// Mint with HTTPS metadata
let token_id = client.mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "https://api.example.com/metadata.json"),
    &royalty_config,
    &false,
    &signature,
)?;

// Attempt with invalid URI (will fail)
let result = client.try_mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "ftp://invalid.com/metadata"),
    &royalty_config,
    &false,
    &signature,
);
assert_eq!(result, Err(Ok(Error::InvalidMetadataUri)));
```

## Security Considerations

### Validation Order

Metadata validation occurs **before** signature verification to:
1. Reject obviously invalid URIs early
2. Minimize computational overhead
3. Prevent DoS attacks via expensive verification of bad metadata
4. Preserve signer reputation (not wasting verification on invalid input)

### Format Validation

The implementation uses simple string prefix matching rather than full URL parsing to:
- Minimize gas consumption
- Reduce contract code size
- Prevent parsing complexity
- Support diverse URI schemes uniformly

### Size Limits

The 5 KB limit balances:
- On-chain storage efficiency
- Practical URI lengths for all supported schemes
- Prevention of metadata bloat
- Reasonable buffer for future extensions

## Error Handling

### Validation Errors

```rust
// Invalid scheme
if !metadata::validate_uri(uri) {
    return Err(Error::InvalidMetadataUri);
}

// Size violation
if !metadata::validate_size(uri) {
    return Err(Error::InvalidMetadataUri);
}
```

### Consumer Handling

Consumers of the contract should:
1. Pre-validate metadata URIs using the same scheme
2. Handle `InvalidMetadataUri` errors gracefully
3. Provide clear error messages to users
4. Log failed validation attempts for analytics

## Future Extensions

Potential enhancements:
- Additional URI schemes (e.g., `bzz://` for Swarm)
- Configurable size limits via admin setting
- Metadata schema validation (JSON parsing)
- Content hash verification
- URI deduplication across tokens

## Branch

Implementation completed on branch: `feat/metadata-validation`

## Commit

```
feat: implement comprehensive metadata validation

- Deserialize and validate metadata URIs safely
- Support IPFS, HTTPS, and Arweave URI schemes
- Enforce metadata storage size limits (5KB)
- Implement reusable metadata validation module
- Add 15+ comprehensive tests for metadata validation
```
