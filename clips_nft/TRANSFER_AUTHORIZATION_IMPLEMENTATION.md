# Transfer Authorization Error Implementation

This document describes the implementation of standardized errors for transfer authorization and the reusable address event helper.

## Overview

This implementation addresses four key requirements:
1. Define standard error for unauthorized transfer attempts
2. Define error for operators without valid approval
3. Define error for modifying non-existent approvals
4. Create reusable helper for address-related events

## Error Definitions

### 1. UnauthorizedTransfer (Code 270)

**Purpose:** Returned when an account without transfer authorization attempts to move an NFT.

**Location:** `clips_nft/src/types.rs` (Error enum, variant 73) and `clips_nft/src/error_infrastructure/registry.rs` (code 270)

**Usage:**
```rust
if caller != &owner && !operator_approval::is_operator(env, &owner, caller) {
    return Err(Error::UnauthorizedTransfer);
}
```

**Acceptance Criteria Met:**
- ✓ Verifies caller is either owner or approved operator
- ✓ Returns standardized error when neither condition is met
- ✓ Comprehensive tests added in `test_transfer_authorization_errors.rs`

**Test Coverage:**
- `test_unauthorized_transfer_non_owner_non_operator`
- `test_unauthorized_transfer_caller_is_neither_owner_nor_approved_operator`
- `test_owner_can_transfer_without_unauthorized_error`
- `test_approved_operator_can_transfer_without_unauthorized_error`

---

### 2. OperatorNotApproved (Code 271)

**Purpose:** Returned when an operator attempts NFT transfers without valid approval.

**Location:** `clips_nft/src/types.rs` (Error enum, variant 74) and `clips_nft/src/error_infrastructure/registry.rs` (code 271)

**Usage:**
```rust
if !is_operator(env, owner, operator) {
    return Err(Error::OperatorNotApproved);
}
```

**Acceptance Criteria Met:**
- ✓ Detects missing approval before transfer
- ✓ Returns standardized error
- ✓ Comprehensive tests added

**Test Coverage:**
- `test_operator_not_approved_transfer_fails`
- `test_operator_not_approved_after_revocation`
- `test_operator_cannot_equal_owner` (self-approval check)

**Security Considerations:**
- Prevents unauthorized transfers by non-approved operators
- Enforces proper operator approval workflow
- Protects against self-approval (owner as their own operator)

---

### 3. ApprovalNotFound (Code 272)

**Purpose:** Returned when attempting to revoke or modify an approval that does not exist.

**Location:** `clips_nft/src/types.rs` (Error enum, variant 75) and `clips_nft/src/error_infrastructure/registry.rs` (code 272)

**Usage:**
```rust
let approved = get_approval(env, token_id)
    .ok_or(Error::ApprovalNotFound)?;
```

**Acceptance Criteria Met:**
- ✓ Detects missing approval on revocation attempts
- ✓ Returns standardized error
- ✓ Comprehensive tests added

**Test Coverage:**
- `test_revoke_nonexistent_token_approval`
- `test_revoke_nonexistent_operator_approval`
- `test_revoke_already_revoked_approval` (double revocation)
- `test_revoke_operator_after_already_revoked`

**Use Cases:**
- Token approval revocation when no approval exists
- Operator approval revocation when operator not approved
- Double revocation scenarios

---

## Address Event Helper

### 4. Reusable Address Event Helper

**Purpose:** Provides consistent serialization and event emission for Stellar addresses across different event types.

**Location:** `clips_nft/src/address_event_helper.rs`

**Functions:**

#### emit_sender_recipient_event
Emits events involving sender and recipient addresses (e.g., transfers).

```rust
pub fn emit_sender_recipient_event(
    env: &Env,
    topic: Symbol,
    sender: &Address,
    recipient: &Address,
    amount: i128,
    timestamp: u64,
)
```

#### emit_creator_owner_event
Emits events involving creator and owner addresses (e.g., NFT lifecycle).

```rust
pub fn emit_creator_owner_event(
    env: &Env,
    topic: Symbol,
    creator: &Address,
    owner: &Address,
    token_id: u32,
    timestamp: u64,
)
```

#### emit_single_address_event
Emits events involving a single primary address (e.g., approvals, config changes).

```rust
pub fn emit_single_address_event(
    env: &Env,
    topic: Symbol,
    address: &Address,
    amount: i128,
    timestamp: u64,
)
```

**Acceptance Criteria Met:**
- ✓ Supports sender and recipient roles
- ✓ Supports creator and owner roles
- ✓ Serializes addresses consistently as `Address` type
- ✓ Uses consistent topic prefix (`addr_evt`)
- ✓ Comprehensive tests added

**Test Coverage:**
- Sender/recipient event emission and serialization
- Creator/owner event emission and serialization
- Single address event emission and serialization
- Consistent serialization across event types
- Independent event emission
- Same vs. different address handling
- Zero value handling

---

## Error Registry Integration

All three errors are properly registered in `clips_nft/src/error_infrastructure/registry.rs`:

```rust
// ── approval (270–273) ───────────────────────────────────────────────────
ErrorCode {
    module: "approval",
    name: "UnauthorizedTransfer",
    code: 270,
    description: "The caller is not authorized to transfer the NFT.",
},
ErrorCode {
    module: "approval",
    name: "OperatorNotApproved",
    code: 271,
    description: "The caller is not an approved operator for the owner.",
},
ErrorCode {
    module: "approval",
    name: "ApprovalNotFound",
    code: 272,
    description: "No approval exists for the requested token or operator.",
},
```

**Registry Guarantees:**
- ✓ Unique error codes (270, 271, 272)
- ✓ Unique names per error
- ✓ Module-grouped (approval module)
- ✓ Complete documentation strings
- ✓ Contiguous code block

---

## Error Category Classification

All three errors are categorized as **Authorization** errors in `clips_nft/src/error_catalog/categories.rs`:

- **UnauthorizedTransfer** → Authorization (caller lacks permission)
- **OperatorNotApproved** → Authorization (missing approval/allow-list entry)
- **ApprovalNotFound** → Authorization (approval state validation)

This categorization helps indexers and clients bucket failures appropriately.

---

## Testing Strategy

### Test File: `clips_nft/tests/test_transfer_authorization_errors.rs`

**Coverage:**
1. **UnauthorizedTransfer Tests:**
   - Non-owner, non-operator attempts
   - Verification that owners can transfer
   - Verification that approved operators can transfer

2. **OperatorNotApproved Tests:**
   - Unapproved operator transfer attempts
   - Operator after revocation
   - Self-approval prevention

3. **ApprovalNotFound Tests:**
   - Revoking non-existent token approval
   - Revoking non-existent operator approval
   - Double revocation scenarios

4. **Combined Scenarios:**
   - Error priority (UnauthorizedTransfer takes precedence)
   - Successful approval flow without errors

### Test File: `clips_nft/tests/test_address_event_helper.rs`

**Coverage:**
1. **Sender/Recipient Events:**
   - Event publication
   - Correct serialization
   - Same vs. different addresses

2. **Creator/Owner Events:**
   - Event publication
   - Correct serialization
   - Creator equals owner scenarios

3. **Single Address Events:**
   - Event publication
   - Correct serialization

4. **Consistency Tests:**
   - Multiple independent events
   - Consistent serialization across types
   - Consistent topic prefix
   - Zero value handling

---

## Integration Points

### Where These Errors Are Used

**UnauthorizedTransfer:**
- `src/token_lifecycle.rs` - Burn authorization check
- `src/transfer_auth_guard.rs` - Transfer authorization guard

**OperatorNotApproved:**
- `src/transfer_guard.rs` - Operator validation in transfers
- `src/operator_validator.rs` - Operator combination validation

**ApprovalNotFound:**
- `src/token_approval.rs` - Token approval revocation
- `src/operator_approval.rs` - Operator approval revocation

### Where Address Event Helper Is Used

The address event helper is designed for use in:
- Transfer events (sender → recipient)
- Mint events (creator + owner)
- Burn events (previous owner)
- Approval events (single address)
- Royalty payment events (multiple recipients)

---

## Compliance with Error Naming Convention

All errors follow the documented naming convention (`docs/error-naming-convention.md`):

| Rule | UnauthorizedTransfer | OperatorNotApproved | ApprovalNotFound |
|------|---------------------|---------------------|------------------|
| PascalCase | ✓ | ✓ | ✓ |
| Subject-first | ✓ (Unauthorized Transfer) | ✓ (Operator NotApproved) | ✓ (Approval NotFound) |
| Full words | ✓ | ✓ | ✓ |
| Pattern match | `Unauthorized*` | `*NotApproved` | `*NotFound` |
| Unique name | ✓ | ✓ | ✓ |
| One-sentence doc | ✓ | ✓ | ✓ |

---

## Running Tests

Execute the comprehensive test suite:

```bash
# Run all transfer authorization tests
cargo test test_transfer_authorization_errors --package clips_nft

# Run all address event helper tests
cargo test test_address_event_helper --package clips_nft

# Run specific test
cargo test test_unauthorized_transfer_non_owner_non_operator --package clips_nft

# Run with output
cargo test test_operator_not_approved_transfer_fails --package clips_nft -- --nocapture
```

---

## Summary

This implementation provides:

1. **Three standardized authorization errors** with unique codes in the 270-272 range
2. **Complete error registry integration** with proper documentation
3. **Reusable address event helper** with consistent serialization
4. **Comprehensive test coverage** (30+ test cases)
5. **Proper categorization** as Authorization errors
6. **Full compliance** with error naming conventions

All acceptance criteria have been met:
- ✓ Caller verification
- ✓ Standardized error returns
- ✓ Comprehensive tests
- ✓ Address serialization consistency
- ✓ Multiple address role support

---

## Related Documentation

- `docs/error-naming-convention.md` - Error naming and documentation standards
- `docs/error-catalog.md` - Complete error catalog and code allocation
- `clips_nft/src/address_event_helper.rs` - Address event helper implementation
- `clips_nft/src/error_infrastructure/registry.rs` - Centralized error registry
