# Metadata Module

## Overview

The metadata module is responsible for managing all NFT metadata structures, helpers, and validation logic for the ClipsNFT contract. It provides a comprehensive solution for handling NFT metadata following industry standards like OpenSea's metadata format and EIP-721.

## Structure

```
metadata/
├── mod.rs          # Module exports and documentation
├── types.rs        # Core metadata type definitions
├── validation.rs   # Metadata validation logic
├── storage.rs      # Metadata storage operations
├── helpers.rs      # Utility functions
└── README.md       # This file
```

## Components

### Types (`types.rs`)

Defines the core metadata structures:

- **`Attribute`**: Represents a trait/attribute with `trait_type` and `value` fields
- **`ClipMetadata`**: Primary metadata structure for ClipCash NFTs including:
  - `clip_id`: Unique identifier for the video clip (required)
  - `metadata_uri`: Primary metadata URI - IPFS, Arweave, or HTTPS (required)
  - `image`: Optional image preview URL (thumbnail or poster frame)
  - `animation_url`: Optional animation/video content URL
  - `description`: Optional human-readable description
  - `external_url`: Optional external link for more information
  - `attributes`: Collection of trait attributes
- **`TokenMetadata`**: Complete metadata representation including:
  - `metadata_uri`: Primary metadata URI (IPFS, Arweave, or HTTPS)
  - `image`: Optional image URL
  - `animation_url`: Optional animation/video URL
  - `description`: Optional text description
  - `external_url`: Optional external link
  - `attributes`: Collection of trait attributes

### Validation (`validation.rs`)

Provides comprehensive validation for all metadata fields:

- **URL Protocol Validation**: Ensures only secure protocols are used (https://, ipfs://, ar://)
- **Field Length Validation**: Enforces maximum lengths for URIs, descriptions, and attributes
- **Attribute Validation**: Validates attribute count, trait_type, and value lengths
- **Security**: Prevents malformed URLs and unsupported protocols

**Supported Protocols:**
- `https://` - Secure HTTP
- `ipfs://` - IPFS (InterPlanetary File System)
- `ar://` - Arweave

**Validation Limits:**
- Max URI length: 512 characters
- Max description length: 1000 characters
- Max attributes per token: 50
- Max trait_type length: 64 characters
- Max trait_value length: 128 characters

### Storage (`storage.rs`)

Handles persistence and retrieval of NFT metadata:

- **`save_metadata()`**: Persist metadata URI for a token
- **`get_metadata()`**: Retrieve metadata URI for a token
- **`update_metadata()`**: Update existing metadata URI
- **`metadata_exists()`**: Check if metadata exists for a token
- **`remove_metadata()`**: Remove metadata (used during burn)

All metadata is stored in persistent storage using `DataKey::Metadata(token_id)`.

### Helpers (`helpers.rs`)

Utility functions for metadata operations:

- **`is_empty_string()`**: Check if a string is empty
- **`clear_optional_field()`**: Clear optional fields containing empty strings
- **`normalize_url()`**: Normalize URL format (placeholder for future enhancement)
- **`build_metadata_json()`**: Build JSON representation (placeholder)
- **`has_duplicate_traits()`**: Check for duplicate trait_types
- **`filter_empty_attributes()`**: Remove empty attributes from a vector

## Usage Examples

### Basic Usage with ClipMetadata

```rust
use crate::metadata::{ClipMetadata, validate_metadata_uri};

// Create minimal ClipMetadata
let clip_id = 12345u32;
let uri = String::from_str(&env, "ipfs://QmHash");
let metadata = ClipMetadata::new(&env, clip_id, uri.clone());

// Validate metadata URI
validate_metadata_uri(&env, &uri)?;
```

### ClipMetadata with Full Data

```rust
use crate::metadata::{ClipMetadata, Attribute};

let clip_id = 67890u32;
let uri = String::from_str(&env, "ipfs://QmFullHash");

// Create attributes
let mut attributes = Vec::new(&env);
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "virality_score"),
    value: String::from_str(&env, "98"),
});
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "duration"),
    value: String::from_str(&env, "42s"),
});

// Create full metadata
let metadata = ClipMetadata::with_full_data(
    clip_id,
    uri,
    Some(String::from_str(&env, "https://example.com/thumb.jpg")),
    Some(String::from_str(&env, "ipfs://QmVideoHash")),
    Some(String::from_str(&env, "Epic gaming moment")),
    Some(String::from_str(&env, "https://clipcash.com/clip/67890")),
    attributes,
);

// Check for optional fields
if metadata.has_optional_fields() {
    let attr_count = metadata.attribute_count();
    // Process rich metadata
}
```

### Basic Usage with TokenMetadata

```rust
use crate::metadata::{
    Attribute, TokenMetadata, validate_metadata_uri, save_metadata
};

// Create metadata
let uri = String::from_str(&env, "ipfs://QmHash");
let metadata = TokenMetadata::new(&env, uri.clone());

// Validate metadata URI
validate_metadata_uri(&env, &uri)?;

// Save metadata
save_metadata(&env, token_id, &uri);
```

### With Optional Fields

```rust
use crate::metadata::{
    validate_image_url, validate_animation_url, validate_attributes
};

let image = Some(String::from_str(&env, "https://example.com/image.png"));
let animation_url = Some(String::from_str(&env, "ipfs://QmVideo"));

// Validate optional fields
validate_image_url(&env, &image)?;
validate_animation_url(&env, &animation_url)?;
```

### Working with Attributes

```rust
use crate::metadata::Attribute;

let mut attributes = Vec::new(&env);
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "rarity"),
    value: String::from_str(&env, "legendary"),
});
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "power"),
    value: String::from_str(&env, "9000"),
});

// Validate attributes
validate_attributes(&attributes)?;
```

### Retrieving Metadata

```rust
use crate::metadata::{get_metadata, metadata_exists};

// Check if metadata exists
if metadata_exists(&env, token_id) {
    // Get metadata
    let uri = get_metadata(&env, token_id)?;
}
```

## Standards Compliance

The metadata module follows these standards:

1. **OpenSea Metadata Standard**: Compatible with OpenSea's expected metadata format
2. **EIP-721**: Follows the ERC-721 metadata JSON schema
3. **Security Best Practices**: Only allows secure protocols, validates input lengths

## Error Handling

The module uses the following errors from `crate::types::Error`:

- `Error::InvalidURI`: Empty, malformed, or too long URI
- `Error::UnsupportedProtocol`: URL uses unsupported protocol (e.g., http://, ftp://)
- `Error::MalformedUrl`: URL format is invalid
- `Error::TokenNotFound`: Token doesn't exist or has no metadata

## Future Enhancements

Potential future improvements:

1. **JSON Generation**: Full JSON serialization support for `build_metadata_json()`
2. **URL Normalization**: Advanced URL cleaning and canonicalization
3. **Content Validation**: Optional content-type checking for URLs
4. **Caching**: Metadata caching for frequently accessed tokens
5. **Batch Operations**: Bulk metadata operations for efficiency
6. **Metadata Versioning**: Track metadata update history

## Testing

Each submodule includes placeholder test sections. To run tests:

```bash
cargo test --package clips_nft
```

## Integration

The metadata module is integrated into the main contract via `lib.rs`:

```rust
pub mod metadata;

pub use metadata::{
    Attribute, TokenMetadata,
    validate_url, validate_metadata_uri, 
    // ... other exports
};
```

This allows other modules to use metadata functionality:

```rust
use crate::metadata::{validate_metadata_uri, save_metadata};
```

## Documentation

Generate full documentation:

```bash
cargo doc --package clips_nft --open
```

## Contributing

When adding new functionality to the metadata module:

1. Add types to `types.rs`
2. Add validation to `validation.rs`
3. Add storage operations to `storage.rs`
4. Add utilities to `helpers.rs`
5. Export in `mod.rs`
6. Update this README
7. Add tests

## License

See the main contract license.
