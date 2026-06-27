# ClipCash NFT Smart Contract

A Soroban smart contract for managing NFT minting with built-in royalty support and administrative controls.

## Implementation Overview

### Storage Architecture (#491)

All storage is managed through a centralized `StorageKey` enum:

```rust
pub enum StorageKey {
    Owner = 0,        // Contract owner address
    Admin = 1,        // Administrator set
    RoyaltyBps = 2,   // Default royalty in basis points
}
```

This design prevents duplicate keys and centralizes storage key management.

### Owner Management (#493)

The contract maintains a single owner address:

- **get_owner()** - Retrieve the current owner
- **set_owner(new_owner)** - Transfer ownership (owner auth required)

Only the current owner can transfer ownership.

### Administrator Management (#494)

The contract supports multiple administrators stored as a vector:

- **add_administrator(admin)** - Add an admin account (owner auth required)
- **remove_administrator(admin)** - Remove an admin account (owner auth required)  
- **is_administrator(admin)** - Check if an address is an admin

The implementation automatically prevents duplicate admins.

### Royalty Configuration (#469)

The contract stores a default royalty percentage in basis points (BPS):

- **get_royalty_bps()** - Get the current default royalty percentage
- **set_royalty_bps(bps)** - Set the royalty percentage (owner auth required)
- **Valid range**: 0 to 10,000 (representing 0% to 100%)

BPS validation is enforced on all writes. Setting invalid BPS will panic.

## Contract Methods

### Initialization

```rust
pub fn initialize(owner: Address, royalty_bps: u32)
```

Initialize the contract with:
- Owner address (will also be added as initial admin)
- Default royalty percentage in BPS (0-10000)

### Owner Operations

```rust
pub fn get_owner() -> Address
pub fn set_owner(new_owner: Address)
```

### Admin Operations

```rust
pub fn add_administrator(admin: Address)
pub fn remove_administrator(admin: Address)
pub fn is_administrator(admin: Address) -> bool
```

### Royalty Operations

```rust
pub fn get_royalty_bps() -> u32
pub fn set_royalty_bps(bps: u32)
```

## Testing

The contract includes unit tests for:
- Owner management (get/set owner)
- Admin management (add/remove/check admin)
- Royalty configuration (get/set royalty)
- BPS validation (invalid BPS panics)

Run tests with:
```bash
cargo test
```

## Authorization

- **Owner**: Can change owner, manage admins, set royalty percentage
- **Others**: Can read owner, check admin status, read royalty percentage

All write operations require owner authorization via `require_auth()`.

## Basis Points (BPS) Reference

- 1 BPS = 0.01%
- 100 BPS = 1%
- 1,000 BPS = 10%
- 10,000 BPS = 100%

## Storage Efficiency

All contract state uses persistent storage for long-term persistence across contract invocations. The storage structure is optimized for minimal on-chain footprint.
