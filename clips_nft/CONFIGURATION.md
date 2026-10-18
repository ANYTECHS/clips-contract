# Configuration Module

Developer reference for the `Config` struct and its storage helpers in `clips_nft`.

---

## Overview

All global contract parameters are packed into a single `Config` struct stored once in **instance** storage. Reading or writing config costs one instance-storage operation regardless of how many fields are accessed.

---

## Storage Layout

| Storage type | Key | Type | Description |
|---|---|---|---|
| `instance` | `StorageKey::Config` | `Config` | All configurable parameters |

The key is a compact `contracttype` enum variant — no heap allocation on encode.

---

## Config Fields

```rust
pub struct Config {
    pub admin: Address,
    pub max_royalty_bps: u32,
    pub mint_cooldown_secs: u64,
    pub platform_fee_bps: u32,
}
```

| Field | Default | Description |
|---|---|---|
| `admin` | set on `init` | Address authorised to call admin functions |
| `max_royalty_bps` | `10_000` | Maximum royalty that can be set on any token (basis points) |
| `mint_cooldown_secs` | `0` | Minimum seconds between mints per address (`0` = no limit) |
| `platform_fee_bps` | `0` | Platform fee applied on secondary sales (basis points) |

---

## Update Flow

```
admin calls set_config(admin, new_config)
        │
        ├─ admin.require_auth()          ← Soroban auth check
        ├─ get_config()                  ← load current config
        ├─ assert cfg.admin == admin     ← ownership check
        ├─ validate_config(&new_config)  ← invariant check
        └─ set_config(&new_config)       ← write to instance storage
```

Only the stored `admin` address may update config. Passing a different address returns `Error::Unauthorized`.

---

## Validation Rules

| Rule | Error |
|---|---|
| `max_royalty_bps <= 10_000` | `Error::InvalidBasisPoints` |
| `platform_fee_bps <= 10_000` | `Error::InvalidBasisPoints` |

`mint_cooldown_secs` and `admin` have no additional validation constraints.

---

## Examples

### Read config

```rust
let cfg = client.get_config();
println!("max royalty: {}bps", cfg.max_royalty_bps);
```

### Update config (admin only)

```rust
let new_cfg = Config {
    admin: admin.clone(),
    max_royalty_bps: 1_000,   // cap royalties at 10%
    mint_cooldown_secs: 3_600, // 1 hour between mints
    platform_fee_bps: 250,     // 2.5% platform fee
};
client.set_config(&admin, &new_cfg);
```

### Rotate admin

```rust
let new_cfg = Config {
    admin: new_admin.clone(),
    ..client.get_config()
};
client.set_config(&current_admin, &new_cfg);
```

---

## Best Practices

- **Keep `platform_fee_bps` small.** High platform fees reduce creator earnings and may deter marketplace integrations.
- **Set `mint_cooldown_secs` > 0 in production.** A cooldown of 60–300 seconds limits spam minting.
- **Admin rotation should be done atomically.** Pass the new admin address inside the same `set_config` call; never store an admin you don't control.
- **Test config changes on testnet first.** Config is stored in instance storage with no on-chain history — changes are immediate and irreversible without another `set_config` call.
