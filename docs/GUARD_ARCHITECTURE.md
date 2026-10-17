# ClipCash NFT Guard Architecture

Developer reference documenting the authorization and validation guard layer
in the `clips_nft` Soroban smart contract. This guide covers every guard
module, its responsibility, the execution order for each protected operation,
a mapping of which guards protect which contract functions, and contribution
examples for future maintainers.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Authorization Policy](#authorization-policy)
3. [Guard Categories](#guard-categories)
4. [Guard Reference](#guard-reference)
5. [Guard Execution Order](#guard-execution-order)
6. [Protected Contract Functions](#protected-contract-functions)
7. [Error Reference](#error-reference)
8. [Storage Backing](#storage-backing)
9. [Examples for Contributors](#examples-for-contributors)

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

## Authorization Policy

Authorization has two separate parts: **identity permission** (whether an
address has the required role) and **Soroban authentication** (`require_auth`,
which requires the address to authorize the invocation). A caller must satisfy
both where the policy requires a role. A caller-provided address is not proof
of permission by itself.

### Roles

| Role | How it is represented | Permissions and limits |
|------|------------------------|------------------------|
| **Contract admin** | Primary address in `DataKey::Admin`. | Controls contract configuration and privileged operations protected by `config_guard`; is the only role that can manage approved minters and blacklist entries in `AtomicMintContract`; may mint and has the transfer admin override. |
| **Registered administrator** | `DataKey::Administrator(address)` in persistent storage. | Accepted by `config_guard` and the royalty authorization guards. It does not grant the approved-minter management role. The current public contract API has no administrator grant/revoke method; these records are managed through internal storage/integration. |
| **Approved minter** | `DataKey::ApprovedMinter(address)` in persistent storage. | May call mint and batch-mint operations after authenticating. Cannot change configuration, manage roles, or transfer tokens by virtue of this role. Only the primary contract admin grants or revokes it. |
| **Token owner** | Current owner recorded for a token. | May transfer their token, grant/revoke token and operator approvals, create/update their listing, update metadata once, and update token royalty where the royalty update pipeline is used. Ownership is token-specific and does not grant contract-wide administration. |
| **Creator** | Creator address recorded for a token. | May update that token's metadata and use the royalty update pipeline. Creator status does not grant transfer, listing, or contract configuration rights. |
| **Approved operator** | Per-token approval or owner-wide operator approval. | May transfer only the approved token(s); may cancel a listing/offer only in the seller/buyer relationship checked by that operation. Approval does not convey ownership or general admin permissions. |
| **Buyer / seller / payer** | Address in the marketplace or payment request. | Must authenticate their own action. A buyer can place an offer or purchase; the current owner can accept offers; a payer can initiate royalty payment. These transaction roles do not grant configuration rights. |

The **primary admin** and **registered administrators** are not interchangeable
everywhere. For example, mint authorization checks the primary admin or the
approved-minter record, while metadata updates check the registered
administrator registry (as well as token owner and creator). Consult the
entrypoint map below before reusing a role check.

### Initialization trust boundary

All current contract `init` implementations accept an admin address and prevent
re-initialization, but do **not** call `require_auth` for that address.
Deployment must therefore initialize the contract atomically through a trusted
deployment flow and verify the resulting admin immediately. Do not treat the
`admin` argument as proof that the initializer is authorized. Add an explicit
initialization authorization check before relying on an externally callable
initializer.

---

## Guard Categories

| Category | Guards | Responsibility |
|----------|--------|----------------|
| **Admin authorization** | `config_guard::require_config_admin`, `royalty_admin_guard::require_royalty_admin`, `storage_guard::guard_admin` | Apply the operation's admin policy (primary admin, registered administrator, or both) before privileged mutations. |
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
privileged operations. Verifies the contract is initialized, the caller is
either the primary admin or a registered administrator, and the caller has
signed the invocation.

**Checks (in order):**
1. `DataKey::Admin` exists in instance storage → `Error::NotInitialized` if absent.
2. Caller matches `DataKey::Admin` or `administrator_storage::is_admin` →
    `Error::UnauthorizedConfigurationUpdate` if neither matches.
3. `caller.require_auth()` — Soroban authentication.

**Returns on success:** `Ok(())`

**Errors:**
| Error | When |
|-------|------|
| `NotInitialized` | `DataKey::Admin` is not set in instance storage. |
| `UnauthorizedConfigurationUpdate` | Caller is neither the primary admin nor a registered administrator. |

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

### 13. `ownership_guard` (issue #1089)

**File:** [`clips_nft/src/ownership_guard.rs`](../clips_nft/src/ownership_guard.rs)

**Responsibility:** Authoritative ownership check. Verifies the caller owns the
token and demands a signed authorization envelope.

| Function | Checks | Errors |
|----------|--------|--------|
| `require_owner(env, caller, token_id)` | `require_auth` + owner match | `TokenNotFound`, `Unauthorized` |
| `check_caller_is_owner(env, caller, token_id)` | read-only owner probe | none (`bool`) |
| `get_owner_for_token(env, token_id)` | owner lookup | `TokenNotFound` |

**Usage:**
```rust
ownership_guard::require_owner(&env, &caller, token_id)?;
```

---

### 14. `admin_access_control_guard` (issue #1090)

**File:** [`clips_nft/src/admin_access_control_guard.rs`](../clips_nft/src/admin_access_control_guard.rs)

**Responsibility:** Restricts administrative operations to authorized
administrators, checking the registered multi-admin list first and the
contract-level `DataKey::Admin` second.

| Function | Checks | Errors |
|----------|--------|--------|
| `require_admin(env, caller)` | initialized + `require_auth` + admin match | `NotInitialized`, `Unauthorized` |
| `check_caller_is_admin(env, caller)` | read-only admin probe | none (`bool`) |
| `get_configured_admin(env)` | primary admin lookup | `NotInitialized` |

**Usage:**
```rust
admin_access_control_guard::require_admin(&env, &caller)?;
```

---

### 15. `guard_composition` (issue #1091)

**File:** [`clips_nft/src/guard_composition.rs`](../clips_nft/src/guard_composition.rs)

**Responsibility:** Combines multiple guards so operations requiring several
independent checks execute them in a deterministic order and stop on the first
failure.

| Combinator | Behavior |
|------------|----------|
| `sequence(first, second)` | Run two guards in order; stop on first error. |
| `GuardBuilder::new().add(g).execute()` | Chain N guards in order; stop on first error. |

**Usage:**
```rust
guard_composition::sequence(
    pause_guard::require_not_paused(&env),
    config_guard::require_config_admin(&env, &admin),
)?;

GuardBuilder::new()
    .add(pause_guard::require_not_paused(&env))
    .add(transfer_guard::check_not_frozen(&env, token_id))
    .execute()?;
```

---

### 16. `purchase_state_guard` (issue #1027)

**File:** [`clips_nft/src/marketplace/purchase_state_guard.rs`](../clips_nft/src/marketplace/purchase_state_guard.rs)

**Responsibility:** Verifies an NFT listing is purchasable before settlement.
Re-exported at the crate root for convenience.

| Function | Checks |
|----------|--------|
| `check_listing_exists` / `get_purchasable_listing` | Listing exists for the token. |
| `check_listing_active` / `require_purchasable` | Listing is `Active`. |
| `check_listing_not_expired` | Expiration is in the future. |
| `check_listing_not_sold` / `require_purchasable_listing` | Listing is not already sold. |

**Usage:**
```rust
purchase_state_guard::require_purchasable_listing(&env, token_id)?;
```

---

### 17. `transfer_auth_guard` (issue #1024) and `transfer_recipient_guard` (issue #1025)

**Files:** [`clips_nft/src/transfer_auth_guard.rs`](../clips_nft/src/transfer_auth_guard.rs),
[`clips_nft/src/transfer_recipient_guard.rs`](../clips_nft/src/transfer_recipient_guard.rs)

**Responsibility:** Focused single-purpose transfer sub-guards reused by the
`transfer_guard` pipeline and batch transfers. `transfer_auth_guard` checks
owner / approval / operator / admin authorization; `transfer_recipient_guard`
rejects the contract's own address as a destination.

---

### 18. `royalty_auth_guard` (issue #1028)

**File:** [`clips_nft/src/royalty_auth_guard.rs`](../clips_nft/src/royalty_auth_guard.rs)

**Responsibility:** Unified pre-condition check for sensitive royalty
configuration changes: pause state, then lifecycle state (exists + not frozen),
then caller identity (registered admin, contract admin, creator, or owner).

| Function | Checks |
|----------|--------|
| `require_royalty_auth(env, caller, token_id)` | pause → state → any authorized identity |
| `require_royalty_admin_auth(env, caller, token_id)` | pause → state → admin identities only |
| `require_royalty_auth_no_token(env, caller)` | pause → caller is admin (no token context) |

**Usage:**
```rust
royalty_auth_guard::require_royalty_auth(env, &caller, token_id)?;
```

---

### 19. `init_guard` and `metadata_update_guard` (issues #1023)

**Files:** [`clips_nft/src/init_guard.rs`](../clips_nft/src/init_guard.rs),
[`clips_nft/src/metadata_update_guard.rs`](../clips_nft/src/metadata_update_guard.rs)

**Responsibility:** `init_guard::require_not_initialized` blocks double
initialization of the contract. `metadata_update_guard` gates metadata
mutation entry points so only authorized callers can rewrite token metadata.

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
   ↓                                     (+ frozen + blacklist guards)
list_nft        → listing_validator::validate_listing → [create + emit]
update_listing  → listing_storage::get_listing → owner check → [update + emit]
cancel_listing  → pause → frozen → blacklist → listing load →
                  owner/operator/admin check → [cancel/remove + emit]
                  (validator variant: listing_validator::cancel_listing
                   enforces pause + frozen + blacklist + Active status)
```

### Marketplace purchase / offer operations

```
Entry point ──→ caller.require_auth ──→ pause_guard::require_not_paused
   ↓                                     (+ frozen + blacklist guards)
buy_listing    → listing load → self-transfer / expiry / asset-match /
                 asset-supported / price-bounds / exact-amount /
                 seller-ownership → [royalty settlement + transfer + emit]
make_offer     → price + bounds + asset-supported + token-exists +
                 no-duplicate + expiry → [store + emit]
accept_offer   → offer load → blacklist + Active status + expiry +
                 owner verification + self-transfer + asset-supported →
                 [royalty settlement + transfer + emit]
cancel_offer   → caller.require_auth → (operator/buyer check) →
                 token-exists + not-frozen → [remove + emit]
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
  2. royalty_pause_guard::require_royalty_not_paused (royalty circuit-breaker)
  3. frozen_token check                        (NFT not soulbound)
  4. blacklist check                           (caller not blocked)
  5. validate_state_for_update                 (token exists + not frozen)
  6. royalty_freeze::require_not_frozen        (config not permanently locked)
  7. authorize_royalty_update                  (admin / creator / owner)
  8. validate_royalty                          (bps, recipients, struct)
  9. validate_royalty_asset                    (asset is supported)
 10. validate_royalty_recipient_struct         (per-recipient checks)
 11. Persistence: set_royalty + indexes
```

### Royalty payment operations

```
Entry point ──→ royalty_payment::pay_royalty
   ↓
  1. royalty_emergency::require_payments_enabled  (emergency toggle)
  2. sale_price > 0                                (inline)
  3. pause_guard::require_not_paused               (circuit-breaker)
  4. royalty_pause_guard::require_royalty_not_paused (royalty circuit-breaker)
  5. frozen_token check                            (NFT not soulbound)
  6. blacklist check                               (payer not blocked)
  7. token_storage::get_royalty                    (token exists)
  8. mark_replay                                   (replay protection)
  9. validate_royalty_recipients                   (recipient validity)
 10. validate_royalty_asset                        (asset support)
 11. transaction_deduction_validator               (total ≤ 100%)
 12. Distribution loop + event emission
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

The map lists public contract entry points and the rule they enforce. It omits
internal storage helpers: calling a helper directly in unit tests or internal
code does not create a new on-chain entry point. Keep authorization in the
public method and, for shared persistence helpers, enforce it at the write
boundary as well.

### Contract administration

| Entry point(s) | Required identity and authentication |
|----------------|--------------------------------------|
| `ClipsNftContract::set_config`, `ClipCashNFT::set_config` | Primary admin or registered administrator via `config_guard`; caller must authenticate. Validate all values before storage is changed. |
| `set_default_royalty_bps`, `pause`, `unpause`, `add_currency`, `remove_currency` | Primary admin or registered administrator via `config_guard`; caller must authenticate. |
| `set_royalty`, `freeze_token`, `unfreeze_token`, `reassign_creator`, `set_royalty_payments_disabled` | Primary admin or registered administrator via `config_guard`; caller must authenticate. Each also enforces its domain state/validation checks. Note: `set_royalty` is admin-only; use `update_royalty` for owner/creator authorization. |
| `AtomicMintContract::set_approved_minter`, `remove_approved_minter`, `add_to_blacklist`, `remove_from_blacklist` | Primary `DataKey::Admin` only, checked inline; caller must authenticate. A registered administrator is not sufficient for these methods. |

### Minting

| Entry point(s) | Required identity and authentication |
|----------------|--------------------------------------|
| `AtomicMintContract::mint`, `AtomicMintContract::batch_mint`, `ClipCashNFT::mint`, `ClipsNftContract::mint` | Contract admin or approved minter via `mint_authorization::require_mint_auth`; the authorized mint address must authenticate. Mint validation and blacklist checks still apply. |

### Token ownership and metadata

| Entry point(s) | Required identity and authentication |
|----------------|--------------------------------------|
| `approve`, `set_approval_for_all`, `revoke_approval`, `revoke_operator_approval` | The token owner authenticates. Approval records are limited to that owner's token(s). |
| `transfer`, `transfer_from`, `transfer_request`, `transfer_as_operator`, `batch_transfer`, `batch_transfer_request`, `transfer_with_request`, `transfer_with_result`, `transfer_from_request` | Caller authenticates and is the current owner, approved for the token, approved operator for the owner's tokens, or primary contract admin (transfer override). Transferability, freeze, blacklist, self-transfer, and recipient checks also apply. |
| `update_metadata` | Caller authenticates and is the token owner, recorded creator, or registered administrator. The token must not be frozen. Non-administrators have one metadata update per token; registered administrators bypass that limit. |
| `burn_token` | Caller authenticates and must satisfy the owner/operator conditions in `token_lifecycle::burn_token`. |

### Marketplace

| Entry point(s) | Required identity and authentication |
|----------------|--------------------------------------|
| `list_nft`, `create_listing` | Seller authenticates and must own the token. Pause, frozen-token, blacklist, listing, asset, and price rules apply. |
| `update_listing` | Listing seller authenticates; only that seller can update the listing. |
| `cancel_listing` | Authenticated caller must be the listing seller, an operator approved by that seller, or the primary contract admin. Pause, blacklist, and frozen-token rules apply. |
| `buy_listing` | Buyer authenticates. Listing, payment asset, amount, ownership, blacklist, and pause checks apply. |
| `make_offer` | Buyer authenticates. Offer, asset, token, amount, blacklist, and pause checks apply. |
| `accept_offer` | Current token owner authenticates and must be the offer's seller. Offer, asset, blacklist, and pause checks apply. |
| `cancel_offer` | Offer buyer or an operator approved by that buyer authenticates. |

### Royalties

| Entry point(s) | Required identity and authentication |
|----------------|--------------------------------------|
| `update_royalty`, `freeze_royalty` | Caller authenticates and is the primary admin, registered administrator, token creator, or token owner through the royalty authorization policy. The token/config must exist, be unfrozen, valid, and the contract must not be paused. |
| `set_royalty` | Admin-only as listed under contract administration; do not infer the broader `update_royalty` policy from the function name. |
| `set_royalty_payments_disabled` | Primary admin or registered administrator via `config_guard`; caller authenticates. |
| `pay_royalty` | Payer authenticates. This is not an admin operation; payment, token, pause, blacklist, asset, replay, and royalty validation apply. |

### Read-only operations

Getters such as `get_config`, `get_currencies`, `owner_of`, `get_listing`,
`get_royalty`, `royalty_info`, `is_approved_for_all`, and royalty/payment
history queries do not require a role or `require_auth`. Keep read-only methods
free of state mutation.

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
| `UnauthorizedConfigurationUpdate` | 15 | `config_guard`, `royalty_validation_pipeline` | Caller is neither the primary admin nor an accepted registered administrator for the operation. |
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

When adding a new privileged operation, apply
`config_guard::require_config_admin` before any state mutation. Validate the
proposed value before writing it, and use the caller address only after the
guard has authenticated and authorized it:

```rust
pub fn set_default_royalty_bps(
    env: Env,
    admin: Address,
    bps: u32,
) -> Result<(), Error> {
    config_guard::require_config_admin(&env, &admin)?;
    default_royalty::set_default_royalty_bps(&env, bps)
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
    // The composite guard includes caller authentication and transfer policy.
    transfer_guard::check_transfer(&env, &caller, &from, &to, token_id)?;

    // Mutate state only after every guard succeeds.
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
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(ClipsNftContract, ());
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            assert!(require_config_admin(&env, &admin).is_ok());
        });
    }

    #[test]
    fn unauthorized_caller_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let intruder = Address::generate(&env);
        let contract_id = env.register(ClipsNftContract, ());
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            assert_eq!(
                require_config_admin(&env, &intruder),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }
}
```

### Integration test: rejected update preserves state

Use a contract client for authorization tests so the test exercises the same
public entry point an external caller invokes. `mock_all_auths()` only supplies
the Soroban signature; it does not make an intruder an admin.

```rust
let before = client.get_config().unwrap();
let mut attempted = before.clone();
attempted.platform_fee_bps = 900;

assert_eq!(
    client.try_set_config(&intruder, &attempted),
    Err(Ok(Error::UnauthorizedConfigurationUpdate)),
);
assert_eq!(client.get_config().unwrap().platform_fee_bps, before.platform_fee_bps);
```

Run the focused config integration suite with:

```powershell
cargo test -p clips_nft --test test_config_guard
```

For a changed guard, also run its unit tests and the broader security tests:

```powershell
cargo test -p clips_nft config_guard
cargo test -p clips_nft guard_security_tests
```

---

## Contributing Checklist

When adding or modifying a guard, verify:

- [ ] The guard is a standalone `pub fn` that returns `Result<(), Error>`.
- [ ] The relevant caller authenticates via `require_auth()` in the guard or entry point, and the permission check cannot be bypassed by supplying a different address.
- [ ] The guard reads from the correct storage tier (instance vs. persistent).
- [ ] Unit tests cover: success case, the primary failure case, and edge cases (revoked access, missing storage).
- [ ] The entry point that uses the guard is listed in [Protected Contract Functions](#protected-contract-functions) above.
- [ ] This document is updated if a new guard or protected function is added.
