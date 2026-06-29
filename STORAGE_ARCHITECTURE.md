# ClipCashNFT — Storage Architecture

## Overview

The contract uses three Soroban storage tiers:

| Tier | Lifetime | Used for |
|------|----------|----------|
| `instance` | Lives as long as the contract instance | Admin, counters, flags, signer |
| `persistent` | Survives ledger archival (with TTL) | Per-token data, dedup indexes |
| `temporary` | Expires after a short TTL | *(unused — reserved for future use)* |

---

## Storage Key Enum (`DataKey`)

All storage operations are keyed by `DataKey`, a `#[contracttype]` enum defined in `types.rs`.

```
DataKey
├── Instance storage
│   ├── Admin               → Address
│   ├── NextTokenId         → u32
│   ├── Paused              → bool
│   ├── Signer              → BytesN<32>
│   ├── Config              → Config
│   ├── PlatformFee         → u32
│   ├── DefaultRoyaltyBps   → u32
│   └── SupportedCurrencies → Vec<Address>
│
└── Persistent storage
    ├── Token(token_id: u32)        → TokenData
    ├── Metadata(token_id: u32)     → String  (IPFS / Arweave URI)
    ├── Royalty(token_id: u32)      → Royalty
    └── ClipIdMinted(clip_id: u32)  → TokenId (u32)
```

---

## Key-by-Key Reference

### Instance Keys

| Key | Type | Set by | Read by | Description |
|-----|------|--------|---------|-------------|
| `Admin` | `Address` | `init` | `require_admin`, `require_config_admin` | Contract owner; never changes after init |
| `NextTokenId` | `u32` | `init`, `mint` | `mint`, `total_supply` | Auto-increment counter; equals total supply |
| `Paused` | `bool` | `pause`, `unpause` | `require_not_paused`, `is_paused` | Global circuit-breaker |
| `Signer` | `BytesN<32>` | `set_signer` | `mint` | Ed25519 backend public key for clip ownership verification |
| `Config` | `Config` | `set_config` | `get_config`, various getters | Packed global settings snapshot |
| `PlatformFee` | `u32` (bps) | `set_platform_fee` | `get_platform_fee` | Platform fee in basis points (max 1 000) |
| `DefaultRoyaltyBps` | `u32` (bps) | `set_default_royalty_bps` | `get_default_royalty_bps` | Default per-token royalty (max 10 000) |
| `SupportedCurrencies` | `Vec<Address>` | `add_currency`, `remove_currency` | `get_currencies`, `is_currency_supported` | Allowlist of SEP-0041 payment assets |

### Persistent Keys

| Key | Type | Set by | Read by | Description |
|-----|------|--------|---------|-------------|
| `Token(id)` | `TokenData` | `mint`, `transfer` | `get_token`, `owner_of` | Owner + clip_id pair for each NFT |
| `Metadata(id)` | `String` | `mint` | `token_uri`, `get_metadata` | IPFS or Arweave URI for the clip |
| `Royalty(id)` | `Royalty` | `mint`, `set_royalty` | `get_royalty`, `royalty_info`, `pay_royalty` | Recipient, basis points, optional asset |
| `ClipIdMinted(clip_id)` | `u32` (token_id) | `mint` | `clip_token_id`, dedup check in `mint` | Reverse index from off-chain clip ID to on-chain token ID |

---

## Struct Definitions

```rust
pub struct TokenData {
    pub owner: Address,
    pub clip_id: u32,
}

pub struct Royalty {
    pub recipient: Address,
    pub basis_points: u32,        // 0–10 000
    pub asset_address: Option<Address>,
}

pub struct Config {
    pub owner: Address,
    pub version: u32,
    pub platform_fee_bps: u32,
    pub default_royalty_bps: u32,
    pub paused: bool,
    pub max_batch_mint_size: u32,
    pub max_collection_size: u32,
}
```

---

## Relationship Diagram

```
                      instance storage
┌────────────────────────────────────────────────────┐
│  Admin ──────────────────────────────────────────► │ authorization source
│  NextTokenId ─────────────────────────────────────► │ mint counter
│  Paused ──────────────────────────────────────────► │ circuit-breaker
│  Signer ──────────────────────────────────────────► │ ed25519 pubkey
│  Config ──────────────────────────────────────────► │ global settings
│  PlatformFee / DefaultRoyaltyBps / Currencies ────► │ fee parameters
└────────────────────────────────────────────────────┘
              │
              │ NextTokenId is used as token_id
              ▼
                      persistent storage (per token)
┌─────────────────────────────────────────────────────────┐
│  Token(id)      ──► TokenData { owner, clip_id }        │
│  Metadata(id)   ──► String (URI)                        │
│  Royalty(id)    ──► Royalty { recipient, bps, asset }   │
│  ClipIdMinted(clip_id) ──► token_id  (dedup / lookup)   │
└─────────────────────────────────────────────────────────┘
```

**Relationships:**
- `NextTokenId` → `Token(id)` / `Metadata(id)` / `Royalty(id)`: the counter value at mint time becomes the key for all three per-token entries.
- `ClipIdMinted(clip_id)` → `token_id`: a reverse index that also prevents double-minting the same clip.
- `Admin` is read by every mutating operation except `transfer` (which requires the `from` owner's auth instead).

---

## Storage Cost at Mint

Each `mint` call performs:

| Operation | Key | Tier |
|-----------|-----|------|
| Read | `Admin` | instance |
| Read | `NextTokenId` | instance |
| Read | `Paused` | instance |
| Read | `ClipIdMinted(clip_id)` | persistent (existence check) |
| Write | `Token(token_id)` | persistent |
| Write | `Metadata(token_id)` | persistent |
| Write | `Royalty(token_id)` | persistent |
| Write | `ClipIdMinted(clip_id)` | persistent |
| Write | `NextTokenId` | instance |

Persistent writes per mint: **4**  
Instance writes per mint: **1**

---

## Migration Guidelines

When upgrading the contract in the future:

1. **Bump `CONTRACT_VERSION`** in `config.rs` before any breaking change.
2. **Never rename existing `DataKey` variants** — doing so orphans all existing persistent entries. Add new variants instead.
3. **Adding fields to `Config`**: use `Option<T>` for new fields so older stored values deserialize gracefully, then migrate defaults via an admin call after upgrade.
4. **Adding fields to `TokenData` / `Royalty`**: same rule — wrap new fields in `Option<T>` until a full re-mint migration is performed.
5. **Removing a `DataKey` variant**: clean up the persistent entries first via an on-chain migration function before removing the variant from the enum.
6. **TTL management**: `persistent` entries require periodic TTL bumps (via `extend_ttl`) to survive long-running collections. Add a keeper/cron job or bump TTL inside `mint` and `transfer`.
7. **Test migrations** against a ledger snapshot (see `tests/upgrade_migration.rs`) before deploying to mainnet.
