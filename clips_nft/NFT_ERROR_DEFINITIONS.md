# NFT Error Definitions

This document comprehensively describes the standardized error types used throughout the NFT contract, organized into four distinct categories addressing different aspects of NFT operations and state management.

## Overview

The contract uses a standardized error enumeration (`Error` in `types.rs`) with numeric codes to represent all possible error conditions. Each error code is unique and stable across contract versions, enabling reliable off-chain error handling, analytics, and user-facing error messages.

---

## Category 1: Self-Transfer Not Allowed

**Error Code:** 51  
**Enum Variant:** `Error::SelfTransferNotAllowed`  
**Module:** Transfer validation, Marketplace operations  
**Category Code:** `241`

### Description

Prevents operations where the sender and recipient are identical. This error enforces the invariant that an address cannot transfer an NFT to itself, eliminating a class of trivial or malformed transactions.

### Acceptance Criteria

- ✅ Error is defined in the contract type system
- ✅ Used in transfer validation functions
- ✅ Used in marketplace purchase validation
- ✅ Used in offer operations (make_offer, accept_offer)
- ✅ Comprehensive test coverage

### Where This Error Occurs

#### 1. Direct Token Transfer
When attempting `transfer(from, from, token_id)`:
```rust
// In owner_portfolio module
if from == to {
    return Err(Error::SelfTransferNotAllowed);
}
```

#### 2. Marketplace Purchase
When buyer attempts to purchase their own listing:
```rust
// In purchase_validator module
if buyer == listing.seller {
    return Err(Error::SelfTransferNotAllowed);
}
```

#### 3. Making an Offer
When attempting to make an offer on a token you own:
```rust
// In offer_request_validator module
if buyer == owner {
    return Err(Error::SelfTransferNotAllowed);
}
```

#### 4. Accepting an Offer
When the token owner attempts to accept their own offer:
```rust
// In offer_acceptance module
if seller == offer.buyer {
    return Err(Error::SelfTransferNotAllowed);
}
```

### Test Cases

- `test_nft_error_definitions::error_self_transfer_not_allowed_on_transfer` - Direct transfer to self
- `test_nft_error_definitions::error_self_transfer_marketplace_purchase` - Marketplace self-purchase
- `test_nft_error_definitions::error_self_transfer_make_offer_to_own_token` - Offer on own token

### Impact

Prevents logic errors and resource waste while maintaining NFT integrity. Users cannot accidentally (or intentionally) waste gas executing meaningless transfers.

---

## Category 2: Frozen Token Errors

**Error Codes:** 70 (Already Frozen), 71 (Already Unfrozen)  
**Transfer Error Code:** 77 (Invalid Transfer State)  
**Enum Variants:** 
- `Error::AlreadyFrozen`
- `Error::AlreadyUnfrozen`
- `Error::InvalidTransferState`

**Module:** Freeze state guard, Transfer guard, Marketplace operations  
**Category Codes:** `261` (AlreadyFrozen), `262` (AlreadyUnfrozen)

### Description

Frozen tokens are permanently non-transferable NFTs that have been locked by an administrator. When an NFT is frozen, it cannot be:
- Transferred to another wallet
- Listed on the marketplace
- Have its royalty configuration modified
- Be involved in any marketplace operation

Frozen tokens can be unfrozen only by an administrator, restoring their transferability.

### Acceptance Criteria

- ✅ Frozen-token error is defined
- ✅ Used during transfers to prevent movement
- ✅ Used during marketplace operations to prevent listing/purchase
- ✅ Used in royalty configuration to prevent modifications
- ✅ Comprehensive test coverage

### Where This Error Occurs

#### 1. Transfer Validation
Frozen tokens cannot be transferred:
```rust
// In transfer_guard module
pub fn check_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);  // Manifested as Unauthorized
    }
    Ok(())
}
```

#### 2. Marketplace Listing
Frozen tokens cannot be listed:
```rust
// In listing_auth_guard module
if frozen_token::is_frozen(env, token_id) {
    return Err(Error::Unauthorized);
}
```

#### 3. Marketplace Purchase
Frozen tokens cannot be purchased:
```rust
// In purchase_validator module
if frozen_token::is_frozen(env, token_id) {
    return Err(Error::Unauthorized);
}
```

#### 4. Royalty Modification
Royalty cannot be modified on frozen tokens:
```rust
// In freeze_state_guard module
pub fn require_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
```

### Storage Keys

- `DataKey::FrozenToken(token_id)` - Boolean flag indicating frozen state
- Stored in persistent storage for durability across contract invocations

### State Transitions

```
Unfrozen ──(freeze)--> Frozen
Frozen ──(unfreeze)--> Unfrozen
Frozen ──X(transfer)──> (blocked)
Frozen ──X(list)──> (blocked)
Frozen ──X(royalty_mod)──> (blocked)
```

### Test Cases

- `test_nft_error_definitions::error_frozen_token_blocks_transfer` - Cannot transfer frozen token
- `test_nft_error_definitions::error_frozen_token_blocks_marketplace_listing` - Cannot list frozen token
- `test_nft_error_definitions::error_frozen_token_blocks_royalty_modification` - Cannot modify royalty
- `test_nft_error_definitions::error_frozen_token_can_be_unfrozen` - Unfreeze restores operations

### Admin Override

Administrators can bypass frozen-token restrictions to perform emergency operations or recovery:
```rust
// In freeze_state_guard module
pub fn require_not_frozen_or_admin(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) && !admin::is_admin(env, caller) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
```

### Impact

Provides emergency freeze capability for compromised, malicious, or recalled NFTs without requiring contract upgrade or pausing.

---

## Category 3: Duplicate Token Errors

**Error Code:** 84 (DuplicateToken)  
**Minting Error:** 7 (ClipAlreadyMinted)  
**Enum Variants:**
- `Error::DuplicateToken` - Duplicate in batch operations
- `Error::ClipAlreadyMinted` - Duplicate clip ID during mint

**Module:** Batch transfer validation, Minting validation  
**Category Codes:** `293` (DuplicateToken), `242` (ClipAlreadyMinted)

### Description

Prevents duplicate token identifiers from appearing in batch operations or minting requests. Each token and each clip ID must be unique.

### Acceptance Criteria

- ✅ Duplicate token detection implemented
- ✅ Standardized error returned on detection
- ✅ Works in batch transfer validation
- ✅ Works in batch mint validation
- ✅ Comprehensive test coverage

### Where This Error Occurs

#### 1. Batch Transfer Validation
Duplicate token IDs in a batch transfer are rejected:
```rust
// In batch_transfer_validator module
pub fn validate_token_ids(env: &Env, token_ids: &Vec<TokenId>) -> Result<(), Error> {
    let mut seen: Vec<TokenId> = Vec::new(env);
    for token_id in token_ids.iter() {
        if seen.contains(&token_id) {
            return Err(Error::DuplicateToken);
        }
        seen.push_back(token_id);
    }
    Ok(())
}
```

#### 2. Batch Mint Validation
Duplicate clip IDs in a batch mint are rejected:
```rust
// In mint_validator module
if seen_clips.contains(request.clip_id) {
    return Err(Error::ClipAlreadyMinted);
}
seen_clips.push_back(request.clip_id);
```

#### 3. Clip ID Storage
Duplicate clip ID mapping is rejected on individual mints:
```rust
// In clip_id_storage module
pub fn save_clip_id(env: &Env, token_id: TokenId, clip_id: u32) -> Result<(), Error> {
    if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }
    env.storage().persistent().set(&DataKey::ClipIdMinted(clip_id), &token_id);
    Ok(())
}
```

### Detection Algorithm

**Batch Transfer Validation:**
1. Create empty set of seen token IDs
2. Iterate through each token ID in batch
3. If token ID already in set, return `Error::DuplicateToken`
4. Add token ID to set
5. Continue until all validated or error detected
6. Time complexity: O(n²) where n is batch size (small batch limits)
7. Space complexity: O(n)

**Minting Validation:**
1. Create empty set of seen clip IDs
2. For each mint request in batch
3. If clip ID already in set, return `Error::ClipAlreadyMinted`
4. If clip ID already minted on-chain, return `Error::ClipAlreadyMinted`
5. Add clip ID to set
6. Continue until all validated or error detected

### Storage Keys

- `DataKey::ClipIdMinted(clip_id)` - Maps clip ID to its token ID
- `DataKey::TokenClipId(token_id)` - Reverse mapping (token ID to clip ID)
- Stored in persistent storage for durability

### Test Cases

- `test_nft_error_definitions::error_duplicate_token_in_batch_transfer` - Consecutive duplicates
- `test_nft_error_definitions::error_duplicate_token_consecutive_duplicates` - Triple duplicates
- `test_nft_error_definitions::error_clip_already_minted_duplicate_clip_id` - Cross-token clip mapping
- `test_nft_error_definitions::error_duplicate_token_in_batch_at_different_positions` - Non-consecutive duplicates

### Impact

Prevents logical errors and ensures data integrity:
- Prevents transferring same NFT twice in one batch
- Prevents minting same video clip twice
- Maintains one-to-one mapping between tokens and clips
- Improves efficiency by fast-failing on invalid input

---

## Category 4: Invalid State Transitions

**Error Codes:**
- 69: `Error::AlreadyBurned`
- 70: `Error::AlreadyFrozen`
- 71: `Error::AlreadyUnfrozen`
- 72: `Error::InvalidLifecycleTransition`

**Module:** Freeze state guard, Token lifecycle, State validation  
**Category Codes:** `260-263` (Lifecycle)

### Description

Enforces valid NFT lifecycle state transitions. NFTs move through various states (active, frozen, burned) and certain transitions are invalid or operations are invalid in certain states.

### Acceptance Criteria

- ✅ Already burned state detected and rejected from operations
- ✅ Already frozen state detected
- ✅ Already unfrozen state detected
- ✅ Invalid lifecycle transitions prevented
- ✅ Comprehensive test coverage

### Valid NFT Lifecycle States

```
Created (Active)
  ├─ Can transfer
  ├─ Can list
  ├─ Can set royalty
  ├─ Can freeze ──→ Frozen
  └─ Can burn ──→ Burned

Frozen
  ├─ Cannot transfer
  ├─ Cannot list
  ├─ Cannot set royalty
  ├─ Can unfreeze ──→ Active
  └─ Can burn ──→ Burned + Frozen

Burned
  ├─ Cannot transfer (terminal state)
  ├─ Cannot list
  ├─ Cannot unfreeze
  └─ Terminal (no outgoing transitions)
```

### Invalid Transitions

| From State | To State | Reason |
|-----------|----------|--------|
| Burned | Any | Burned is terminal |
| Burned | Unfrozen | Cannot unfrozen what was already destroyed |
| Non-frozen | Unfrozen | Only frozen tokens can be unfrozen |
| Active | Active | Duplicate freeze attempt (operation idempotence guard) |

### Where This Error Occurs

#### 1. Already Burned Prevention
```rust
// Tokens marked burned cannot be operated on
env.storage()
    .persistent()
    .get(&DataKey::BurnedToken(token_id))
    .unwrap_or(false)
// If true, reject operation
```

#### 2. Already Frozen Detection
```rust
pub fn freeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if env.storage().persistent().has(&key) {
        return false;  // Already frozen, return false
    }
    env.storage().persistent().set(&key, &true);
    true  // Now frozen, return true
}
```

#### 3. Already Unfrozen Detection
```rust
pub fn unfreeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if !env.storage().persistent().has(&key) {
        return false;  // Not frozen, return false
    }
    env.storage().persistent().remove(&key);
    true  // Now unfrozen, return true
}
```

#### 4. Invalid Lifecycle Transition
```rust
// General check for invalid state combinations
if is_burned && attempting_frozen_operation {
    return Err(Error::InvalidLifecycleTransition);
}
```

### Test Cases

- `test_nft_error_definitions::error_already_frozen_on_duplicate_freeze` - Freeze idempotence
- `test_nft_error_definitions::error_already_unfrozen_on_duplicate_unfreeze` - Unfreeze non-frozen
- `test_nft_error_definitions::error_already_burned_prevents_operations` - Burned terminal state
- `test_nft_error_definitions::error_invalid_lifecycle_transition_frozen_to_burned` - Multi-state tracking
- `test_nft_error_definitions::error_invalid_lifecycle_transition_multiple_states` - Complex transitions

### Idempotence Guarantees

Operations that modify NFT state are designed to be safe when called multiple times:

```rust
// Freeze: Safe to call multiple times
// First call: returns true, token becomes frozen
// Subsequent calls: return false, state unchanged

// Unfreeze: Safe to call multiple times
// First call on frozen: returns true, token becomes active
// Calls on active: return false, state unchanged
```

### Impact

Maintains NFT invariants and prevents impossible state combinations:
- Ensures burned tokens remain immutable
- Prevents double-freezing causing logic errors
- Prevents unfreezing tokens that weren't frozen
- Maintains historical accuracy of state changes
- Enables idempotent safe operations

---

## Error Code Registry

| Code | Enum Variant | Category | Module | Description |
|------|--------------|----------|--------|-------------|
| 7 | ClipAlreadyMinted | Minting | clip_id_storage | Clip ID already mapped to a token |
| 51 | SelfTransferNotAllowed | Transfer | owner_portfolio | Sender and recipient are identical |
| 69 | AlreadyBurned | Lifecycle | token_lifecycle | Token has been permanently burned |
| 70 | AlreadyFrozen | Lifecycle | frozen_token | Token is already frozen |
| 71 | AlreadyUnfrozen | Lifecycle | frozen_token | Token is not frozen (unfrozen) |
| 72 | InvalidLifecycleTransition | Lifecycle | token_lifecycle | Invalid state transition attempted |
| 77 | InvalidTransferState | Transfer | transfer_guard | Token cannot be transferred in current state |
| 84 | DuplicateToken | Validation | batch_transfer_validator | Token appears multiple times in batch |

---

## Error Handling Best Practices

### For Contract Developers

1. **Check freezing before transfers:**
   ```rust
   frozen_token::is_frozen(env, token_id)?;
   ```

2. **Validate unique recipients:**
   ```rust
   if from == to {
       return Err(Error::SelfTransferNotAllowed);
   }
   ```

3. **Detect duplicate tokens:**
   ```rust
   batch_transfer_validator::validate_token_ids(env, &token_ids)?;
   ```

4. **Handle state transitions safely:**
   ```rust
   let was_frozen = freeze_token(env, token_id);
   if !was_frozen {
       return Err(Error::AlreadyFrozen);
   }
   ```

### For Off-Chain Indexers

1. Listen for error events to track failures
2. Build error counters by code for analytics
3. Alert users to self-transfer attempts (potential UI bug)
4. Track frozen token counts for compliance reporting
5. Monitor duplicate token rejections to detect minting bugs

### For End Users

1. **Self-transfer rejection:** Copy-paste error or accidental duplicate address
2. **Frozen token blocking:** Contact support if token is wrongfully frozen
3. **Duplicate token rejection:** Try batch operation again with unique tokens
4. **State transition failure:** Ensure token is in correct state before operation

---

## Testing Strategy

Comprehensive test coverage is provided in `tests/test_nft_error_definitions.rs`:

### Test Organization

- **Category 1 Tests:** 3 tests covering self-transfer scenarios
- **Category 2 Tests:** 4 tests covering frozen token blocking
- **Category 3 Tests:** 4 tests covering duplicate detection
- **Category 4 Tests:** 5 tests covering state transitions
- **Integration Tests:** 1 test verifying error distinction

### Running Tests

```bash
# Run all error definition tests
cargo test test_nft_error_definitions --

# Run specific test category
cargo test test_nft_error_definitions::error_self_transfer --

# Run with output
cargo test test_nft_error_definitions -- --nocapture
```

---

## Migration & Versioning

All error codes are stable and will not change across contract versions. When new errors are needed:

1. Use the next available code (after 88)
2. Document the error in this file
3. Add comprehensive tests
4. Update the error registry table

---

## References

- **Source Code:** `src/types.rs` - Error enum definition
- **Storage Keys:** `src/types.rs` - DataKey enum
- **Validation Logic:** `src/*_validator.rs` modules
- **Guard Logic:** `src/*_guard.rs` modules
- **Frozen Token Module:** `src/frozen_token.rs`
- **Batch Transfer Validator:** `src/batch_transfer_validator.rs`
- **Clip ID Storage:** `src/clip_id_storage.rs`
- **Test Suite:** `tests/test_nft_error_definitions.rs`

---

## Summary

This error system provides:

✅ **Standardized error codes** for reliable off-chain handling  
✅ **Clear error semantics** for transfer validation  
✅ **Security enforcement** via frozen tokens  
✅ **Data integrity** via duplicate detection  
✅ **State safety** via lifecycle validation  
✅ **Comprehensive testing** of all error paths  
✅ **Detailed documentation** for developers and users  

The four error categories work together to ensure NFT operations are secure, deterministic, and maintainable.
