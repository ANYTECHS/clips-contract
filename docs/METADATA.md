# ClipCash NFT Metadata Documentation

> **Issue:** #632 — Metadata Architecture Documentation  
> **Status:** Complete  
> **Last Updated:** 2026-07-23

---

## Table of Contents

1. [Overview](#overview)
2. [Metadata Schema](#metadata-schema)
3. [Field Descriptions](#field-descriptions)
4. [Serialization Format](#serialization-format)
5. [IPFS Compatibility](#ipfs-compatibility)
6. [Examples](#examples)
7. [Upgrade Strategy](#upgrade-strategy)
8. [Validation Rules](#validation-rules)
9. [Storage Layout](#storage-layout)
10. [Configuration](#configuration)
11. [Error Reference](#error-reference)

---

## Overview

ClipCash NFTs use a dual-layer metadata architecture:

| Layer | Location | Purpose |
|-------|----------|---------|
| **On-chain** | Soroban persistent storage | Stores a compact metadata URI pointer and essential token data |
| **Off-chain** | IPFS / Arweave / HTTPS | Stores the full JSON metadata following OpenSea/EIP-721 standards |

This design minimizes on-chain storage costs (critical on Stellar) while preserving rich, standards-compliant metadata that marketplaces and wallets can consume.

The contract supports two metadata representations:

1. **Legacy `ClipMetadata`** (`metadata.rs`) — Compact pipe-delimited string for basic minting
2. **Modern `TokenMetadata`** (`metadata/` module) — Full structured metadata with validation, versioning, and update policies

---

## Metadata Schema

### Core Types

#### `Attribute`

Represents a single trait/attribute pair following the OpenSea metadata standard.

```rust
#[contracttype]
pub struct Attribute {
    pub trait_type: String,  // e.g., "virality_score", "duration", "rarity"
    pub value: String,       // e.g., "98", "42s", "legendary"
}
```

**Constraints:**
- `trait_type`: 1–64 characters
- `value`: 1–128 characters
- Maximum 50 attributes per token

---

#### `ClipMetadata` (Legacy)

Compact on-chain metadata for basic clip representation.

```rust
pub struct ClipMetadata {
    pub title: String,        // Required. Clip title.
    pub description: String,  // Required. Human-readable description.
    pub uri: String,          // Required. Primary metadata URI (IPFS/HTTPS).
    pub image: String,        // Optional. Preview image URI (empty if none).
    pub creator: String,      // Required. Creator identifier/address.
}
```

**On-chain encoding:** `title|description|uri|image|creator` (pipe-delimited)

---

#### `TokenMetadata` (Modern)

Full metadata structure following OpenSea and EIP-721 standards.

```rust
#[contracttype]
pub struct TokenMetadata {
    pub metadata_uri: String,              // Required. Primary URI.
    pub image: Option<String>,             // Optional. Image preview URL.
    pub animation_url: Option<String>,     // Optional. Video/animation URL.
    pub description: Option<String>,       // Optional. Text description.
    pub external_url: Option<String>,      // Optional. External link.
    pub attributes: Vec<Attribute>,        // Optional. Trait collection.
}
```

---

#### `MetadataImage` (Thumbnail)

Structured thumbnail metadata with dimensions and MIME type.

```rust
#[contracttype]
pub struct MetadataImage {
    pub image_url: String,   // Thumbnail URL
    pub mime_type: String,   // e.g., "image/png", "image/jpeg"
    pub width: u32,          // Width in pixels
    pub height: u32,         // Height in pixels
}
```

---

### Metadata Timestamps

Tracks creation and last-update times for audit and policy enforcement.

```rust
#[contracttype]
pub struct MetadataTimestamps {
    pub created_at: u64,   // Ledger timestamp at mint
    pub updated_at: u64,   // Ledger timestamp at last update
}
```

---

### Metadata Version

Tracks the metadata schema version for upgrade compatibility.

```rust
#[contracttype]
pub struct MetadataVersion {
    pub version: u32,  // Default: 1
}
```

---

## Field Descriptions

### Required Fields

| Field | Type | Description | Max Length |
|-------|------|-------------|------------|
| `clip_id` | `u32` | Unique off-chain clip identifier. Prevents double-minting. | — |
| `metadata_uri` | `String` | Primary pointer to off-chain JSON metadata. | 512 chars |
| `title` | `String` | Human-readable clip title (legacy only). | — |
| `description` | `String` | Human-readable clip description (legacy only). | 1000 chars |
| `creator` | `String` | Creator wallet address or identifier (legacy only). | — |

### Optional Fields

| Field | Type | Description | Max Length |
|-------|------|-------------|------------|
| `image` | `Option<String>` | Preview image/thumbnail URL. | 512 chars |
| `thumbnail` | `Option<MetadataImage>` | Structured thumbnail with MIME type and dimensions. | — |
| `animation_url` | `Option<String>` | Video or animation content URL. | 512 chars |
| `external_url` | `Option<String>` | Link to original platform or creator page. | 512 chars |
| `attributes` | `Vec<Attribute>` | Trait/attribute pairs for filtering and display. | 50 items |

---

## Serialization Format

### On-Chain Storage

Metadata is stored in **Soroban persistent storage** under the key `DataKey::Metadata(token_id)`.

```rust
// Storage key pattern
DataKey::Metadata(token_id: u32) -> String
```

Only the **metadata URI** is stored on-chain by default. The full JSON metadata lives off-chain at that URI.

### Legacy Serialization (`metadata.rs`)

The legacy `ClipMetadata` serializes to a pipe-delimited string:

```
title|description|uri|image|creator
```

**Example:**
```
Epic Gaming Moment|Best clutch of the season|ipfs://QmHash123|ipfs://QmImage456|GDA...
```

**Deserialization** splits on `|` and expects exactly 4 separators (5 fields). Returns `MetadataError::MalformedData` if the format is invalid.

### Modern Serialization (`metadata/` module)

The modern system uses Soroban's `#[contracttype]` derive, which automatically handles:

- **XDR serialization** for contract storage
- **SCVal conversion** for cross-contract calls
- **Type safety** at compile time

```rust
// Stored directly as a typed struct
env.storage().persistent().set(
    &DataKey::Metadata(token_id),
    &metadata_uri_string
);
```

### IPFS Metadata JSON Generation

The contract can generate IPFS-compatible JSON metadata strings:

```json
{
  "name": "Epic Gaming Moment",
  "description": "Best clutch of the season",
  "image": "ipfs://QmImage456",
  "animation_url": "ipfs://QmVideo789",
  "creator": "GDA..."
}
```

---

## IPFS Compatibility

### Supported Protocols

| Protocol | Prefix | Use Case |
|----------|--------|----------|
| **IPFS** | `ipfs://` | Primary decentralized storage |
| **HTTPS** | `https://` | Centralized gateways and APIs |
| **Arweave** | `ar://` | Permanent decentralized storage |

### IPFS Gateway Configuration

The contract supports a configurable IPFS gateway for HTTP resolution:

```rust
// Default gateway
const DEFAULT_IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

// Set custom gateway (admin only)
set_ipfs_gateway(env, "https://gateway.pinata.cloud/ipfs/");
```

**Gateway validation rules:**
- Must start with `https://` or `http://`
- Raw `ipfs://` is rejected (gateways are HTTP endpoints)

### Metadata Base URI

Administrators can configure a base URI prepended to token IDs:

```rust
set_metadata_base_uri(env, "https://api.clipcash.com/metadata/");
// Token 42 resolves to: https://api.clipcash.com/metadata/42
```

**Supported schemes:** `https://`, `http://`, `ipfs://`, `ar://`

### URI Builder Helpers

```rust
// Check if URI uses IPFS scheme
is_ipfs("ipfs://QmHash") -> true

// Build IPFS URI from CID
build_ipfs_uri(env, "QmHash") -> "ipfs://QmHash"

// Validate any URI
validate_uri("ipfs://QmHash") -> Ok(())
validate_uri("ftp://files.com") -> Err(Error::InvalidURI)
```

---

## Examples

### Example 1: Minimal Mint (Legacy)

```rust
let metadata = ClipMetadata {
    title: String::from_str(&env, "First Blood"),
    description: String::from_str(&env, "Clutch 1v3 in ranked"),
    uri: String::from_str(&env, "ipfs://QmMinimalHash"),
    image: String::from_str(&env, ""),
    creator: String::from_str(&env, "GDA..."),
};

// Validate
validate(&env, &metadata)?;  // Checks non-empty title, description, uri, creator

// Serialize for storage
let serialized = serialize(&env, &metadata);
// -> "First Blood|Clutch 1v3 in ranked|ipfs://QmMinimalHash||GDA..."
```

### Example 2: Full Metadata (Modern)

```rust
use crate::metadata::{ClipMetadata, Attribute};

let mut attributes = Vec::new(&env);
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "virality_score"),
    value: String::from_str(&env, "98"),
});
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "duration"),
    value: String::from_str(&env, "42s"),
});
attributes.push_back(Attribute {
    trait_type: String::from_str(&env, "platform"),
    value: String::from_str(&env, "twitch"),
});

let metadata = ClipMetadata::with_full_data(
    12345,                                          // clip_id
    String::from_str(&env, "ipfs://QmFullHash"),    // metadata_uri
    Some(String::from_str(&env, "https://cdn.clipcash.com/thumb/12345.jpg")), // image
    Some(String::from_str(&env, "ipfs://QmVideoHash")), // animation_url
    Some(String::from_str(&env, "Epic 1v3 clutch in Diamond ranked")), // description
    Some(String::from_str(&env, "https://clipcash.com/clip/12345")), // external_url
    attributes,
);

// Validate all fields
validate_metadata_uri(&env, &metadata.metadata_uri)?;
validate_image_url(&env, &metadata.image)?;
validate_animation_url(&env, &metadata.animation_url)?;
validate_description(&metadata.description)?;
validate_attributes(&metadata.attributes)?;
```

### Example 3: Off-Chain JSON Metadata

The metadata URI should resolve to a JSON file following this schema:

```json
{
  "name": "Epic 1v3 Clutch #12345",
  "description": "Epic 1v3 clutch in Diamond ranked",
  "image": "https://cdn.clipcash.com/thumb/12345.jpg",
  "animation_url": "ipfs://QmVideoHash",
  "external_url": "https://clipcash.com/clip/12345",
  "attributes": [
    {
      "trait_type": "virality_score",
      "value": "98"
    },
    {
      "trait_type": "duration",
      "value": "42s"
    },
    {
      "trait_type": "platform",
      "value": "twitch"
    }
  ]
}
```

### Example 4: Metadata Update (One-Time)

```rust
// Owner can update metadata once
check_update_allowed(&env, token_id, &caller)?;  // Fails if already used
update_metadata(&env, token_id, &new_uri)?;
mark_update_used(&env, token_id);  // Consume the one-time slot

// Admin can always update (bypasses one-time limit)
check_update_allowed(&env, token_id, &admin)?;  // Always Ok for admin
update_metadata(&env, token_id, &new_uri)?;
```

### Example 5: Storage Operations

```rust
// Save metadata at mint time
save_metadata(&env, token_id, &metadata_uri);

// Retrieve metadata
let uri = get_metadata(&env, token_id)?;  // Returns Err(TokenNotFound) if missing

// Check existence
if metadata_exists(&env, token_id) {
    // Safe to retrieve
}

// Update existing metadata
update_metadata(&env, token_id, &new_uri)?;  // Fails if token doesn't exist

// Remove on burn
remove_metadata(&env, token_id);
```

---

## Upgrade Strategy

### Metadata Versioning

The contract tracks a global metadata schema version:

```rust
const DEFAULT_METADATA_VERSION: u32 = 1;

// Get current version
let version = get_metadata_version(&env);  // Defaults to 1

// Set new version (admin only)
set_metadata_version(&env, MetadataVersion { version: 2 });
```

### Update Policy

Each token has a **one-time metadata update** slot:

| Caller | Updates Allowed |
|--------|-----------------|
| Token owner | 1 (one-time) |
| Contract admin | Unlimited |

The update flag is stored at `DataKey::MetadataUpdated(token_id)` in persistent storage.

### Migration Guidelines

When upgrading metadata schema:

1. **Bump `MetadataVersion`** before any breaking change
2. **Never rename `DataKey` variants** — orphans existing entries
3. **Add new fields as `Option<T>`** so older values deserialize gracefully
4. **Clean up old entries** via migration function before removing `DataKey` variants
5. **Update timestamps** on every metadata change via `touch_updated_at()`

### Size Limits

| Limit | Value | Purpose |
|-------|-------|---------|
| Max metadata URI | 512 bytes | Prevents unbounded storage |
| Max description | 1000 chars | Prevents gas abuse |
| Max attributes | 50 | Keeps metadata readable |
| Max metadata size | 102,400 bytes (100 KB) | Global mint-time limit |

---

## Validation Rules

### URL Validation

```rust
SUPPORTED_PROTOCOLS = ["https://", "ipfs://", "ar://"]

validate_url("https://example.com")   -> Ok(())
validate_url("ipfs://QmHash")         -> Ok(())
validate_url("ar://TxId")             -> Ok(())
validate_url("http://insecure.com")   -> Err(UnsupportedProtocol)
validate_url("ftp://files.com")       -> Err(UnsupportedProtocol)
validate_url("")                      -> Err(MalformedUrl)
```

### Field Validation Matrix

| Field | Empty | Too Long | Bad Protocol | Duplicate Traits |
|-------|-------|----------|--------------|------------------|
| `metadata_uri` | Reject | >512 | Reject | — |
| `image` | Skip | >512 | Reject | — |
| `animation_url` | Skip | >512 | Reject | — |
| `external_url` | Skip | >512 | Reject | — |
| `description` | Skip | >1000 | — | — |
| `attributes` | Empty vec | >50 | — | Reject |
| `trait_type` | Reject | >64 | — | — |
| `value` | Reject | >128 | — | — |

### Helper Utilities

| Function | Purpose |
|----------|---------|
| `is_empty_string(s)` | Check if string is empty |
| `clear_optional_field(opt)` | Convert `Some("")` to `None` |
| `has_duplicate_traits(attrs)` | O(n²) duplicate detection |
| `filter_empty_attributes(env, attrs)` | Remove empty trait_type/value pairs |
| `normalize_url(url)` | Placeholder for future normalization |

---

## Storage Layout

### Persistent Storage (Per-Token)

```
DataKey::Metadata(token_id)        -> String (metadata URI)
DataKey::MetadataUpdated(token_id) -> bool   (update consumed flag)
DataKey::MetadataTimestamps(token_id) -> MetadataTimestamps
```

### Instance Storage (Global)

```
ConfigKey::IpfsGateway      -> String (HTTP gateway URL)
ConfigKey::MetadataBaseUri  -> String (base URI for token resolution)
DataKey::MetadataVersion    -> MetadataVersion
DataKey::MaxMetadataSize    -> u32 (default: 102400)
```

### Storage Cost at Mint

| Operation | Key | Tier | Type |
|-----------|-----|------|------|
| Read | `Admin` | instance | auth |
| Read | `NextTokenId` | instance | counter |
| Read | `Paused` | instance | circuit-breaker |
| Read | `ClipIdMinted(clip_id)` | persistent | dedup check |
| **Write** | `Token(token_id)` | persistent | owner + clip_id |
| **Write** | `Metadata(token_id)` | persistent | URI |
| **Write** | `Royalty(token_id)` | persistent | royalty config |
| **Write** | `ClipIdMinted(clip_id)` | persistent | reverse index |
| **Write** | `NextTokenId` | instance | increment counter |

**Total persistent writes per mint: 4**  
**Total instance writes per mint: 1**

---

## Configuration

### IPFS Gateway

```rust
// Set (admin only)
set_ipfs_gateway(env, "https://ipfs.io/ipfs/")?;  // Ok
set_ipfs_gateway(env, "ipfs://QmHash")?;          // Err(InvalidURI)

// Get
let gateway = get_ipfs_gateway(env);  // Option<String>
```

### Metadata Base URI

```rust
// Set (admin only)
set_metadata_base_uri(env, "https://api.clipcash.com/metadata/")?;  // Ok
set_metadata_base_uri(env, "ftp://files.com/")?;                    // Err(InvalidURI)

// Get
let base = get_metadata_base_uri(env);  // Option<String>
```

### Max Metadata Size

```rust
// Set (admin only)
set_max_metadata_size(env, 102400)?;  // 100 KB
set_max_metadata_size(env, 0)?;       // Err(InvalidConfig)

// Get
let max = get_max_metadata_size(env);  // u32, defaults to 102400
```

---

## Error Reference

| Error | Module | Cause | Resolution |
|-------|--------|-------|------------|
| `MetadataError::EmptyTitle` | `metadata.rs` | Title field is empty | Provide a non-empty title |
| `MetadataError::EmptyDescription` | `metadata.rs` | Description field is empty | Provide a non-empty description |
| `MetadataError::InvalidUri` | `metadata.rs` | URI empty or bad protocol | Use `ipfs://` or `https://` |
| `MetadataError::InvalidImage` | `metadata.rs` | Image has bad protocol | Use supported protocol or leave empty |
| `MetadataError::EmptyCreator` | `metadata.rs` | Creator field is empty | Provide creator address |
| `MetadataError::MalformedData` | `metadata.rs` | Pipe-delimited string invalid | Ensure exactly 4 `\|` separators |
| `Error::InvalidURI` | `validation.rs` | URI empty or too long | Keep under 512 chars |
| `Error::UnsupportedProtocol` | `validation.rs` | Protocol not in whitelist | Use `https://`, `ipfs://`, or `ar://` |
| `Error::MalformedUrl` | `validation.rs` | URL format invalid | Check URL structure |
| `Error::TokenNotFound` | `storage.rs` | No metadata for token ID | Mint token first |
| `Error::MetadataAlreadyUpdated` | `update_policy.rs` | One-time update already used | Contact admin for override |
| `Error::MetadataSizeTooLarge` | `metadata_size.rs` | URI exceeds 512 bytes | Use shorter URI or IPFS CID |
| `Error::InvalidConfig` | `metadata_config.rs` | Max size is 0 or URI too large | Set valid max size |

---

## Standards Compliance

| Standard | Status | Notes |
|----------|--------|-------|
| **OpenSea Metadata Standard** | Full | All standard fields supported |
| **EIP-721 Metadata JSON Schema** | Full | Compatible with ERC-721 marketplaces |
| **EIP-2981 Royalty Info** | Adapted | Soroban-native royalty implementation |
| **SEP-0041** | Supported | Multi-asset royalty payments |

---

## Module Reference

```
clips_nft/src/
├── metadata.rs              # Legacy: serialization, validation, IPFS JSON gen
├── metadata/
│   ├── mod.rs               # Module exports and documentation
│   ├── types.rs             # Attribute, ClipMetadata, TokenMetadata, MetadataImage
│   ├── validation.rs        # URL, field, and attribute validation
│   ├── storage.rs           # save, get, update, exists, remove
│   ├── helpers.rs           # is_empty_string, has_duplicate_traits, filter_empty_attributes
│   ├── tests.rs             # Unit tests
│   ├── README.md            # Module user guide
│   ├── ARCHITECTURE.md      # Technical architecture
│   └── CHANGELOG.md         # Change history
├── metadata_config.rs       # Max metadata size configuration
├── metadata_repository.rs   # Encapsulated metadata storage (issue #437)
├── metadata_version.rs      # Schema versioning
├── metadata_update_policy.rs # One-time update + admin override
├── metadata_timestamps.rs   # created_at / updated_at tracking
├── metadata_size.rs         # URI byte-length validation (issue #560)
├── metadata_uri_builder.rs  # IPFS URI construction (issue #556)
├── metadata_uri_validator.rs # Protocol validation (issue #561)
└── config/
    ├── ipfs_gateway.rs      # Gateway URL config (issue #477)
    └── metadata_base_uri.rs # Base URI config (issue #476)
```

---

## See Also

- [`STORAGE_ARCHITECTURE.md`](../STORAGE_ARCHITECTURE.md) — Full contract storage layout
- [`METADATA_MODULE_SUMMARY.md`](../METADATA_MODULE_SUMMARY.md) — Module implementation summary
- [`clips_nft/src/metadata/README.md`](../clips_nft/src/metadata/README.md) — Module user guide
- [`clips_nft/src/metadata/ARCHITECTURE.md`](../clips_nft/src/metadata/ARCHITECTURE.md) — Technical deep-dive
