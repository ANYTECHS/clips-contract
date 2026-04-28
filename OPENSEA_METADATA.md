# OpenSea Metadata Standard Support

This document describes the NFT metadata system extensions that support both static thumbnails and animated previews per token, following the [OpenSea metadata standard](https://docs.opensea.io/docs/metadata-standards) for broad marketplace compatibility.

## Overview

The contract now supports two optional metadata fields on each token:

- **`image`**: Static thumbnail URL
- **`animation_url`**: Animated preview URL

Both fields are optional and backward-compatible with existing tokens.

## Metadata Fields

### `image: Option<String>`

The static thumbnail URL for the token.

- **Purpose**: Provides a fallback image when animations cannot be displayed
- **Recommended formats**: PNG, JPEG, GIF (static), SVG
- **Max size**: 100 MB
- **URL requirements**: Must start with `https://` or `ipfs://`
- **Optional**: Tokens can be minted without this field

### `animation_url: Option<String>`

The animated preview URL for the token.

- **Purpose**: Provides rich media content (video, 3D models, interactive HTML)
- **Recommended formats**: 
  - Video: GIF, MP4 (H.264), WEBM
  - 3D: GLB, GLTF
  - Interactive: HTML
- **Max size**: 100 MB
- **URL requirements**: Must start with `https://` or `ipfs://`
- **Optional**: Tokens can be minted without this field
- **Precedence**: Takes precedence for playback; `image` is used as the fallback thumbnail

## Minting with Metadata

### Basic Mint

```rust
// Mint with both image and animation_url
let token_id = client.mint(
    &owner,
    &clip_id,
    &metadata_uri,
    &Some(String::from_str(&env, "https://example.com/thumbnail.png")),
    &Some(String::from_str(&env, "ipfs://QmAnimation.mp4")),
    &royalty,
    &false,
    &signature
);
```

### Mint with Only Animation

```rust
// Mint with only animation_url (no thumbnail)
let token_id = client.mint(
    &owner,
    &clip_id,
    &metadata_uri,
    &None,  // No image
    &Some(String::from_str(&env, "https://example.com/video.mp4")),
    &royalty,
    &false,
    &signature
);
```

### Mint without Media Fields

```rust
// Mint without image or animation_url (backward compatible)
let token_id = client.mint(
    &owner,
    &clip_id,
    &metadata_uri,
    &None,
    &None,
    &royalty,
    &false,
    &signature
);
```

## Updating Metadata

### Refresh Metadata

The `refresh_metadata` function allows updating metadata fields independently:

```rust
// Update only the image
client.refresh_metadata(
    &admin,
    &token_id,
    &None,  // Keep existing metadata_uri
    &Some(String::from_str(&env, "https://example.com/new-image.png")),
    &None   // Keep existing animation_url
);

// Update only the animation_url
client.refresh_metadata(
    &admin,
    &token_id,
    &None,
    &None,
    &Some(String::from_str(&env, "ipfs://QmNewAnimation.webm"))
);

// Update all fields
client.refresh_metadata(
    &admin,
    &token_id,
    &Some(String::from_str(&env, "ipfs://QmNewMetadata")),
    &Some(String::from_str(&env, "https://example.com/new-image.png")),
    &Some(String::from_str(&env, "ipfs://QmNewAnimation.mp4"))
);

// Clear a field by passing an empty string
client.refresh_metadata(
    &admin,
    &token_id,
    &None,
    &Some(String::from_str(&env, "")),  // Clear image
    &None
);
```

## JSON Output

The `get_metadata_json` function returns OpenSea-compatible JSON:

```rust
let json = client.get_metadata_json(&token_id);
// Returns: {"metadata_uri":"ipfs://QmMetadata","image":"https://example.com/image.png","animation_url":"ipfs://QmAnimation.mp4"}
```

Fields are only included in the JSON output when they are set:

- If `image` is `None`, the `"image"` key is omitted
- If `animation_url` is `None`, the `"animation_url"` key is omitted
- The `"metadata_uri"` key is always present

## URL Validation

Both fields enforce strict URL validation:

### Valid URL Schemes

- `https://` - Secure HTTP URLs
- `ipfs://` - IPFS content-addressed URLs

### Invalid URL Schemes

Any other scheme will be rejected with an error:

- `http://` - Insecure HTTP (rejected)
- `ftp://` - FTP protocol (rejected)
- `data:` - Data URLs (rejected)
- Relative paths (rejected)

### Error Codes

- `Error::InvalidImageUrl` (21) - Image URL does not start with `https://` or `ipfs://`
- `Error::InvalidAnimationUrl` (22) - Animation URL does not start with `https://` or `ipfs://`

## Batch Minting

Batch minting also supports the new fields:

```rust
let mut clip_ids = Vec::new(&env);
clip_ids.push_back(1u32);
clip_ids.push_back(2u32);

let mut uris = Vec::new(&env);
uris.push_back(String::from_str(&env, "ipfs://QmMetadata1"));
uris.push_back(String::from_str(&env, "ipfs://QmMetadata2"));

let mut images = Vec::new(&env);
images.push_back(Some(String::from_str(&env, "https://example.com/image1.png")));
images.push_back(None);

let mut animation_urls = Vec::new(&env);
animation_urls.push_back(Some(String::from_str(&env, "ipfs://QmAnimation1.mp4")));
animation_urls.push_back(Some(String::from_str(&env, "ipfs://QmAnimation2.webm")));

let minted = client.batch_mint(
    &owner,
    &clip_ids,
    &uris,
    &images,
    &animation_urls,
    &royalty,
    &false,
    &signatures
);
```

## Backward Compatibility

All existing tokens minted before this change remain valid and readable:

- Tokens without `image` or `animation_url` fields will have `None` for both
- The `get_metadata_json` function will omit these keys for legacy tokens
- No migration is required for existing tokens

## Marketplace Integration

### OpenSea

OpenSea will automatically recognize and display:

1. The `animation_url` content as the primary media (if present)
2. The `image` as the thumbnail/fallback (if present)
3. The `metadata_uri` for additional metadata

### Other Marketplaces

Most NFT marketplaces follow the OpenSea metadata standard, so these fields should be recognized automatically.

## Best Practices

1. **Always provide an image**: Even if you have an animation, provide a static thumbnail for better compatibility
2. **Use IPFS for permanence**: IPFS URLs ensure content permanence and decentralization
3. **Optimize file sizes**: Keep media under 100 MB for better loading performance
4. **Test URLs before minting**: Ensure URLs are accessible and correctly formatted
5. **Use appropriate formats**: 
   - PNG/JPEG for static images
   - MP4 (H.264) for videos (best compatibility)
   - WEBM for smaller file sizes
   - GLB/GLTF for 3D models

## Examples

### Video NFT

```rust
client.mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "ipfs://QmMetadata"),
    &Some(String::from_str(&env, "ipfs://QmThumbnail.png")),
    &Some(String::from_str(&env, "ipfs://QmVideo.mp4")),
    &royalty,
    &false,
    &signature
);
```

### 3D Model NFT

```rust
client.mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "ipfs://QmMetadata"),
    &Some(String::from_str(&env, "ipfs://QmPreview.png")),
    &Some(String::from_str(&env, "ipfs://QmModel.glb")),
    &royalty,
    &false,
    &signature
);
```

### Interactive HTML NFT

```rust
client.mint(
    &owner,
    &clip_id,
    &String::from_str(&env, "ipfs://QmMetadata"),
    &Some(String::from_str(&env, "ipfs://QmScreenshot.png")),
    &Some(String::from_str(&env, "ipfs://QmInteractive.html")),
    &royalty,
    &false,
    &signature
);
```

## Testing

A comprehensive test suite is available in `clips_nft/tests/test_metadata_fields.rs` that verifies:

- Minting with both fields
- Minting with only one field
- Minting without either field
- URL validation (valid and invalid schemes)
- Metadata refresh operations
- Field clearing with empty strings
- JSON output formatting
- Backward compatibility

Run the tests with:

```bash
cargo test --test test_metadata_fields
```
