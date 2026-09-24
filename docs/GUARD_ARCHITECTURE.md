# ClipCash NFT Guard Architecture

Developer reference documenting the authorization and validation guard layer
in the `clips_nft` Soroban smart contract. This guide covers every guard
module, its responsibility, the execution order for each protected operation,
a mapping of which guards protect which contract functions, and contribution
examples for future maintainers.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Guard Categories](#guard-categories)
3. [Guard Reference](#guard-reference)
4. [Guard Execution Order](#guard-execution-order)
5. [Protected Contract Functions](#protected-contract-functions)
6. [Error Reference](#error-reference)
7. [Storage Backing](#storage-backing)
8. [Examples for Contributors](#examples-for-contributors)

---

## Architecture Overview

The contract uses a **layered guard architecture** — thin contract entry points
delegate authorization and validation checks to dedicated guard modules before
any state is mutated. This separation ensures:

- **Centralized policy** — every entry point reuses the same guard functions,
  so changing an authorization rule in one place updates all callers.
- **Fail-fast semantics** — all storage mutations are gated behind a
  pass/fail check that returns a typed [`Error`] before any write occurs.
- **Composable layers** — guards can be chained (e.g. pause check + admin
  check + freeze check) without duplicating logic.

```
┌──────────────────────────────────────────────────────────┐
│  Contract Entry Point (lib.rs / atomic_mint.rs)          │
│  ClipsNftContract / AtomicMintContract                    │
└──────────────────────────┬───────────────────────────────┘
                           │ calls
                           ▼
┌──────────────────────────────────────────────────────────┐
│  Guard Layer (guard modules)                             │
│  ┌─ Admin authorization    │ config_guard, storage_guard  │
│  ├─ Pause enforcement      │ pause_guard, royalty_pause_guard │
│  ├─ Transfer validation    │ transfer_guard               │
│  ├─ Mint authorization     │ mint_authorization           │
│  ├─ Royalty admin          │ royalty_admin_guard          │
│  ├─ Royalty pipeline       │ royalty_validation_pipeline   │
│  ├─ Marketplace listing    │ listing_validator            │
│  └─ Marketplace purchase   │ purchase_validator           │
└──────────────────────────┬───────────────────────────────┘
                           │ reads / writes
                           ▼
┌──────────────────────────────────────────────────────────┐
│  Storage / Domain Modules                                │
│  administrator_storage · pause_state · token_owner_storage │
│  blacklist · frozen_token · token_approval · operator_approval │
│  creator_storage · royalty_storage · payment_currency    │
└──────────────────────────────────────────────────────────┘
```

---

## Guard Categories

| Category | Guards | Responsibility |
|----------|--------|----------------|
| **Admin authorization** | `config_guard::require_config_admin`, `royalty_admin_guard::require_royalty_admin`, `storage_guard::guard_admin` | Verify the caller is a registered administrator before configuration, royalty, or storage mutations. |
| **Pause enforcement** | `pause_guard::require_not_paused`, `royalty_pause_guard::require_royalty_not_paused`, `storage_guard::guard_not_paused` | Block all state-changing operations when the contract circuit-breaker is active. |
| **Transfer validation** | `transfer_guard::check_transfer` (+ sub-guards) | Run the full pre-condition suite before any NFT ownership change. |
| **Mint authorization** | `mint_authorization::require_mint_auth` | Verify the caller is the contract admin or an approved minter before minting. |
| **Royalty lifecycle** | `royalty_freeze::require_not_frozen`, `royalty_emergency::require_payments_enabled`, `royalty_validation_pipeline::validate_royalty_operation` | Protect royalty configuration updates, payments, and emergency toggles. |
| **Marketplace validation** | `listing_validator::validate_listing`, `purchase_validator::validate_purchase` | Validate listing creation and purchase pre-conditions, including pause state. |
| **Owner verification** | `token_owner_storage::verify_owner`, `storage_guard::guard_token_owner` | Confirm the caller owns the token involved in an operation. |

---

## Guard Reference

### 1. `pause_guard::require_not_paused`

**File:** [`clips_nft/src/pause_guard.rs`](../clips_nft/src/pause_guard.rs)

**Responsibility:** The global circuit-breaker guard. Every state-changing
entry point that respects the pause flag calls this as its first check.

**Checks:** Reads `DataKey::Paused` from instance storage; if `true`, returns
`Error::ContractPaused`.

**Returns on success:** `Ok(())`

**Errors:**
| Error | When |
|-------|------|
| `ContractPaused` | `DataKey::Paused` is `true` in instance storage. |

**Usage:**
```rust
pause_guard::require_not_paused(&env)?;
```

---

### 2. `config_guard::require_config_admin`

**File:** [`clips_nft/src/config_guard.rs`](../clips_nft/src/config_guard.rs)

**Responsibility:** The primary admin-authorization guard for configuration and
privileged operations. Verifies the contract is initialized, the caller matches
the stored admin, and the caller has signed the invocation.

**Checks (in order):**
1. `DataKey::Admin` exists in instance storage → `Error::NotInitialized` if absent.
2. `caller == admin` → `Error::UnauthorizedConfigurationUpdate` if mismatch.
3. `caller.require_auth()` — Soroban authentication.

**Returns on success:** `Ok(())`

**Errors:**
| Error | When |
|-------|------|
| `NotInitialized` | `DataKey::Admin` is not set in instance storage. |
| `UnauthorizedConfigurationUpdate` | `caller` does not match the stored admin. |

**Usage:**
```rust
config_guard::require_config_admin(&env, &caller)?;
```

---

### 3. `transfer_guard::check_transfer`

**File:** [`clips_nft/src/transfer_guard.rs`](../clips_nft/src/transfer_guard.rs)

**Responsibility:** The composite transfer guard. Runs all transfer
pre-conditions in a fixed order and returns on the first failure. Individual
sub-guards are also exported for testing and reuse.

**Arguments:** `env`, `caller`, `from`, `to`, `token_id`

**Returns on success:** `Ok(())`

**Errors:**
| Error | When |
|-------|------|
| `TokenNotFound` | Token does not exist or `from` is not its owner. |
| `SelfTransferNotAllowed` | `from` and `to` are the same address. |
| `Unauthorized` | Token is frozen, or caller is not owner/approved/admin. |
| `InvalidAddress` | `from` or `to` is blacklisted. |
| `InvalidRecipient` | `to` is the contract's own address. |

**Sub-guards:**
| Function | Issue | Condition |
|----------|-------|-----------|
| `check_not_frozen` | #727 | Token is not frozen. |
| `check_not_blacklisted` | #728 | Neither sender nor recipient is blacklisted. |
| `check_valid_recipient` | #724 | Destination is not the contract address. |
| `check_not_self_transfer` | — | Sender ≠ recipient. |
| `check_caller_authorized` | #730 / #731 | Caller is owner, single-token approver, operator, or admin. |

**Usage:**
```rust
transfer_guard::check_transfer(&env, &caller, &from, &to, token_id)?;
```

---

### 4. `royalty_admin_guard::require_royalty_admin`

**File:** [`clips_nft/src/royalty_admin_guard.rs`](../clips_nft/src/royalty_admin_guard.rs)

**Responsibility:** Validates that the caller is a registered administrator
(via `administrator_storage::is_admin`), as opposed to the single contract
owner stored at `DataKey::Admin`. Used by royalty administrative operations
that delegate to the multi-admin registry.

**Checks:**
1. `caller.require_auth()` — Soroban authentication.
2. `administrator_storage::is_admin(env, caller)` → `Error::Unauthorized` if false.

**Errors:**
| Error | When |
|-------|------|
| `Unauthorized` | Caller is not in the `Administrator` registry. |

**Usage:**
```rust
royalty_admin_guard::require_royalty_admin(env, &caller)?;
```

---

### 5. `royalty_pause_guard::require_royalty_not_paused`

**File:** [`clips_nft/src/royalty_pause_guard.rs`](../clips_nft/src/royalty_pause_guard.rs)

**Responsibility:** Domain-specific pause check for royalty state-changing
operations. Reads the same global pause flag as `pause_guard` but is intended
for royalty-specific entry points.

**Checks:** `get_pause_state(env)` — if `true`, returns `Error::ContractPaused`.

**Errors:**
| Error | When |
|-------|------|
| `ContractPaused` | Global pause flag is `true`. |

**Usage:**
```rust
royalty_pause_guard::require_royalty_not_paused(env)?;
```

---

### 6. `royalty_freeze::require_not_frozen`

**File:** [`clips_nft/src/royalty_freeze.rs`](../clips_nft/src/royalty_freeze.rs)

**Responsibility:** Prevents modifications to a token's royalty configuration
once it has been permanently frozen.

**Checks:** Reads `DataKey::RoyaltyFrozen(token_id)` from persistent storage;
if `true`, returns `Error::RoyaltyFrozen`.

**Errors:**
| Error | When |
|-------|------|
| `RoyaltyFrozen` | `DataKey::RoyaltyFrozen(token_id)` is `true`. |

**Usage:**
```rust
royalty_freeze::require_not_frozen(env, token_id)?;
```

---

### 7. `royalty_emergency::require_payments_enabled`

**File:** [`clips_nft/src/royalty_emergency.rs`](../clips_nft/src/royalty_emergency.rs)

**Responsibility:** Enforces the emergency toggle that disables all royalty
payment distribution. Checked at the top of `pay_royalty`.

**Checks:** Reads `DataKey::RoyaltyPaymentsDisabled` from instance storage;
if `true`, returns `Error::RoyaltyPaymentsDisabled`.

**Errors:**
| Error | When |
|-------|------|
| `RoyaltyPaymentsDisabled` | `DataKey::RoyaltyPaymentsDisabled` is `true`. |

**Usage:**
```rust
royalty_emergency::require_payments_enabled(env)?;
```

---

### 8. `royalty_validation_pipeline::validate_royalty_operation`

**File:** [`clips_nft/src/royalty_validation_pipeline.rs`](../clips_nft/src/royalty_validation_pipeline.rs)

**Responsibility:** Centralized 5-stage validation pipeline for all royalty
configuration operations. Individual stages are also exported for partial
validation.

**Stages (executed in order):**
1. **Pause state** — `validate_contract_not_paused` → `require_not_paused`.
2. **Token existence** — `validate_token_exists` → `Error::TokenNotFound`.
3. **Caller authorization** — `authorize_royalty_update` (admin, creator, or owner).
4. **Royalty state** — `validate_royalty_state` → rejects frozen configs.
5. **Configuration validity** — `validate_royalty_configuration` (bps, recipients, asset).

**Errors:**
| Error | When |
|-------|------|
| `ContractPaused` | Contract is paused (stage 1). |
| `TokenNotFound` | No royalty config for the token (stage 2). |
| `UnauthorizedConfigurationUpdate` | Caller is admin/creator/owner (stage 3). |
| `RoyaltyFrozen` | Royalty config is frozen (stage 4). |
| `InvalidBasisPoints` | bps out of 0–10 000 range (stage 5). |
| `InvalidRecipient` | Recipient is the contract address (stage 5). |
| `UnsupportedAsset` | Payment asset not in supported currencies (stage 5). |

**Usage:**
```rust
validate_royalty_operation(env, &caller, token_id, &new_royalty)?;
```

---

### 9. `mint_authorization::require_mint_auth`

**File:** [`clips_nft/src/mint_authorization.rs`](../clips_nft/src/mint_authorization.rs)

**Responsibility:** Validates that the caller is authorized to mint NFTs.
Authorization is granted to the contract admin or an explicitly approved minter.

**Checks (in order):**
1. Contract is initialized (`DataKey::Admin` exists) → `Error::NotInitialized`.
2. Caller is the contract owner → pass + `require_auth`.
3. Caller is an approved minter (`DataKey::ApprovedMinter(address)`) → pass + `require_auth`.
4. Otherwise → `Error::UnauthorizedMinter`.

**Errors:**
| Error | When |
|-------|------|
| `NotInitialized` | `DataKey::Admin` is absent. |
| `UnauthorizedMinter` | Caller is neither admin nor an approved minter. |

**Usage:**
```rust
mint_authorization::require_mint_auth(&env, &caller)?;
```

---

### 10. `listing_validator::validate_listing`

**File:** [`clips_nft/src/marketplace/listing_validator.rs`](../clips_nft/src/marketplace/listing_validator.rs)

**Responsibility:** Pre-condition checks for marketplace listing creation and
modification. Also re-exports `cancel_listing` which validates cancellation
authorization.

**Checks (in order):**
1. Contract not paused → `ContractPaused`.
2. NFT exists and caller is the owner → `TokenNotFound` / `Unauthorized`.
3. Price is positive and within bounds → `InvalidSalePrice` / `PriceOverflow`.
4. Payment asset is supported → `UnsupportedAsset`.
5. No active listing already exists → `DuplicateRecord`.
6. Expiration is in the future (if set) → `InvalidConfig`.

**Usage:**
```rust
listing_validator::validate_listing(env, &seller, token_id, price, &asset, expires_at)?;
```

---

### 11. `purchase_validator::validate_purchase`

**File:** [`clips_nft/src/marketplace/purchase_validator.rs`](../clips_nft/src/marketplace/purchase_validator.rs)

**Responsibility:** Pre-condition checks for NFT purchases. Validates listing
liveness, payment, buyer eligibility, and seller ownership before the sale
settlement is executed.

**Checks (in order):**
1. Contract not paused → `ContractPaused`.
2. Listing is `Active` → `ListingNotActive`.
3. Listing not expired → `OfferExpired`.
4. Buyer ≠ seller → `SelfTransferNotAllowed`.
5. Buyer not blacklisted → `Unauthorized`.
6. Payment asset matches listing and is supported → `UnsupportedAsset`.
7. Payment amount ≥ listing price → `InvalidSalePrice`.
8. Seller is current on-chain owner → `Unauthorized`.
9. Token is not frozen → `Unauthorized`.

**Usage:**
```rust
purchase_validator::validate_purchase(env, &buyer, &listing, &payment_asset, payment_amount)?;
```

---

### 12. `storage_guard`

**File:** [`clips_nft/src/storage_guard.rs`](../clips_nft/src/storage_guard.rs)

**Responsibility:** Lower-level storage-access guards used primarily by
internal storage helpers. Provides three functions with simpler logic than
the primary guards above. These are used in contexts where full guard modules
are not yet wired up or for direct storage-layer protection.

| Function | Checks | Errors |
|----------|--------|--------|
| `guard_admin(env, caller)` | `DataKey::Admin` exists and matches `caller`; `require_auth`. | `NotInitialized`, `Unauthorized` |
| `guard_not_paused(env)` | `DataKey::Paused` is `false`. | `ContractPaused` |
| `guard_token_owner(env, caller, token_id)` | `DataKey::Token(token_id)` exists and `owner == caller`; `require_auth`. | `TokenNotFound`, `Unauthorized` |

> **Note:** `storage_guard` overlaps with `config_guard` and `pause_guard`.
> New code should prefer `config_guard::require_config_admin` and
> `pause_guard::require_not_paused`. `storage_guard` is retained for
> backward compatibility and internal storage-layer checks.

---

## Guard Execution Order

Each protected operation runs guards in a deterministic order. The order is
important because later checks depend on earlier ones passing, and because
some checks are cheaper than others (e.g. pause check vs. storage reads).

### Contract-wide state mutations

```
Entry point (lib.rs) ──→ config_guard::require_config_admin ──→ [domain logic]
   ↓
Operations: set_default_royalty_bps, pause, unpause,
            add_currency, remove_currency, set_royalty,
            freeze_token, unfreeze_token, reassign_creator
```

The pause/unpause commands additionally call `pause_state::get_pause_state`
to ensure idempotency.

### Marketplace listing operations

```
Entry point ──→ caller.require_auth ──→ pause_guard::require_not_paused
   ↓
list_nft        → listing_validator::validate_listing → [create + emit]
update_listing  → listing_storage::get_listing → owner check → [update + emit]
cancel_listing  → listing_storage::get_listing → owner/operator/admin check → [cancel + emit]
```

### Marketplace purchase / offer operations

```
Entry point ──→ caller.require_auth ──→ pause_guard::require_not_paused
   ↓
buy_listing    → purchase_validator::validate_purchase → [settlement]
make_offer     → (inline validation: price, asset, expiration) → [store + emit]
accept_offer   → pause_guard::require_not_paused → [load offer → validate → settlement]
cancel_offer   → caller.require_auth → (operator/buyer check) → [remove + emit]
```

### NFT transfer operations

```
Entry point ──→ caller.require_auth
   ↓
transfer_guard::check_transfer:
  1. Token exists & from is owner  (token_owner_storage::get_owner)
  2. Not self-transfer            (check_not_self_transfer)
  3. Token not frozen             (check_not_frozen via frozen_token)
  4. Neither party blacklisted    (check_not_blacklisted via blacklist)
  5. Caller authorized            (check_caller_authorized:
                                    owner / single-token approval /
                                    operator approval / admin)
  6. Recipient valid              (check_valid_recipient: not contract self)
```

### Royalty configuration operations

```
Entry point ──→ [royalty_updater::update_royalty_configuration]
   ↓
  1. pause_guard::require_not_paused          (circuit-breaker)
  2. validate_state_for_update                 (token exists + not frozen)
  3. authorize_royalty_update                  (admin / creator / owner)
  4. validate_royalty                          (bps, recipients, struct)
  5. validate_royalty_asset                    (asset is supported)
  6. validate_royalty_recipient_struct         (per-recipient checks)
  7. Persistence: set_royalty + indexes
```

### Royalty payment operations

```
Entry point ──→ royalty_payment::pay_royalty
   ↓
  1. royalty_emergency::require_payments_enabled  (emergency toggle)
  2. sale_price > 0                                (inline)
  3. token_storage::get_royalty                    (token exists)
  4. mark_replay                                   (replay protection)
  5. validate_royalty_recipients                   (recipient validity)
  6. validate_royalty_asset                        (asset support)
  7. transaction_deduction_validator               (total ≤ 100%)
  8. Distribution loop + event emission
```

### Mint operations

```
Entry point ──→ mint_authorization::require_mint_auth
   ↓
  1. Contract initialized (DataKey::Admin exists)
  2. Caller is admin OR approved minter
  3. Caller not blacklisted
  4. execute_atomic_mint:
     a. validate_owner (not contract address)
     b. signature replay check
     c. validate_mint (clip dedup, metadata, royalty)
     d. storage_validator (metadata URI, royalty cap)
     e. [Phase 2 writes with rollback]
```

---

## Protected Contract Functions

The table below maps every admin-gated or guarded entry point in the contract
to the guard(s) that protect it. Source references are to `lib.rs` line numbers.

### Admin-only functions (`config_guard::require_config_admin`)

| Function | lib.rs | Guard Applied | Purpose |
|----------|--------|---------------|---------|
| `set_default_royalty_bps` | 341 | `require_config_admin` | Set contract-wide default royalty bps. |
| `pause` | 365 | `require_config_admin` + `pause_state::get_pause_state` | Pause the contract (circuit-breaker). |
| `unpause` | 389 | `require_config_admin` + `pause_state::get_pause_state` | Resume the contract. |
| `add_currency` | 441 | `require_config_admin` | Register a supported payment asset. |
| `remove_currency` | 450 | `require_config_admin` | Deregister a payment asset. |
| `set_royalty` | 508 | `require_config_admin` + freeze check | Set per-token royalty config. |
| `freeze_token` | 535 | `require_config_admin` | Permanently freeze an NFT (soulbound). |
| `unfreeze_token` | 555 | `require_config_admin` | Remove frozen status from an NFT. |
| `reassign_creator` | 581 | `require_config_admin` | Change a token's original creator. |
| `set_royalty_payments_disabled` | 477 | `require_config_admin` (via `royalty_emergency`) | Toggle emergency royalty payment disable. |

### Pause-guarded functions (`pause_guard::require_not_paused`)

| Function | lib.rs | Guard Applied | Purpose |
|----------|--------|---------------|---------|
| `update_listing` | 666 | `require_not_paused` | Modify an active listing's price/expiration. |
| `buy_listing` | 718 | `require_not_paused` | Purchase a listed NFT. |
| `make_offer` | 780 | `require_not_paused` | Place a buy offer on a token. |
| `accept_offer` | 830 | `require_not_paused` | Accept a buyer's offer. |

### Transfer-authorized functions

| Function | lib.rs | Guard Applied | Purpose |
|----------|--------|---------------|---------|
| `revoke_approval` | 413 | `owner.require_auth` + `token_owner_storage::verify_owner` | Revoke single-token approval. |
| `revoke_operator_approval` | 430 | `owner.require_auth` | Revoke operator approval. |
| `list_nft` | 617 | `seller.require_auth` + `listing_validator::validate_listing` | List NFT for sale. |
| `create_listing` | 622 | `listing.seller.require_auth` + `token_owner_storage::verify_owner` | Create a marketplace listing. |
| `cancel_listing` | 644 | `seller.require_auth` + owner check | Cancel an active listing. |
| `buy_listing` | 718 | `buyer.require_auth` + `purchase_validator::validate_purchase` | Execute a purchase. |
| `accept_offer` | 830 | `seller.require_auth` + owner verification | Accept an offer and transfer NFT. |
| `cancel_offer` | 884 | `caller.require_auth` + operator/buyer check | Cancel a pending offer. |

### Royalty lifecycle functions

| Function | lib.rs | Guard Applied | Purpose |
|----------|--------|---------------|---------|
| `freeze_royalty` | 919 | `authorize_royalty_update` | Permanently freeze royalty config. |
| `is_royalty_frozen` | 924 | *(read-only, no guard)* | Query royalty freeze state. |
| `update_royalty` | 933 | `pause_guard::require_not_paused` (via `royalty_updater`) | Update per-token royalty config. |
| `pay_royalty` | 468 | `require_payments_enabled` | Process secondary sale royalty payment. |
| `royalty_info` | 490 | *(read-only, no guard)* | Preview royalty amount for a sale. |

### Mint functions

| Function | Module | Guard Applied | Purpose |
|----------|--------|---------------|---------|
| `AtomicMintContract::mint` | atomic_mint.rs:290 | `require_mint_auth` + blacklist check | Mint a single NFT atomically. |
| `AtomicMintContract::batch_mint` | atomic_mint.rs:458 | `require_mint_auth` + blacklist check | Mint multiple NFTs atomically. |
| `ClipsNftContract::mint` | lib.rs:40 | `require_mint_auth` + `NotInitialized` check | Main contract mint entry point. |

---

## Error Reference

| Error Variant | Code | Guard(s) That Can Return It | Meaning |
|---------------|------|------------------------------|---------|
| `NotInitialized` | 2 | `config_guard`, `mint_authorization` | Contract admin not set; call `init` first. |
| `Unauthorized` | 3 | `transfer_guard`, `listing_validator`, `purchase_validator` | Caller lacks permission (generic). |
| `ContractPaused` | 4 | `pause_guard`, `royalty_pause_guard`, `listing_validator`, `purchase_validator` | Contract is paused; operation blocked. |
| `NotPaused` | 5 | `pause`/`unpause` (inline in lib.rs) | Operation requires current pause state. |
| `TokenNotFound` | 6 | `transfer_guard`, `royalty_validation_pipeline`, `purchase_validator` | Token or royalty config does not exist. |
| `UnauthorizedMinter` | 43 | `mint_authorization` | Caller is not an approved minter. |
| `UnauthorizedConfigurationUpdate` | 15 | `config_guard`, `royalty_validation_pipeline` | Caller is not the contract admin. |
| `RoyaltyFrozen` | 54 | `royalty_freeze`, `royalty_validation_pipeline` | Royalty configuration is permanently locked. |
| `RoyaltyPaymentsDisabled` | 61 | `royalty_emergency` | Emergency royalty payment toggle is active. |
| `InvalidAddress` | 12 | `transfer_guard` (blacklist), `token_owner_storage::validate_owner` | Address is blacklisted or invalid. |
| `InvalidRecipient` | 41 | `transfer_guard`, `royalty_validation_pipeline` | Address is the contract itself or invalid. |
| `SelfTransferNotAllowed` | 51 | `transfer_guard`, `owner_portfolio`, `purchase_validator` | Sender and recipient are the same. |
| `InvalidSalePrice` | 19 | `listing_validator`, `purchase_validator` | Price is zero, negative, or insufficient. |
| `PriceOverflow` | 60 | `listing_validator`, `purchase_validator` | Price exceeds maximum allowed value. |
| `UnsupportedAsset` | 50 | `royalty_asset_validator`, `listing_validator`, `purchase_validator` | Payment asset is not supported. |
| `InvalidBasisPoints` | 10 | `royalty_validator`, `royalty_validation_pipeline` | bps outside 0–10 000 range. |
| `InvalidConfig` | 18 | `listing_validator` | Listing expiration in the past. |

---

## Storage Backing

Guards read authorization and state data from two Soroban storage tiers:

| Storage Tier | Key(s) Read | Guard(s) That Read |
|--------------|-------------|---------------------|
| **Instance storage** | `DataKey::Admin` | `config_guard`, `storage_guard::guard_admin`, `mint_authorization`, `royalty_validation_pipeline::authorize_royalty_update` |
| **Instance storage** | `DataKey::Paused` | `pause_guard`, `royalty_pause_guard`, `storage_guard::guard_not_paused` |
| **Instance storage** | `DataKey::RoyaltyPaymentsDisabled` | `royalty_emergency` |
| **Persistent storage** | `DataKey::Administrator(Address)` | `royalty_admin_guard`, `administrator_storage` |
| **Persistent storage** | `DataKey::Blacklisted(Address)` | `transfer_guard` (blacklist sub-guard), mint path, purchase validator |
| **Persistent storage** | `DataKey::FrozenToken(TokenId)` | `transfer_guard` (frozen sub-guard) |
| **Persistent storage** | `DataKey::RoyaltyFrozen(TokenId)` | `royalty_freeze`, `royalty_validation_pipeline` |
| **Persistent storage** | `DataKey::TokenOwner(TokenId)` | `transfer_guard`, `token_owner_storage::verify_owner`, `listing_validator` |
| **Persistent storage** | `DataKey::Approval(TokenId)` | `transfer_guard::check_caller_authorized` (single-token approval) |
| **Persistent storage** | `DataKey::OperatorApproval(Owner, Operator)` | `transfer_guard::check_caller_authorized` (operator approval) |
| **Persistent storage** | `DataKey::ApprovedMinter(Address)` | `mint_authorization` |

---

## Examples for Contributors

### Example 1: Adding a new admin-gated function

When adding a new privileged operation, apply `config_guard::require_config_admin`
as the first check inside the entry point, before any storage read or write:

```rust
pub fn set_platform_fee(env: Env, admin: Address, fee_bps: u32) -> Result<(), Error> {
    // 1. Admin authorization — fail fast before any work.
    config_guard::require_config_admin(&env, &admin)?;

    // 2. Domain-specific validation.
    if fee_bps > MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidFee);
    }

    // 3. Persist the change.
    platform_fee::set_platform_fee(&env, fee_bps);

    // 4. Emit an audit event (optional but recommended).
    events::config::emit_platform_fee_updated(&env, &admin, fee_bps, env.ledger().timestamp());

    Ok(())
}
```

### Example 2: Adding a new transfer-like operation

Reuse `transfer_guard::check_transfer` at the top of any function that moves
token ownership. Do not inline authorization logic — call the composite guard
so all sub-checks (freeze, blacklist, approval, self-transfer, recipient)
are consistently applied:

```rust
pub fn safe_transfer(
    env: Env,
    caller: Address,
    from: Address,
    to: Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // 1. Soroban auth — the caller must sign the transaction.
    caller.require_auth();

    // 2. All transfer pre-conditions in one call.
    transfer_guard::check_transfer(&env, &caller, &from, &to, token_id)?;

    // 3. State mutation only after all guards pass.
    token_owner_storage::update_owner(&env, token_id, &to)?;

    Ok(())
}
```

### Example 3: Composing pause + admin for a new lifecycle function

For operations that must respect both the circuit-breaker and admin
authorization, chain the guards in order (pause first, then admin):

```rust
pub fn emergency_admin_operation(env: Env, admin: Address) -> Result<(), Error> {
    // Pause takes precedence — even admins cannot bypass a paused contract
    // unless the operation is specifically an admin toggle (pause/unpause).
    pause_guard::require_not_paused(&env)?;

    // Then verify admin identity.
    config_guard::require_config_admin(&env, &admin)?;

    // ... perform the operation ...
    Ok(())
}
```

> **Exception:** `pause` and `unpause` themselves must NOT call
> `require_not_paused` — they are the mechanisms that change the pause state.

### Example 4: Adding a sub-guard to the transfer pipeline

If a new transfer validation is needed (e.g. "reject transfers of tokens
flagged as premium"), add it as a new sub-guard in `transfer_guard.rs` and
insert it into the `check_transfer` chain at the appropriate position:

```rust
/// Example: reject transfers of premium (non-fungible) tokens.
pub fn check_not_premium(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if premium_token::is_premium(env, token_id) {
        return Err(Error::PremiumTransferRestricted);
    }
    Ok(())
}
```

Then in `check_transfer`, insert the call at the desired position — typically
after the ownership/frozen checks and before the caller-authorization check:

```rust
// Existing order preserved; new guard inserted at position 3.5:
token_owner_storage::get_owner(env, token_id)?;  // 1
check_not_self_transfer(from, to)?;               // 2
check_not_frozen(env, token_id)?;                 // 3 — frozen check
check_not_premium(env, token_id)?;                // NEW — premium restriction
check_not_blacklisted(env, from, to)?;            // 4 — blacklist
check_caller_authorized(env, caller, from, token_id)?; // 5 — authorization
check_valid_recipient(env, to)?;                   // 6 — recipient
```

### Example 5: Testing a new guard

Every guard module includes a `#[cfg(test)] mod tests` block. Follow the
existing pattern: set up the `Env`, manipulate storage directly via
`env.storage().instance().set(...)` or `env.storage().persistent().set(...)`,
then assert the guard's return value:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DataKey;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn admin_passes_guard() {
        let env = Env::default();
        let admin = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);

        assert!(require_config_admin(&env, &admin).is_ok());
    }

    #[test]
    fn unauthorized_caller_rejected() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let intruder = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);

        assert_eq!(
            require_config_admin(&env, &intruder),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    }
}
```

---

## Contributing Checklist

When adding or modifying a guard, verify:

- [ ] The guard is a standalone `pub fn` that returns `Result<(), Error>`.
- [ ] `require_auth()` is called on the relevant caller `Address` inside the guard, not in the entry point.
- [ ] The guard reads from the correct storage tier (instance vs. persistent).
- [ ] Unit tests cover: success case, the primary failure case, and edge cases (revoked access, missing storage).
- [ ] The entry point that uses the guard is listed in [Protected Contract Functions](#protected-contract-functions) above.
- [ ] This document is updated if a new guard or protected function is added.
