# Metadata Module Architecture

## Module Structure

```
clips_nft/src/metadata/
│
├── mod.rs                  # Module root, exports, and documentation
├── types.rs                # Type definitions (Attribute, TokenMetadata)
├── validation.rs           # Validation logic and rules
├── storage.rs              # Storage operations
├── helpers.rs              # Utility functions
├── tests.rs                # Test suite
├── README.md               # User documentation
├── CHANGELOG.md            # Change history
└── ARCHITECTURE.md         # This file
```

## Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Metadata Module (mod.rs)                 │
│                      Public Interface                        │
└─────────────────────────────────────────────────────────────┘
                              │
                ┌─────────────┼─────────────┐
                │             │             │
                ▼             ▼             ▼
        ┌──────────┐  ┌─────────────┐  ┌─────────┐
        │  Types   │  │ Validation  │  │ Storage │
        │          │  │             │  │         │
        │ Attribute│  │validate_url │  │save_    │
        │ Token    │  │validate_uri │  │get_     │
        │ Metadata │  │validate_*   │  │update_  │
        └──────────┘  └─────────────┘  └─────────┘
                              │
                              ▼
                        ┌─────────┐
                        │ Helpers │
                        │         │
                        │is_empty │
                        │clear_   │
                        │filter_  │
                        └─────────┘
```

## Data Flow

### Minting Flow
```
1. Contract receives mint request with metadata
   │
   ▼
2. Validation Module validates all fields
   │ - validate_metadata_uri()
   │ - validate_image_url()
   │ - validate_animation_url()
   │ - validate_attributes()
   │
   ▼
3. Storage Module persists metadata
   │ - save_metadata()
   │
   ▼
4. Token minted with validated metadata
```

### Retrieval Flow
```
1. Contract receives query for token metadata
   │
   ▼
2. Storage Module retrieves from persistent storage
   │ - get_metadata()
   │
   ▼
3. Helpers format/transform if needed
   │ - build_metadata_json()
   │
   ▼
4. Return metadata to caller
```

### Update Flow
```
1. Contract receives metadata update request
   │
   ▼
2. Validation Module validates new fields
   │
   ▼
3. Storage Module updates existing metadata
   │ - update_metadata()
   │
   ▼
4. Metadata updated successfully
```

## Type Relationships

```
TokenMetadata
├── metadata_uri: String (required)
├── image: Option<String>
├── animation_url: Option<String>
├── description: Option<String>
├── external_url: Option<String>
└── attributes: Vec<Attribute>
    └── Attribute
        ├── trait_type: String
        └── value: String
```

## Validation Rules

```
┌─────────────────────────────────────────────────────┐
│                  Validation Layer                    │
├─────────────────────────────────────────────────────┤
│                                                      │
│  URL Validation                                      │
│  ├── Protocol Check (https://, ipfs://, ar://)     │
│  ├── Length Check (max 512 chars)                  │
│  └── Format Check (basic malformation detection)    │
│                                                      │
│  Description Validation                             │
│  └── Length Check (max 1000 chars)                 │
│                                                      │
│  Attributes Validation                              │
│  ├── Count Check (max 50 attributes)               │
│  ├── trait_type Length (max 64 chars)              │
│  ├── value Length (max 128 chars)                  │
│  └── Non-empty Check                               │
│                                                      │
└─────────────────────────────────────────────────────┘
```

## Storage Layout

```
Persistent Storage:
┌──────────────────────────────────────┐
│ DataKey::Metadata(token_id)          │
│ └─> String (metadata_uri)            │
└──────────────────────────────────────┘

Future Extension (not yet implemented):
┌──────────────────────────────────────┐
│ DataKey::TokenData(token_id)         │
│ └─> TokenMetadata (full struct)      │
└──────────────────────────────────────┘
```

## Error Handling

```
Validation Errors:
├── Error::InvalidURI
│   ├── Empty URI
│   ├── URI too long
│   └── Malformed URI
├── Error::UnsupportedProtocol
│   └── Protocol not in whitelist
├── Error::MalformedUrl
│   └── URL format invalid
└── Error::TokenNotFound
    └── Token doesn't exist
```

## Integration Points

```
┌────────────────────┐
│   Main Contract    │
│     (lib.rs)       │
└────────┬───────────┘
         │
         │ pub use metadata::*;
         │
         ▼
┌────────────────────┐
│ Metadata Module    │
│  Public Interface  │
├────────────────────┤
│ - Attribute        │
│ - TokenMetadata    │
│ - validate_*()     │
│ - save_metadata()  │
│ - get_metadata()   │
└────────────────────┘
         │
         │ Used by:
         │
    ┌────┴─────┬──────────┬─────────────┐
    │          │          │             │
    ▼          ▼          ▼             ▼
 mint()   refresh_   get_token_   batch_mint()
        metadata()     data()
```

## Design Principles

1. **Separation of Concerns**
   - Types: Data structures only
   - Validation: Business rules only
   - Storage: Persistence only
   - Helpers: Utilities only

2. **Single Responsibility**
   - Each module has one clear purpose
   - Functions are focused and specific

3. **Open/Closed Principle**
   - Open for extension (new validators, new fields)
   - Closed for modification (stable interfaces)

4. **Dependency Direction**
   - All modules depend on types
   - No circular dependencies
   - Clear dependency hierarchy

5. **Error Handling**
   - Consistent error types
   - Early validation
   - Clear error messages

## Security Considerations

1. **Input Validation**
   - All inputs validated before storage
   - Length limits prevent DoS attacks
   - Protocol whitelist prevents malicious URLs

2. **Storage Safety**
   - Persistent storage for critical data
   - Existence checks before updates
   - Clean removal on burn

3. **Type Safety**
   - Strong typing prevents errors
   - Option types for optional fields
   - Result types for fallible operations

## Performance Considerations

1. **Storage Efficiency**
   - Only URI stored by default
   - Full metadata optional
   - Minimal storage footprint

2. **Validation Performance**
   - Fast protocol checks
   - Early rejection of invalid input
   - No heavy computations

3. **Future Optimizations**
   - Caching for frequently accessed metadata
   - Batch operations for multiple tokens
   - Lazy loading of optional fields

## Extension Points

1. **New Validators**
   - Add new validation functions to `validation.rs`
   - Export via `mod.rs`
   - Document in README

2. **New Types**
   - Add new structs to `types.rs`
   - Maintain backward compatibility
   - Version appropriately

3. **New Storage Operations**
   - Add new functions to `storage.rs`
   - Follow existing patterns
   - Maintain consistency

4. **New Helpers**
   - Add utility functions to `helpers.rs`
   - Keep functions pure when possible
   - Document thoroughly

## Testing Strategy

1. **Unit Tests**
   - Test each function in isolation
   - Cover edge cases
   - Test error conditions

2. **Integration Tests**
   - Test module interactions
   - Test with real Soroban environment
   - Test full workflows

3. **Property Tests**
   - Test validation invariants
   - Test storage consistency
   - Test helper idempotency

## Future Enhancements

1. **Phase 1: Core Functionality**
   - ✅ Basic types
   - ✅ URL validation
   - ✅ Storage operations
   - ✅ Basic helpers

2. **Phase 2: Enhanced Features**
   - ⏳ JSON serialization
   - ⏳ Advanced URL normalization
   - ⏳ Duplicate detection
   - ⏳ Comprehensive tests

3. **Phase 3: Advanced Features**
   - 🔮 Metadata caching
   - 🔮 Batch operations
   - 🔮 Versioning support
   - 🔮 Content validation

4. **Phase 4: Optimization**
   - 🔮 Performance tuning
   - 🔮 Storage optimization
   - 🔮 Gas optimization
   - 🔮 Advanced indexing

Legend: ✅ Complete | ⏳ In Progress | 🔮 Planned
