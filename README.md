# ClipCash NFT Smart Contract

Stellar Soroban smart contract for minting video clips as NFTs with royalty support on the Stellar network.

## Overview

This smart contract enables ClipCash users to mint their video clips as NFTs on the Stellar blockchain. It includes built-in royalty support, allowing creators to earn royalties on secondary sales.

## Features

- **NFT Minting**: Create unique NFTs for video clips
- **Royalty System**: Configure royalties up to 100% (in basis points)
- **Metadata Storage**: Store clip title, description, media URL, and thumbnail
- **Ownership Transfer**: Transfer NFTs between addresses
- **Burn Capability**: Owners can burn (destroy) their NFTs
- **Total Supply Tracking**: Query total number of minted NFTs

## Quick Start

### Prerequisites

- Rust 1.70+
- wasm32-unknown-unknown target

### Install Dependencies

```bash
make install-deps
```

### Build

```bash
make build
```

### Test

```bash
make test
```

## Contract Functions

| Function | Description |
|----------|-------------|
| `init` | Initialize contract with admin |
| `mint` | Mint a new NFT |
| `transfer` | Transfer NFT ownership |
| `get_metadata` | Get NFT metadata |
| `get_royalty` | Get royalty info |
| `get_owner` | Get token owner |
| `total_supply` | Get total supply |
| `exists` | Check if token exists |
| `burn` | Burn/destroy NFT |

## Usage Example

```rust
// Initialize
let admin = Address::generate(&env);
client.init(&admin);

// Mint NFT
let metadata = TokenMetadata {
    title: "My Viral Clip".to_string(),
    description: "An amazing moment".to_string(),
    media_url: "ipfs://Qm...".to_string(),
    thumbnail_url: "ipfs://Qm...".to_string(),
    creator: user.clone(),
    created_at: env.ledger().timestamp(),
};

let royalty = Royalty {
    recipient: user.clone(),
    basis_points: 500, // 5%
};

let token_id = client.mint(&admin, &user, &metadata, &royalty);
```

## Project Structure

```
contracts/
├── clips_nft/           # Smart contract
│   ├── src/lib.rs      # Contract implementation
│   └── Cargo.toml      # Dependencies
├── Makefile            # Build commands
├── CONTRIBUTING.md     # Contribution guide
└── README.md           # This file
```

## License

MIT License
