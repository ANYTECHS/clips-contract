# Error Naming Convention

> Resolves issue #1375: establishes a consistent naming convention for all
> ClipCash contract errors.

This document is the contributor guide for adding or changing contract errors.
It covers error **names**, **codes**, **descriptions**, **categories** and
**documentation**. The companion [Error Catalog](error-catalog.md) lists the
errors that exist today; this document explains the rules they follow.

## Where errors live

| Location | Purpose |
|----------|---------|
| `clips_nft/src/types.rs` → `Error` | Top-level `#[contracterror]` enum returned by contract entry points (codes `1–99`). |
| `clips_nft/src/storage_errors.rs` → `StorageErrorKind` | Reusable storage-layer errors (codes `100–199`). |
| `clips_nft/src/error_infrastructure/` | Standardized, module-scoped error enums (codes `200+`). |
| `clips_nft/src/error_infrastructure/registry.rs` → `ERROR_CODES` | Single source of truth mapping code → module, name and description. |
| `clips_nft/src/error_catalog/categories.rs` → `ErrorCategory` | Classification of codes for indexers and clients. |
| `clips_nft/src/errors.rs` | Re-exports and semantic type aliases (`StateError`, `TransferError`, …). |

Before adding an error, search these locations. **Reuse an existing error
whenever it describes the same failed invariant** — do not create a second
name for the same condition.

## 1. Error names

### Rules

| Rule | Good | Bad |
|------|------|-----|
| Use `PascalCase` (Rust enum variant style). | `ListingNotFound` | `listing_not_found`, `LISTING_NOT_FOUND` |
| Describe the failed invariant, not the code path that failed. | `InsufficientBalance` | `TransferCallFailed` |
| Put the subject (noun) first, then the condition. | `OfferExpired`, `RoyaltyTooHigh` | `ExpiredOffer`, `TooHighRoyalty` |
| Use the full word; avoid abbreviations. | `InvalidAddress` | `InvAddr`, `BadAddr` |
| Do not repeat the enum/module name in the variant. | `ConfigurationError::InvalidFee` | `ConfigurationError::ConfigInvalidFee` |
| Do not prefix with `Error` or suffix with `Err`. | `TokenNotFound` | `ErrorTokenNotFound`, `TokenNotFoundErr` |
| Keep names unique across the whole contract. | — | Two enums both defining `InvalidFee` with different codes |
| Spell acronyms as words (`Uri`, `Url`, `Id`). | `InvalidUri`, `InvalidTokenId` | `InvalidURI`, `InvalidTokenID` |

> Note: some older variants (e.g. `InvalidURI`) predate this rule. They are kept
> for ABI compatibility — do not rename existing variants; apply the rule to new
> ones.

### Standard name patterns

Pick the pattern that matches the failed invariant:

| Pattern | Meaning | Examples |
|---------|---------|----------|
| `<Subject>NotFound` | A referenced record does not exist. | `TokenNotFound`, `ListingNotFound`, `ApprovalNotFound` |
| `<Subject>AlreadyExists` / `Already<State>` | Uniqueness or one-shot state violated. | `OfferAlreadyExists`, `AlreadyInitialized`, `AlreadyBurned` |
| `Duplicate<Subject>` | The same item appears twice in one request or index. | `DuplicateToken`, `DuplicateListing` |
| `Invalid<Subject>` | Input is structurally or semantically wrong. | `InvalidAddress`, `InvalidFee`, `InvalidBatchRequest` |
| `Empty<Subject>` | A required value or collection is empty. | `EmptyTitle`, `EmptyBatch` |
| `<Subject>TooHigh` / `<Subject>TooLarge` | Value exceeds an upper bound. | `RoyaltyTooHigh`, `BatchTooLarge` |
| `<Subject>Overflow` | Arithmetic overflow. | `RoyaltyOverflow`, `PriceOverflow` |
| `<Subject>LimitExceeded` | A configured count/limit is exceeded. | `BatchLimitExceeded` |
| `<Subject>Expired` | A time-bounded record is past its deadline. | `OfferExpired`, `ListingExpired` |
| `Unauthorized` / `Unauthorized<Action>` | Caller lacks permission for an action. | `Unauthorized`, `UnauthorizedTransfer` |
| `<Subject>NotApproved` | Missing approval/allow-list entry. | `OperatorNotApproved` |
| `Unsupported<Subject>` | Input is valid but not accepted by this contract. | `UnsupportedAsset`, `UnsupportedProtocol` |
| `<Subject>Mismatch` | Two values that must match do not. | `PaymentAssetMismatch` |
| `Not<State>` / `<Subject>NotActive` | Required state is not active. | `NotInitialized`, `NotPaused`, `ListingNotActive` |
| `Contract<State>` | Global contract-level guard. | `ContractPaused` |

## 2. Error codes

Error codes are the on-chain identifier returned by Soroban (the `#[repr(u32)]`
discriminant). They are part of the public ABI.

### Rules

1. **Every code is unique** across every `#[contracterror]` enum in the crate.
2. **Codes are explicit.** Always write `Variant = <code>`; never rely on
   implicit discriminants.
3. **Codes are permanent.** Never change or reuse the code of a released
   error. Retired errors keep their code reserved; mark them deprecated in
   their doc comment instead of deleting the number.
4. **Codes are allocated in module blocks.** Each module owns a contiguous
   range; new errors take the next free code in their module's block.
5. **New modules get a new block of 10** (e.g. `300–309`), aligned to a
   multiple of ten, after the highest allocated block.
6. **Aliases are constants, not new codes.** If a second name is needed for
   compatibility, add a `pub const Alias: Self = Self::Canonical;` (see
   `impl Error` in `types.rs`) instead of a new variant.
7. **Register every code** in `error_infrastructure/registry.rs` in the same
   pull request that introduces it.

### Code ranges

| Range | Owner | Notes |
|-------|-------|-------|
| `1–99` | `types::Error` | Top-level contract errors (legacy flat space). Append new top-level errors after the current highest code. |
| `100–199` | `storage_errors::StorageErrorKind` | Storage-layer errors. |
| `200–209` | `initialization` | |
| `210–219` | `configuration` | |
| `220–229` | `validation` | |
| `230–239` | `core` (token / ownership) | |
| `240–249` | `transfer` | |
| `250–259` | `minting` | |
| `260–269` | `royalty_validation`, `marketplace`, `payment_replay` | Currently shared; see the [Error Catalog](error-catalog.md). New modules must not share a block. |
| `270–279` | approval and transfer authorization | |
| `280–289` | transfer state | |
| `290–299` | batch transfers | |
| `300+` | unallocated | Next new module starts here. |

## 3. Error descriptions

Every error variant has a `///` doc comment, and every registry entry has a
non-empty `description` string. Descriptions are read by developers,
indexers and front-end clients, so they must be precise.

### Rules

| Rule | Good | Bad |
|------|------|-----|
| One sentence, ending with a period. | `/// The token has already been burned.` | `/// burned` |
| State the condition that caused the failure. | `/// Listing has expired and is no longer purchasable.` | `/// Error in buy().` |
| Present tense, neutral wording, no blame. | `/// Payment amount does not match the listing price.` | `/// You sent the wrong amount!` |
| Mention the limit or rule when one applies. | `/// Royalty basis points exceed the allowed maximum (10 000 = 100 %).` | `/// Royalty is bad.` |
| No sensitive data, no addresses, no internals. | `/// Caller is not an approved minter.` | `/// Caller G... missing from MINTERS map at key 0x…` |
| Optional: reference the originating issue. | `/// Offer has expired and is no longer valid (#886).` | — |

The `///` doc comment on the variant and the registry `description` should say
the same thing; the registry string may be the variant comment verbatim.

## 4. Error categories

Every code maps to exactly one **primary** category from
`error_catalog::ErrorCategory`. Categories let indexers and clients bucket
failures without knowing every individual code.

| Category | Use when | Typical name patterns |
|----------|----------|-----------------------|
| `Validation` | Input fails structural or semantic checks. | `Invalid*`, `Empty*`, `*TooHigh`, `*TooLarge`, `Duplicate*` |
| `Authorization` | Caller lacks permission or approval. | `Unauthorized*`, `*NotApproved` |
| `Ownership` | Caller does not own the referenced asset, or ownership data is inconsistent. | `MissingOwner`, `InvalidOwnershipState` |
| `Payment` | Payment amount, asset, balance or replay checks fail. | `InsufficientBalance`, `PaymentAssetMismatch`, `PaymentAlreadyProcessed` |
| `Marketplace` | Listing or offer lifecycle failure. | `Listing*`, `Offer*` |
| `Royalty` | Royalty configuration or payout failure. | `Royalty*` |
| `Storage` | Storage read/write/integrity failure. | `Storage*`, `CorruptedStorage`, `MalformedData` |
| `Security` | Security guard triggered (replay, reentrancy, frozen token, signatures). | `ReentrantCall`, `InvalidSignature`, `SignatureAlreadyUsed` |
| `Configuration` | Contract or configuration state is invalid (initialization, pause, config values). | `AlreadyInitialized`, `NotInitialized`, `ContractPaused`, `InvalidConfig` |

### Rules

1. Choose the category that describes **why** the call failed, not which
   module raised it (e.g. a marketplace purchase that fails because the buyer
   lacks funds is `Payment`).
2. When adding a code, update `categorize_by_code` in
   `error_catalog/categories.rs` so the new code maps to its category.
3. Add a new `ErrorCategory` variant only when no existing category fits, and
   append it to `ALL_CATEGORIES` with a lowercase `as_str` name.

## 5. Error documentation

An error is not complete until it is documented in all of these places:

1. **Variant doc comment** — `///` description on the enum variant (section 3).
2. **Enum doc comment** — module-level enums state their code block, e.g.
   `/// Maps to the 210–214 block in the centralized error registry.`
3. **Registry entry** — an `ErrorCode { module, name, code, description }`
   entry in `error_infrastructure/registry.rs`; `name` must match the Rust
   variant name exactly.
4. **Category mapping** — the code is handled by `categorize_by_code`.
5. **Error Catalog** — if a new code block is created, add it to the
   allocation table in [`docs/error-catalog.md`](error-catalog.md).

## Checklist for contributors

When adding a new error, confirm that:

- [ ] No existing error already describes the same failed invariant.
- [ ] The name is `PascalCase`, follows a standard pattern, and is unique.
- [ ] The code is explicit, unique, and inside the owning module's block.
- [ ] No existing code was changed or reused.
- [ ] The variant has a one-sentence `///` description.
- [ ] The error is registered in `error_infrastructure/registry.rs`.
- [ ] The code is mapped to a primary category in `categorize_by_code`.
- [ ] A test triggers the error (e.g. `assert_eq!(result, Err(Ok(Error::ListingExpired)))`).
- [ ] `docs/error-catalog.md` is updated if a new code block was added.

## Example

```rust
/// Errors for marketplace listing operations.
///
/// Maps to the `300–309` block in the centralized error registry.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ListingError {
    /// The listing price must be greater than zero.
    InvalidListingPrice = 300,

    /// The listing duration exceeds the configured maximum.
    ListingDurationTooLong = 301,
}

impl ListingError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "listing";
}
```

Registry entry:

```rust
ErrorCode {
    module: "listing",
    name: "InvalidListingPrice",
    code: 300,
    description: "The listing price must be greater than zero.",
},
```
