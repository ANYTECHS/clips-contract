# Metadata Module - Quick Reference

## Quick Start

```rust
use crate::metadata::{Attribute, validate_metadata_uri, save_metadata};

// Validate and save metadata
validate_metadata_uri(&env, &uri)?;
save_metadata(&env, token_id, &uri);
```

## Common Operations

### Save Metadata
```rust
save_metadata(&env, token_id, &metadata_uri);
```

### Get Metadata
```rust
let uri = get_metadata(&env, token_id)?;
```

### Update Metadata
```rust
update_metadata(&env, token_id, &new_uri)?;
```

### Check Existence
```rust
if metadata_exists(&env, token_id) {
    // ...
}
```

## Validation

### URL Validation
```rust
validate_url(&env, &url)?;
validate_metadata_uri(&env, &uri)?;
validate_image_url(&env, &image)?;
validate_animation_url(&env, &animation_url)?;
```

### Attribute Validation
```rust
validate_attributes(&attributes)?;
```

### Description Validation
```rust
validate_description(&description)?;
```

## Supported Protocols

- `https://` - Secure HTTP
- `ipfs://` - IPFS
- `ar://` - Arweave

## Validation Limits

| Field | Maximum |
|-------|---------|
| URI Length | 512 chars |
| Description | 1000 chars |
| Attributes Count | 50 |
| Trait Type | 64 chars |
| Trait Value | 128 chars |

## Error Types

- `Error::InvalidURI` - Empty or malformed URI
- `Error::UnsupportedProtocol` - Invalid protocol
- `Error::TokenNotFound` - Token doesn't exist
- `Error::MalformedUrl` - URL format invalid

## Types

### Attribute
```rust
Attribute {
    trait_type: String,
    value: String,
}
```

### TokenMetadata
```rust
TokenMetadata {
    metadata_uri: String,
    image: Option<String>,
    animation_url: Option<String>,
    description: Option<String>,
    external_url: Option<String>,
    attributes: Vec<Attribute>,
}
```

## Helper Functions

```rust
is_empty_string(&s)                      // Check empty
clear_optional_field(&field)             // Clear empty optional
has_duplicate_traits(&attributes)        // Check duplicates
filter_empty_attributes(&env, &attrs)    // Filter empty
```

## Full Documentation

See [README.md](./README.md) for complete documentation.
