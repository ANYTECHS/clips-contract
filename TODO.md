# TODO

## Metadata: Create Metadata Image Structure

- [ ] Add `MetadataImage` struct (image_url, mime_type, width, height) to `clips_nft/src/metadata/types.rs`.
- [ ] Add `thumbnail: Option<MetadataImage>` to `ClipMetadata`.
- [ ] Update constructors in `ClipMetadata` to initialize `thumbnail` to `None`.
- [ ] Update `ClipMetadata::with_full_data` (or add a new constructor) to support thumbnail if needed.
- [ ] Re-export the new struct from `clips_nft/src/metadata/mod.rs`.
- [ ] Add unit tests for cloning/equality and basic field assignment.
- [ ] Run `cargo test` to ensure the codebase compiles and tests pass.

