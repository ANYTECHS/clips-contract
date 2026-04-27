# TODO: Prepare contract for Soroban's future standards compliance

- [ ] Step 1: Fix compilation bug — malformed duplicate `test_tokens_of_owner_respects_result_limit` nested inside `test_batch_mint_duplicate_clip_id_fails` in `clips_nft/src/lib.rs`
- [ ] Step 2: Add `balance_of` view function to `clips_nft/src/lib.rs`
- [ ] Step 3: Add `token_by_index` enumerable view function to `clips_nft/src/lib.rs`
- [ ] Step 4: Emit `TransferEvent` on mint (from contract address) and burn (to contract address) for ERC-721-style compliance
- [ ] Step 5: Update/add tests for new functions and event counts
- [ ] Step 6: Create `STANDARDS_COMPLIANCE.md` documenting compliance level
- [ ] Step 7: Run `cargo check` and `cargo test` to verify

