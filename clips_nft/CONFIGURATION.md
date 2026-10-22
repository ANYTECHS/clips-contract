# Contract configuration

Developer reference for the global `Config` snapshot and related operator
settings in `clips_nft`. This page describes the current implementation; in
particular, not every field in `Config` is currently connected to the live
setting with a similar name.

## Overview

`Config` is a contract type stored as one value under `DataKey::Config` in
instance storage. `ClipsNftContract::get_config` returns `None` until a
configuration snapshot has been saved. Initializing the contract sets the
administrator and counters, but does not create a `Config` value.

The public snapshot update entry point is:

```text
set_config(updater, config) -> Result<(), Error>
```

The `updater` must be an authorized administrator and must authorize the
invocation. The stored administrator is the address at `DataKey::Admin`;
registered administrators are also accepted. The `owner` field inside
`Config` is data in the snapshot, not the access-control source. Changing it
does not change `DataKey::Admin` or grant the new address administrator
privileges.

### Config fields

| Field | Meaning | Accepted values |
|---|---|---|
| `owner: Address` | Address recorded in the configuration snapshot. It does not rotate the administrator. | Any Soroban `Address` value |
| `version: u32` | Version value recorded in the snapshot. The update path does not enforce monotonic increases or require a non-zero value. | Any `u32` |
| `platform_fee_bps: u32` | Platform-fee value in basis points. | `0..=1_000` |
| `default_royalty_bps: u32` | Default-royalty value in basis points. | `0..=10_000`, and combined with the platform fee must not exceed `10_000` |
| `paused: bool` | Pause value recorded in the snapshot. | `true` or `false` |
| `max_batch_mint_size: u32` | Maximum number of mints accepted in one batch. | `1..=100` |
| `max_batch_transfer_size: u32` | Maximum number of transfers accepted in one batch. | `1..=100` |
| `max_collection_size: u32` | Collection-size limit recorded in the snapshot. | `1..=100_000` |

Basis points use a 10,000-point scale: 100 bps is 1%, 500 bps is 5%, and
1,000 bps is 10%.

`Config` has no `Default` implementation, and initialization does not save
default field values. If no snapshot exists, batch mint and batch transfer
validation fall back to 50 items. The separate default royalty getter falls
back to 500 bps, the separate platform-fee getter falls back to 0 bps, and
the pause state falls back to `false`.

### Related operator settings

These settings are independently stored and must not be confused with
same-named fields in the `Config` snapshot:

| Setting | Public contract operations | Storage and behavior |
|---|---|---|
| Default royalty | `set_default_royalty_bps(admin, bps)`, `get_default_royalty_bps()` | `DataKey::DefaultRoyaltyBps` in instance storage; defaults to 500 bps; setter accepts `0..=10_000`. |
| Pause state | `pause(admin, reason)`, `unpause(admin)`, `is_paused()` | `DataKey::Paused` in instance storage; defaults to `false`; pause and unpause reject redundant state changes. |
| Supported payment currencies | `add_currency(admin, currency)`, `remove_currency(admin, currency)`, `get_currencies()` | `DataKey::SupportedCurrencies` in instance storage; defaults to an empty list; duplicates and removal of absent currencies are rejected. |
| Platform fee | `set_platform_fee` / `get_platform_fee` crate helpers | `DataKey::PlatformFee` in instance storage; defaults to 0 bps; setter accepts `0..=1_000`. These are not `ClipsNftContract` entry points, and the setter helper does not perform authorization. |
| Treasury wallet | `set_treasury_wallet` / `get_treasury_wallet` crate helpers | `DataKey::Treasury` in instance storage; unset until written. These helpers are not `ClipsNftContract` entry points, and the setter helper does not perform authorization. |

The standalone default royalty, pause, and currency operations use the
contract administrator guard. In contrast, `Config.platform_fee_bps`,
`Config.default_royalty_bps`, and `Config.paused` are only written to the
snapshot by `set_config`; that method does not update the separate fee,
royalty, or pause storage keys. The same is true of `owner`: it does not
update administrator storage. Only the configured batch mint and transfer
limits are read by their respective request validators. Treat the other
snapshot fields as recorded configuration data unless their runtime
integration is added.

## Storage layout

| Storage scope | Key | Value | Notes |
|---|---|---|---|
| Instance | `DataKey::Config` | `Config` | Entire global configuration snapshot. Missing until `set_config` is called. |
| Instance | `DataKey::Admin` | `Address` | Primary administrator used by config authorization; initialized by `init`. |
| Persistent | `DataKey::Administrator(address)` | `bool` | Additional administrator membership. |
| Instance | `DataKey::DefaultRoyaltyBps` | `u32` | Separate live default royalty; getter fallback is 500 bps. |
| Instance | `DataKey::PlatformFee` | `u32` | Separate live platform fee; getter fallback is 0 bps. |
| Instance | `DataKey::Treasury` | `Address` | Separate treasury-wallet value; no value is returned until written. |
| Instance | `DataKey::Paused` | `bool` | Separate live pause state; getter fallback is `false`. |
| Instance | `DataKey::SupportedCurrencies` | `Vec<Address>` | Separate supported-currency list; getter fallback is empty. |

Instance storage is appropriate for contract-wide values. Administrator
membership is keyed per address in persistent storage.

## Update flow

For `set_config`, the contract:

1. Reads the primary administrator from `DataKey::Admin` and checks whether
   `updater` is that administrator or a registered administrator.
2. Requires authorization from `updater`.
3. Validates the fee, royalty, combined deductions, batch sizes, and collection
   size.
4. If an old snapshot exists, emits a `cfg_updt` event for each changed field
   it tracks (fees, royalty, batch sizes, collection size, version, and owner).
   No field-change events are emitted when creating the first snapshot.
5. Writes the new `Config` to `DataKey::Config`.

As with other contract calls, a failed invocation does not persist a partial
update. A successful update replaces the entire snapshot; callers should read
the current value and preserve fields they do not intend to change.

The separate public operations for default royalty, pause state, and supported
currencies authorize the administrator and then update their own storage. The
currency operations also emit `cfg_updt` events on successful additions and
removals. The standalone royalty and pause operations do not use
`set_config`.

## Validation rules

| Validation | Failure |
|---|---|
| `platform_fee_bps <= 1_000` | `Error::InvalidBasisPoints` |
| `default_royalty_bps <= 10_000` | `Error::InvalidBasisPoints` |
| `default_royalty_bps + platform_fee_bps <= 10_000` | `Error::TotalDeductionsExceedSalePrice` |
| `max_batch_mint_size` is from 1 through 100 | `Error::InvalidConfig` |
| `max_batch_transfer_size` is from 1 through 100 | `Error::InvalidConfig` |
| `max_collection_size` is from 1 through 100,000 | `Error::InvalidConfig` |
| Caller is initialized administrator or registered administrator | `Error::NotInitialized` or `Error::UnauthorizedConfigurationUpdate`; caller authorization is also required |

There is no additional validation in `set_config` for `owner`, `version`, or
`paused`. The direct default-royalty setter independently accepts 0 through
10,000 bps and returns `Error::InvalidBasisPoints` above that range. Pause
operations return `Error::ContractPaused` if already paused and
`Error::NotPaused` if already unpaused.

## Examples

The following uses the generated Soroban client methods illustratively.

### Read the snapshot

```rust
let snapshot = client.get_config();
match snapshot {
    Some(config) => {
        // Use the saved configuration.
    }
    None => {
        // No Config snapshot has been written yet.
    }
}
```

### Save a valid snapshot

```rust
let config = Config {
    owner: admin.clone(),
    version: 1,
    platform_fee_bps: 250,       // 2.5%
    default_royalty_bps: 500,    // 5%; combined deductions are 7.5%
    paused: false,
    max_batch_mint_size: 50,
    max_batch_transfer_size: 50,
    max_collection_size: 10_000,
};

client.set_config(&admin, &config);
```

### Change one field and preserve the rest

```rust
if let Some(current) = client.get_config() {
    let updated = Config {
        max_batch_mint_size: 25,
        ..current
    };
    client.set_config(&admin, &updated);
}
```

Use `set_default_royalty_bps` or `pause`/`unpause` to change the corresponding
live settings; changing the similarly named field in `Config` alone does not
change those settings.

## Best practices

- Read the current snapshot before changing it and preserve fields not involved
  in the update.
- Keep the combined platform fee and default royalty at or below 10,000 bps.
- Use the dedicated royalty and pause entry points for their live values;
  do not rely on similarly named `Config` fields to update separate storage.
- Do not use `Config.owner` as an administrator rotation mechanism. Verify
  administrator access separately before relying on a new operator address.
- Treat `version` as informational: `set_config` does not enforce monotonic
  changes or tie it to the compile-time contract version.
- Test proposed settings and downstream behavior on testnet before production
  changes, and record successful updates from `cfg_updt` events where
  available.
