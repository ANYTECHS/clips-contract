# ClipCash — Soroban Smart Contracts

Stellar Soroban smart contracts for minting video clips as NFTs with royalty support.

## Overview

ClipCashNFT lets creators mint their best clips as NFTs on the Stellar blockchain.
Each token stores a metadata URI (IPFS / Arweave) on-chain and supports EIP-2981-style
royalties so creators earn on every secondary sale.

## Project Structure

```
clips-contract/
├── clips_nft/
│   ├── src/
│   │   └── lib.rs        # ClipCashNFT contract
│   └── Cargo.toml
├── Cargo.toml            # Workspace manifest
├── Makefile              # Build / test helpers
├── CONTRIBUTING.md
└── README.md
```

## Prerequisites

| Tool | Version |
|------|---------|
| Rust | 1.74+ |
| wasm32-unknown-unknown target | — |
| Stellar CLI (optional, for deployment) | 22+ |

```bash
# Install Rust wasm target
rustup target add wasm32-unknown-unknown

# Install Stellar CLI (optional)
cargo install --locked stellar-cli
```

## Quick Start

```bash
# Check
make check

# Run tests
make test

# Build release WASM
make build
```

## Contract: `clips_nft`

### Storage layout

| Key | Type | Description |
|-----|------|-------------|
| `Admin` | `Address` | Contract owner / admin |
| `TokenCount` | `u32` | Total minted supply |
| `NextTokenId` | `u32` | Auto-increment token ID counter |
| `Owner(token_id)` | `Address` | Token owner |
| `Metadata(token_id)` | `String` | Metadata URI (IPFS / Arweave) |
| `Royalty(token_id)` | `Royalty` | Royalty config for the token |
| `Balance(address)` | `u32` | Token balance per address |
| `ClipIdMinted(clip_id)` | `TokenId` | Prevents double-minting same clip |
| `TokenClipId(token_id)` | `u32` | Reverse map for burn cleanup |

### Public functions

| Function | Auth | Description |
|----------|------|-------------|
| `init(admin)` | — | Initialize contract, set admin |
| `mint(admin, to, clip_id, metadata_uri, royalty)` | admin | Mint NFT for a clip; emits `mint` event |
| `transfer(from, to, token_id)` | from | Transfer token ownership |
| `burn(owner, token_id)` | owner | Destroy token |
| `owner_of(token_id)` | view | Returns token owner |
| `balance_of(owner)` | view | Returns token count for address |
| `token_uri(token_id)` | view | Returns metadata URI |
| `clip_token_id(clip_id)` | view | Resolve clip ID → token ID |
| `get_metadata(token_id)` | view | Alias for `token_uri` |
| `get_royalty(token_id)` | view | Returns `Royalty` struct |
| `royalty_info(token_id, sale_price)` | view | Returns `RoyaltyInfo { receiver, royalty_amount }` |
| `set_royalty(admin, token_id, royalty)` | admin | Update royalty config post-mint |
| `total_supply()` | view | Returns total minted count |
| `exists(token_id)` | view | Returns true if token exists |

### Events

| Topic | Data type | Emitted by |
|-------|-----------|------------|
| `"mint"` | `MintEvent` | `mint()` |

`MintEvent` fields: `to`, `clip_id`, `token_id`, `metadata_uri`.

### Usage example

```rust
// Initialize
client.init(&admin);

// Mint
let token_id = client.mint(
    &admin,
    &creator,
    &42u32,                                          // clip_id
    &String::from_str(&env, "ipfs://QmXyz..."),      // metadata URI
    &Royalty { recipient: creator.clone(), basis_points: 500 }, // 5%
);

// Query
let owner   = client.owner_of(&token_id);
let balance = client.balance_of(&creator);
let uri     = client.token_uri(&token_id);

// Royalty for a 1 XLM sale (in stroops: 10_000_000)
let info = client.royalty_info(&token_id, &10_000_000i128);
// info.royalty_amount == 500_000 stroops (5%)
```

## Royalty model

Royalties follow the EIP-2981 pattern adapted for Soroban:

```
royalty_amount = sale_price × basis_points / 10_000
```

- `basis_points` range: `0` – `10_000` (0 % – 100 %)
- Marketplaces call `royalty_info(token_id, sale_price)` to get the exact
  amount to forward to `receiver` before crediting the seller.

## License

MIT
