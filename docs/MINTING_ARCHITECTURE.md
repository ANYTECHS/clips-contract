# ClipCash NFT Minting Architecture

Developer reference for contributors working on the `clips_nft` Soroban smart contract.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Sequence Diagram](#sequence-diagram)
3. [Request Validation](#request-validation)
4. [Token ID Generation](#token-id-generation)
5. [Storage Updates](#storage-updates)
6. [Metadata Association](#metadata-association)
7. [Creator Assignment](#creator-assignment)
8. [Royalty Initialization](#royalty-initialization)
9. [Ownership Assignment](#ownership-assignment)
10. [Event Lifecycle](#event-lifecycle)
11. [Batch Mint Workflow](#batch-mint-workflow)
12. [Error Handling](#error-handling)
13. [Security Considerations](#security-considerations)
14. [Testing Strategy](#testing-strategy)
15. [Example Request and Response](#example-request-and-response)

---

## Architecture Overview

The minting system is organized into three layers:

```
┌────────────────────────────────────────────────────────┐
│  Entry Point                                           │
│  AtomicMintContract::mint(params: MintParams)          │
│  mint_service::execute_batch_mint(batch)               │
└────────────────────┬───────────────────────────────────┘
                     │
┌────────────────────▼───────────────────────────────────┐
│  Orchestration Layer                                   │
│  atomic_mint::execute_atomic_mint()                    │
│  mint_service::execute_mint()                          │
└────────────────────┬───────────────────────────────────┘
                     │
┌────────────────────▼───────────────────────────────────┐
│  Storage / Domain Modules                              │
│  token_storage  creator_storage  clip_id_storage       │
│  wallet_token_index  signature_replay_storage          │
│  royalty_percentage  creator_portfolio  total_supply   │
└────────────────────────────────────────────────────────┘
```

**Key design principles:**

- **All-or-nothing** — every write step is tracked in a `MintRollback` struct;
  any failure triggers an explicit undo of all prior writes before returning.
- **Read-only validation first** — no storage is mutated until all checks pass.
- **Signature replay protection** — backend-issued signatures are SHA-256 hashed
  and persisted. A hash can never be reused.
- **Clip uniqueness enforced on-chain** — each `clip_id` maps to exactly one
  `token_id` for the lifetime of the contract.

---

## Sequence Diagram

```
Caller                 AtomicMintContract         execute_atomic_mint()
  │                           │                           │
  │── mint(MintParams) ──────►│                           │
  │                           │── execute_atomic_mint() ─►│
  │                           │                           │
  │                           │         ┌─ Phase 1: Validation (read-only) ──────┐
  │                           │         │ validate_owner()                       │
  │                           │         │ ensure_signature_unused()              │
  │                           │         │ validate_mint() [clip dedup, URI,      │
  │                           │         │   royalty bps, blacklist]              │
  │                           │         │ validate_metadata_uri()                │
  │                           │         │ validate_royalty()                     │
  │                           │         └────────────────────────────────────────┘
  │                           │                           │
  │                           │         ┌─ Phase 2: Writes (with rollback) ──────┐
  │                           │         │ 1. assign_owner()                      │
  │                           │         │ 2. set_metadata()                      │
  │                           │         │ 3. set_royalty()                       │
  │                           │         │ 4. set_creator_with_name()             │
  │                           │         │ 5. add_token_to_creator()              │
  │                           │         │ 6. save_clip_id()                      │
  │                           │         │ 7. add_token_to_wallet()               │
  │                           │         │ 8. mark_signature_used()               │
  │                           │         │ 9. increment_total_supply()            │
  │                           │         │ 10. commit_token_id()                  │
  │                           │         └────────────────────────────────────────┘
  │                           │                           │
  │                           │         ┌─ Phase 3: Events ───────────────────────┐
  │                           │         │ emit_mint()      (topic: "mint")        │
  │                           │         │ emit_nft_minted() (topic: "nft_mntd")  │
  │                           │         └────────────────────────────────────────┘
  │                           │                           │
  │◄── Ok(token_id) ──────────│◄── Ok(token_id) ──────────│
```

---

## Request Validation

Validation runs in two places: **pre-flight** (before writes) and **struct-level**
(on `MintRequest`/`BatchMintRequest`).

### Single mint — `mint_validator::validate_mint()`

Called inside `execute_atomic_mint` before any storage is touched.

| Check | Source | Error |
|---|---|---|
| `clip_id` not in `DataKey::ClipIdMinted` | `clip_id_storage` | `ClipAlreadyMinted` |
| `metadata_uri` is non-empty | inline | `InvalidURI` |
| `owner` not in `DataKey::Blacklisted` | persistent storage | `Unauthorized` |
| `royalty.basis_points ≤ MAX_ROYALTY_BPS` (10 000) | `storage_constants` | `InvalidBasisPoints` |

### Full request — `mint_validator::validate_mint_request()`

Used by the batch path; runs a stricter set of checks.

| Check | Module |
|---|---|
| Owner address structural validity | `token_owner_storage::validate_owner` |
| Owner not blacklisted | persistent `DataKey::Blacklisted` |
| Optional `creator_address` not blacklisted | persistent `DataKey::Blacklisted` |
| `clip_id` not in `clip_id_storage` or `ClipIdMinted` | `clip_id_storage` |
| `metadata_uri` non-empty and valid scheme | `metadata_uri_builder::validate_uri` |
| Optional `thumbnail_uri` valid scheme | `metadata_uri_builder::validate_uri` |
| Optional `preview_video_uri` valid scheme | `metadata_uri_builder::validate_uri` |
| `royalty_info.basis_points ≤ MAX_ROYALTY_BPS` | `storage_constants` |
| Royalty recipient address is valid | `royalty_recipient_validator` |

Accepted URI schemes: `ipfs://`, `https://`, `ar://`.

### Storage-level — `storage_validator`

Additional guards called inside `execute_atomic_mint`:

- `validate_metadata_uri` — scheme and structural check on the URI string.
- `validate_royalty` — enforces the contract-level royalty cap stored in config.

### Signature replay check — `signature_replay_storage::ensure_signature_unused()`

Reads `DataKey::UsedSignature(hash)` from persistent storage. Returns
`SignatureAlreadyUsed` if the hash is already present. This check runs in
Phase 1 so no writes happen before it passes.

---

## Token ID Generation

Token IDs are **monotonically incrementing `u32` values** starting at `0`.

**Storage key:** `DataKey::NextTokenId` in **instance** storage.

```
execute_atomic_mint:
  token_id = instance.get(NextTokenId).unwrap_or(0)   // read current
  ... all writes ...
  instance.set(NextTokenId, token_id + 1)             // commit after success
```

The counter is read **before** writes begin so the ID is stable throughout the
invocation, and it is committed **only after all writes succeed** (including the
signature mark and supply increment). If any write rolls back, `commit_token_id`
is never called, so the counter stays at its pre-mint value.

`mint_service::execute_mint` uses a slightly different pattern — it reads
`NextTokenId.saturating_add(1)` so the counter starts at 1 for the first token.
Both code paths guarantee there are no gaps unless a rollback occurs, and no
rollback leaves a "phantom" ID committed.

**Maximum supply:** `u32::MAX` (4 294 967 295). The contract does not enforce a
lower ceiling by default; add a `max_supply` config field if you need one.

---

## Storage Updates

Every successful single mint writes to the following persistent storage keys,
in order:

| Step | `DataKey` variant | Value type | Module |
|------|-------------------|------------|--------|
| 1 | `Token(token_id)` | `TokenData { owner, clip_id }` | `token_storage` |
| 2 | `Metadata(token_id)` | `String` (metadata URI) | `token_storage` |
| 2b | `MetadataIndex(uri)` | `TokenId` | `token_storage` |
| 3 | `Royalty(token_id)` | `Royalty` | `token_storage` |
| 4 | `Creator(token_id)` | `CreatorMetadata` | `creator_storage` |
| 5 | `CreatorTokens(creator)` | `Vec<TokenId>` | `creator_portfolio` |
| 6 | `TokenClipId(token_id)` | `u32` | `clip_id_storage` |
| 6b | `ClipIdMinted(clip_id)` | `bool` | `clip_id_storage` |
| 6c | `ClipMinted(clip_id)` | `bool` | `clip_id_storage` |
| 7 | `WalletTokens(owner)` | `Vec<TokenId>` | `wallet_token_index` |
| 8 | `UsedSignature(hash)` | `bool` | `signature_replay_storage` |

Instance storage (contract-global, not per-token):

| `DataKey` | Value type | Updated by |
|-----------|------------|------------|
| `NextTokenId` | `u32` | `commit_token_id()` |
| `TotalSupply` | `u32` | `total_supply::increment_total_supply()` |

The `mint_service` path also writes:
- `DataKey::ThumbnailUri(token_id)` — optional, via `thumbnail_uri` module
- `DataKey::PreviewVideoUri(token_id)` — optional, via `preview_video_uri` module
- `DataKey::RoyaltyPercentage(token_id)` — basis points as `u32`
- `DataKey::RoyaltyRecipient(token_id)` — recipient `Address`
- `DataKey::OwnerTokens(owner)` — owner portfolio index

---

## Metadata Association

Metadata is linked to a token via two storage entries:

```
DataKey::Metadata(token_id)       → String (the URI itself)
DataKey::MetadataIndex(uri)       → TokenId (reverse index, prevents duplicates)
```

The reverse `MetadataIndex` means if you attempt to mint a second token with
the same URI, the write will overwrite the existing index entry and can break
lookups. The `mint_validator` layer catches duplicates via the URI validator
before any write happens, and `DuplicateMetadata` is returned.

**URI schemes accepted by `validate_uri`:**

| Scheme | Use case |
|--------|----------|
| `ipfs://` | Decentralized, content-addressed storage (recommended) |
| `ar://` | Arweave permanent storage |
| `https://` | Centralized hosting (allowed but discouraged for permanence) |

**Optional media URIs** are stored separately from the canonical metadata URI
and do not affect the duplicate check:

- `DataKey::ThumbnailUri(token_id)` — marketplace thumbnail image
- `DataKey::PreviewVideoUri(token_id)` — video preview clip

These are only written when the corresponding `Option<String>` fields in
`MintRequest` are `Some`. They are removed during rollback.

---

## Creator Assignment

The creator defaults to the owner address when `MintParams.creator_address` is
`None`. When an explicit creator is provided it can be any valid address —
useful for platform-minted tokens on behalf of content creators.

**Write path in `atomic_mint`:**

```rust
let creator_addr = params.creator_address
    .clone()
    .unwrap_or_else(|| params.owner.clone());

creator_storage::set_creator_with_name(
    env, token_id, &creator_addr, params.creator_display_name.clone(),
);
creator_portfolio::add_token_to_creator(env, &creator_addr, token_id);
```

`set_creator_with_name` stores a `CreatorMetadata` struct:

```rust
pub struct CreatorMetadata {
    pub creator_address: Address,
    pub display_name:    Option<String>,
    pub verified:        bool,          // always false at mint time
}
```

`verified` is intentionally `false` at mint time. Only the platform admin can
call `set_creator_verified` to promote a creator, preventing self-verification.

**Creator portfolio** (`DataKey::CreatorTokens(creator)`) is a `Vec<TokenId>`
appended by `creator_portfolio::add_token_to_creator`. It is used by frontends
to enumerate all tokens a creator has made without scanning all token records.

The `creator_event::emit_creator_assigned` function is **not** called from
`execute_atomic_mint` directly; it is available for callers that go through
`creator_storage::assign_creator`. The `mint_service` path calls it on step 9.

---

## Royalty Initialization

Royalties follow a simplified EIP-2981 model: each token has a single recipient
and a basis-points rate.

**`atomic_mint` writes:**

```rust
token_storage::set_royalty(env, token_id, &params.royalty);
// DataKey::Royalty(token_id) → Royalty { recipient, basis_points, asset_address }
```

**`mint_service` writes additionally:**

```rust
royalty_percentage::set_royalty_percentage(env, token_id, basis_points)?;
// DataKey::RoyaltyPercentage(token_id) → u32

royalty_recipient::set_royalty_recipient(env, token_id, &recipient);
// DataKey::RoyaltyRecipient(token_id) → Address
```

The three keys serve different lookup patterns:

| Key | Purpose |
|-----|---------|
| `Royalty(token_id)` | Full struct — used by marketplace sale execution |
| `RoyaltyPercentage(token_id)` | Lightweight bps read for fee calculation |
| `RoyaltyRecipient(token_id)` | Lightweight address read for payout routing |

**Validation:** `royalty.basis_points` must be ≤ `MAX_ROYALTY_BPS` (10 000 = 100%).
The platform also enforces a contract-level cap via `storage_validator::validate_royalty`
which reads the configured maximum from instance storage.

---

## Ownership Assignment

Ownership is recorded in two places:

1. **Token record** — `DataKey::Token(token_id)` stores `TokenData { owner, clip_id }`.
   This is the canonical source of truth for `owner_of` queries.

2. **Wallet index** — `DataKey::WalletTokens(owner)` stores a `Vec<TokenId>`.
   This is an append-only list used by `tokens_of_owner` queries so wallets can
   enumerate their holdings without iterating all tokens.

`token_owner_storage::assign_owner` writes step 1. `wallet_token_index::add_token_to_wallet`
writes step 2. Both are called in Phase 2; the wallet write comes after the
clip-ID write so that a `ClipAlreadyMinted` error still rolls back ownership.

During rollback, both entries are explicitly removed:

```rust
token_owner_storage::remove_owner(env, token_id);
wallet_token_index::remove_token_from_wallet(env, &owner, token_id);
```

---

## Event Lifecycle

Two events are emitted per successful single mint, both in Phase 3 — **after
all state writes have committed**. No event is emitted on a failed or rolled-back
mint.

### `"mint"` — legacy event (topic symbol: `mint`)

```rust
pub struct MintEvent {
    pub to:           Address,   // owner
    pub clip_id:      u32,
    pub token_id:     TokenId,
    pub metadata_uri: String,
}
```

Retained for backward compatibility with indexers that subscribe to the short
`"mint"` topic. New integrations should prefer `"nft_mntd"`.

### `"nft_mntd"` — rich event (topic symbol: `nft_mntd`)

```rust
pub struct NFTMintedEvent {
    pub token_id:     TokenId,
    pub clip_id:      u32,
    pub creator:      Address,   // may differ from owner
    pub owner:        Address,
    pub metadata_uri: String,
    pub timestamp:    u64,       // env.ledger().timestamp()
}
```

This is the canonical event for indexers, wallets, and marketplaces. Receiving
it guarantees the token is fully persisted: all seven storage keys, the supply
counter, and the signature mark are committed before `emit_nft_minted` is called.

**The `creator_event::emit_creator_assigned` event** (topic: `"creator"`) is
emitted by `creator_storage::assign_creator`, which is called from `mint_service`
but **not** from `execute_atomic_mint`. If you need this event on the atomic
path, call `assign_creator` instead of `set_creator_with_name`.

### Event ordering per mint

```
1. emit_mint()        → topic "mint"      (MintEvent)
2. emit_nft_minted()  → topic "nft_mntd"  (NFTMintedEvent)
```

---

## Batch Mint Workflow

The batch path lives in `mint_service::execute_batch_mint`. It wraps multiple
`MintRequest` structs in a `BatchMintRequest` and processes them atomically.

### Pre-validation (all-or-nothing before any write)

```
validate_batch_mint(env, batch)
  ├── batch.validate_against_env()           // size: 1 ≤ len ≤ max_batch_mint_size
  ├── for each request:
  │     ├── check clip_id not in seen_clips  // intra-batch dedup
  │     └── validate_mint_request()          // full per-request checks
  └── Err(ClipAlreadyMinted) on first failure → abort, no writes
```

If `validate_batch_mint` returns `Err`, **zero storage writes have occurred**.

### Per-mint execution with rollback

```
for request in batch.requests:
  match execute_mint(env, request):
    Ok(result)  → push to results, continue
    Err(e)      → roll back ALL prior results in this batch
                  restore NextTokenId and TotalSupply to pre-batch values
                  return Err(e)
```

### Batch size limits

| Constant | Value | Location |
|----------|-------|----------|
| `MIN_BATCH_MINT_SIZE` | 1 | `storage_constants` |
| `MAX_BATCH_MINT_SIZE` | 50 | `storage_constants` |
| Runtime override | configurable up to 100 | `Config.max_batch_mint_size` |

### Return type

`execute_batch_mint` returns `Result<Vec<MintResult>, Error>` where each
`MintResult` contains `{ token_id, owner, clip_id, metadata_uri }`.

---

## Error Handling

### Error variants relevant to minting

| Variant | Code | Trigger |
|---------|------|---------|
| `NotInitialized` | 2 | `DataKey::Admin` absent in instance storage |
| `Unauthorized` | 3 | Owner or creator address is blacklisted |
| `ClipAlreadyMinted` | 7 | `clip_id` already in `ClipIdMinted` index |
| `InvalidBasisPoints` | 10 | `royalty.basis_points > MAX_ROYALTY_BPS` |
| `InvalidAddress` | 12 | `assign_owner` rejected the address |
| `InvalidURI` | 13 | `metadata_uri` is empty or invalid scheme |
| `InvalidConfig` | 18 | Batch size < 1 |
| `DuplicateWalletEntry` | 43 | Token ID already in owner's wallet index |
| `SignatureAlreadyUsed` | 44 | Replay attempt detected |
| `BatchLimitExceeded` | 45 | Batch size > configured max |

### Rollback guarantees

`MintRollback` tracks eight boolean flags, one per write step:

```
wrote_owner, wrote_metadata, wrote_royalty, wrote_creator_metadata,
wrote_creator_portfolio, wrote_clip_index, wrote_wallet_index, wrote_signature
```

`revert()` undoes writes in **reverse order** — signature first, owner last —
to match Soroban's own transaction rollback semantics. This means a partial
failure always leaves the contract in the exact same state as before the call.

`total_supply` and `commit_token_id` are called **after** `wrote_signature` is
set. If either fails, `revert()` unwinds all eight prior writes.

---

## Security Considerations

### Signature replay prevention

Every mint requires a unique `BytesN<32>` signature hash, derived from a
backend-issued `BytesN<64>` Ed25519 signature via SHA-256
(`signature_replay_storage::hash_signature`). The hash is checked in Phase 1
and marked used in Phase 2 step 8 — the last write before supply increment.
An attacker replaying a captured signature will receive `SignatureAlreadyUsed`
immediately with zero state change.

### Clip ID uniqueness

`clip_id` is a unique off-chain identifier. The contract enforces uniqueness via
three redundant keys (`ClipIdMinted`, `ClipMinted`, `TokenClipId`). An attempt
to mint the same `clip_id` twice returns `ClipAlreadyMinted` in Phase 1, before
any write.

### Blacklist

`DataKey::Blacklisted(address) → bool` is checked for both `owner` and
`creator_address`. A blacklisted address cannot mint or be set as a creator.

### Frozen tokens

`DataKey::FrozenToken(token_id)` marks a soulbound token. The transfer layer
checks this key before permitting a transfer. Frozen status cannot be set during
mint — it requires a separate admin call.

### Owner ≠ Creator separation

The contract deliberately allows `creator_address` to differ from `owner`.
This supports platforms that mint on behalf of creators. The separation is
critical for attribution: changing token ownership later does not change the
on-chain creator record.

### Admin initialization guard

`execute_atomic_mint` checks `DataKey::Admin` in instance storage before
reading `NextTokenId`. If `Admin` is absent the contract is considered
uninitialized and returns `NotInitialized` immediately.

---

## Testing Strategy

Tests are colocated with their modules (`#[cfg(test)] mod tests` at the bottom
of each `.rs` file). The testing stack uses the Soroban test utilities:
`Env::default()`, `env.mock_all_auths()`, and `env.register(Contract, ())`.

### Layers tested

| Layer | File | What is covered |
|-------|------|-----------------|
| Atomic mint integration | `atomic_mint.rs` | Happy path, replay, duplicate clip, rollback on wallet conflict, multiple mints, creator defaults/explicit, portfolio rollback |
| Event emission | `mint_event.rs` | Both events publish, field values round-trip through XDR, no spurious events on failed path |
| Validator | `mint_validator.rs` | Valid mint passes, duplicate clip, empty URI, blacklisted wallet, intra-batch duplicates |
| Signature replay | `signature_replay_storage.rs` | Store/detect, double-write rejection, deterministic hash |
| Creator storage | `creator_storage.rs` | CRUD for address, display name, verified flag; per-token isolation; rollback via `remove` |
| Service layer | `mint_service.rs` | Token ID sequencing, timestamp, supply increment, duplicate clip, media URIs, wallet index, creator portfolio |

### Testing rollback

`duplicate_clip_rolls_back_partial_writes` is the canonical rollback test:

```rust
// Mint once, then attempt duplicate clip_id with new sig and URI.
// The second call must fail and leave the first token untouched,
// with no new token_id committed and no leaked storage entries.
assert_eq!(client.next_token_id(), 1);   // still 1, not 2
assert!(client.token_exists(&0));         // original intact
assert!(!client.token_exists(&1));        // no phantom token
```

`wallet_index_conflict_rolls_back_prior_writes` pre-seeds a conflicting wallet
entry to force a failure at write step 7, verifying that writes 1–6 are all
undone:

```rust
assert!(!client.token_exists(&0));
assert!(!client.signature_used(&params.signature_hash));
assert_eq!(client.next_token_id(), 0);
```

### Adding a new mint write step

1. Add the write inside `execute_atomic_mint` Phase 2.
2. Add a corresponding `wrote_<step>: bool` field to `MintRollback`.
3. Add the undo logic to `MintRollback::revert()`, in reverse order.
4. Write a test that forces a failure *after* the new step and asserts the new
   key is absent after rollback.

---

## Example Request and Response

### Single mint via `AtomicMintContract::mint`

**Input — `MintParams`:**

```rust
MintParams {
    owner: Address::from_str("GBXG..."),          // token recipient
    clip_id: 12345,                               // unique off-chain clip ID
    metadata_uri: String::from("ipfs://QmXyz..."), // IPFS metadata JSON
    royalty: Royalty {
        recipient:    Address::from_str("GCRE..."), // royalty payout address
        basis_points: 500,                          // 5%
        asset_address: None,                        // native XLM
    },
    signature_hash: BytesN::from([...]),           // SHA-256 of backend sig
    creator_address:      Some(Address::from_str("GCRE...")), // optional
    creator_display_name: Some(String::from("ClipCreator")), // optional
}
```

**Output — `Ok(TokenId)`:**

```
0   // u32, first token ever minted on this contract
```

**Storage written (persistent):**

```
Token(0)                  → TokenData { owner: GBXG..., clip_id: 12345 }
Metadata(0)               → "ipfs://QmXyz..."
MetadataIndex("ipfs://…") → 0
Royalty(0)                → Royalty { recipient: GCRE..., bps: 500, asset: None }
Creator(0)                → CreatorMetadata { address: GCRE..., name: Some("ClipCreator"), verified: false }
CreatorTokens(GCRE...)    → [0]
TokenClipId(0)            → 12345
ClipIdMinted(12345)       → true
ClipMinted(12345)         → true
WalletTokens(GBXG...)     → [0]
UsedSignature(hash)       → true
```

**Storage written (instance):**

```
NextTokenId   → 1
TotalSupply   → 1
```

**Events emitted:**

```
topic: ("mint",)
data:  MintEvent { to: GBXG..., clip_id: 12345, token_id: 0, metadata_uri: "ipfs://QmXyz..." }

topic: ("nft_mntd",)
data:  NFTMintedEvent {
    token_id: 0, clip_id: 12345,
    creator: GCRE..., owner: GBXG...,
    metadata_uri: "ipfs://QmXyz...",
    timestamp: 1_700_000_000
}
```

### Batch mint via `mint_service::execute_batch_mint`

**Input — `BatchMintRequest`:**

```rust
BatchMintRequest {
    requests: vec![
        MintRequest { clip_id: 1, owner: ADDR_A, metadata_uri: "ipfs://Qm1", ... },
        MintRequest { clip_id: 2, owner: ADDR_B, metadata_uri: "ipfs://Qm2", ... },
        MintRequest { clip_id: 3, owner: ADDR_A, metadata_uri: "ipfs://Qm3", ... },
    ]
}
```

**Output — `Ok(Vec<MintResult>)`:**

```rust
[
    MintResult { token_id: 1, owner: ADDR_A, clip_id: 1, metadata_uri: "ipfs://Qm1", ... },
    MintResult { token_id: 2, owner: ADDR_B, clip_id: 2, metadata_uri: "ipfs://Qm2", ... },
    MintResult { token_id: 3, owner: ADDR_A, clip_id: 3, metadata_uri: "ipfs://Qm3", ... },
]
```

> `mint_service` starts token IDs at 1 (reads `NextTokenId` and adds 1 before
> writing), while `execute_atomic_mint` starts at 0 (reads then commits). The
> two code paths are separate entry points; choose the one that matches your
> deployment's initialization.

### Failed mint — duplicate clip

```rust
// Second call with clip_id: 12345 (already minted above)
client.try_mint(&params_with_duplicate_clip)
// Returns: Err(Error::ClipAlreadyMinted)
// Storage: unchanged from successful first mint
// NextTokenId: still 1
```

---

## Extension Points

| What to extend | Where to add code |
|---|---|
| New per-token storage field | Add `DataKey` variant → write in Phase 2 → add rollback flag |
| New validation rule | Add check in `mint_validator::validate_mint` or `validate_mint_request` |
| New event | Add struct to `types.rs`, emitter to `mint_event.rs`, call after all writes |
| Richer batch response | Extend `MintResult` / `MintSuccessResponse` structs |
| Supply cap | Read `max_supply` from `Config` in `execute_atomic_mint` Phase 1 |
| Cooldown enforcement | Read `mint_cooldown_secs` from `Config`; compare with `env.ledger().timestamp()` |
