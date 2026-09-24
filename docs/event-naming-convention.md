# Event Naming Convention

> Resolves issue #907: establishes a consistent naming convention for all
> contract events across the Clips NFT smart contract.

## Overview

All events emitted by the Clips NFT contract follow a unified naming convention
to ensure consistency, discoverability, and reliable indexing by off-chain
systems (wallets, indexers, analytics dashboards).

## Event Structure

Every Soroban event has two components:

| Component | Description |
|-----------|-------------|
| **Topics** | Short symbols (≤ 9 chars) used for filtering and classification |
| **Data** | Structured payload carrying event-specific information |

### Topic Hierarchy

Events use a two-level topic structure:

1. **Prefix topic** — groups related events (`nft_event`, `addr_event`)
2. **Specific topic** — identifies the exact event type (`nft_mint`, `nft_xfer`)

```
Topics: ["nft_event", "nft_mint"]
Data:   { token_id: 42, creator: "G...", owner: "G...", timestamp: 1720000000 }
```

## Naming Rules

### Topic Names

| Rule | Example | Bad Example |
|------|---------|-------------|
| Format: `<entity>_<action>` | `nft_mint` | `minted` |
| Use snake_case abbreviations | `nft_xfer` | `nft_transfer` |
| Max 9 characters (Soroban limit) | `nft_freeze` | `nft_frozen_event` |
| Use imperative/action verbs | `nft_burn` | `nft_burned` |
| Prefix with entity for grouping | `rlyt_paid` | `royalty_payment` |

### Topic Prefixes

| Prefix | Domain | Example Topics |
|--------|--------|----------------|
| `nft_` | NFT lifecycle | `nft_mint`, `nft_xfer`, `nft_burn`, `nft_freeze` |
| `rlyt_` | Royalty | `rlyt_paid`, `rlyt_asgn`, `rlyt_upd` |
| `appr_` | Approval | `appr_grnt`, `appr_rvkd` |
| `cfg_` | Configuration | `cfg_upd`, `cfg_pause` |
| `ofr_` | Offers | `ofr_crtd`, `ofr_acpt` |
| `bat_` | Batch ops | `bat_mint`, `bat_asgn` |

### Data Fields

| Field Type | Naming | Format | Example |
|------------|--------|--------|---------|
| Token identifiers | `token_id` | `u32` | `42` |
| Listing identifiers | `listing_id` | `u32` | `7` |
| Wallet addresses | `<role>` | `Address` | `"GABC..."` |
| Amounts | `amount` / `price` | `i128` (stroops) | `1_000_000` |
| Timestamps | `timestamp` | `u64` (epoch secs) | `1_720_000_000` |
| Asset addresses | `asset` | `Address` | `"GDEF..."` |

### Address Roles

When an event involves addresses, use descriptive role names:

| Role | Meaning |
|------|---------|
| `sender` / `from` | Source of the transfer/action |
| `recipient` / `to` / `new_owner` | Destination of the transfer/action |
| `creator` | Original minter/creator |
| `owner` | Current token holder |
| `seller` | Marketplace listing creator |
| `caller` | Address that initiated the call |
| `verifier` | Address that verified/approved |

## Constants

All topic constants are centralized in `event_topics.rs`:

```rust
pub const TOPIC_MINT: Symbol = soroban_sdk::symbol_short!("nft_mint");
pub const TOPIC_TRANSFER: Symbol = soroban_sdk::symbol_short!("nft_xfer");
pub const TOPIC_BURN: Symbol = soroban_sdk::symbol_short!("nft_burn");
```

**Never** use inline `symbol_short!()` calls in event emission code — always
import from `event_topics`.

## Helpers

Two helper modules reduce boilerplate:

- **`nft_event_helper`** — for NFT lifecycle events (mint, transfer, burn)
- **`address_event_helper`** — for events involving wallet addresses

```rust
// NFT event
use crate::nft_event_helper::emit_nft_event;
emit_nft_event(env, token_id, TOPIC_MINT, payload);

// Address event
use crate::address_event_helper::emit_sender_recipient_event;
emit_sender_recipient_event(env, "transfer", &sender, &recipient, amount, timestamp);
```

## Checklist for New Events

- [ ] Topic symbol is ≤ 9 characters
- [ ] Topic follows `<entity>_<action>` format
- [ ] Topic constant added to `event_topics.rs`
- [ ] Data struct uses consistent field naming (see table above)
- [ ] Timestamp is included (ledger timestamp in seconds)
- [ ] Event is emitted **after** all storage writes (see existing examples)
- [ ] Unit tests cover: emission, field values, multiple emissions
