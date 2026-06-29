# Contract Initialization Flow

This document describes how the `ClipsNftContract` (Soroban / Stellar) is initialized and configured before use.

---

## Overview

The contract uses a one-time `init` entrypoint.  Calling any mutable function before `init` returns `Error::NotInitialized` (error code 28).  Calling `init` a second time panics with `"already initialized"`.

---

## Initialization Sequence

```
1. Deploy WASM to the Stellar network (soroban contract deploy)
2. Install the signer public key via set_signer() (admin only)
3. Call init(admin)
4. Optionally call set_global_config() to override defaults
```

---

## Required Parameters

| Parameter | Type      | Description                                |
|-----------|-----------|--------------------------------------------|
| `admin`   | `Address` | The account that will own the contract and authorise admin-only calls. Must sign the `init` transaction. |

---

## Default Values Set by `init`

| Storage key                   | Default value          |
|-------------------------------|------------------------|
| `Admin`                       | `admin` argument       |
| `NextTokenId`                 | `1`                    |
| `TotalSupply`                 | `0`                    |
| `Paused`                      | `false`                |
| `MintingPaused`               | `false`                |
| `PlatformRecipient`           | `admin`                |
| `PlatformFeeBps`              | `100` (1 %)            |
| `DefaultRoyaltyBps`           | `0`                    |
| `DefaultRoyaltyAsset`         | `None`                 |
| `Name`                        | `"ClipCash Clips"`     |
| `Symbol`                      | `"CLIP"`               |
| `MintCooldownSeconds`         | `0` (disabled)         |
| `CircuitBreakerEnabled`       | `false`                |
| `CircuitBreakerThreshold`     | `100`                  |
| `CircuitBreakerWindowSeconds` | `60`                   |
| `BackendAddress`              | `admin`                |

---

## Storage Layout

All values listed above live in **instance** storage and travel with the contract address through upgrades.  Token-level data (`TokenData`, approvals, balances) lives in **persistent** storage keyed by `DataKey::Token(token_id)`, etc.  Temporary counters (`CountMint`, `TotalGasMint`) live in **temporary** storage and may expire.

See `src/lib.rs` — the `DataKey` enum — for the full key list.

---

## Events Emitted at Init

`init` itself does not emit events.  The first observable on-chain signal is the ledger entry created for `DataKey::Admin`.

---

## Error Handling

| Situation                          | Error                    |
|------------------------------------|--------------------------|
| Any mutable call before `init`     | `NotInitialized` (28)    |
| Calling `init` a second time       | panic `"already initialized"` |
| Non-admin calling admin functions  | `Unauthorized` (1)       |

---

## Example Deployment (Soroban CLI)

```bash
# 1. Build
cargo build --target wasm32-unknown-unknown --release

# 2. Deploy
CONTRACT_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/clips_nft.wasm \
  --source $ADMIN_SECRET \
  --network testnet)

# 3. Initialize
soroban contract invoke \
  --id $CONTRACT_ID \
  --source $ADMIN_SECRET \
  --network testnet \
  -- init \
  --admin $ADMIN_ADDRESS

# 4. Set backend signer (32-byte Ed25519 public key)
soroban contract invoke \
  --id $CONTRACT_ID \
  --source $ADMIN_SECRET \
  --network testnet \
  -- set_signer \
  --admin $ADMIN_ADDRESS \
  --pubkey $SIGNER_PUBKEY_HEX
```

---

## Re-configuration After Init

Use `set_global_config` (issue #457) to atomically update all tuneable parameters, or call individual setters (`set_platform_fee`, `set_default_royalty`, `set_mint_cooldown`, etc.).  All setters require the `admin` address and emit a `ConfigUpdated` event.
