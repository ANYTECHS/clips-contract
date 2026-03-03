<<<<<<< HEAD
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
=======
# clips-contract
# ClipCash

**Turn your long videos into short viral clips — automatically, with full control, and optional NFT ownership.**

ClipCash helps content creators (YouTubers, podcasters, gamers, coaches…) save many hours of work by turning one long video into dozens or hundreds of short clips ready for TikTok, Instagram Reels, YouTube Shorts, and more.

You always stay in control:  
→ Preview every clip  
→ Choose which ones you like  
→ Delete the bad ones  
→ Then post only the good ones automatically

Bonus: you can also turn your best clips into NFTs on the Stellar network (very cheap & fast) so you truly own them and can earn royalties forever.

## What makes ClipCash special?

- **Full preview & selection** — most tools post random clips. ClipCash lets you see and pick only the best ones.
- **Automatic posting** to 7+ platforms (TikTok, Instagram, YouTube Shorts, Facebook Reels, Snapchat Spotlight, Pinterest, LinkedIn)
- **Web2 + Web3 in one app** — normal accounts + optional Stellar NFTs with royalties
- **Simple & beautiful interface** — dark mode, clean design, easy to use

## Main Features (MVP – 2026)

- Upload long video or paste YouTube/TikTok link
- AI creates 50–200 short clips (15–60 seconds each)
- Preview screen: watch short previews, select / deselect / bulk delete
- One-click post selected clips to multiple platforms
- Earnings dashboard (shows money from all platforms)
- Optional: mint selected clips as NFTs on Stellar (Soroban smart contracts)
- Subscription plans + small revenue share (we take 5–10% only if you want)

## Tech Stack – Simple Overview

| Part          | Technology                  | Why we chose it                     |
|---------------|-----------------------------|--------------------------------------|
| Frontend      | Next.js 15 + React + Tailwind | Fast, beautiful, mobile-friendly    |
| Backend       | NestJS (TypeScript)         | Clean, organized, easy to grow      |
| Database      | PostgreSQL (via Supabase or Prisma) | Reliable & real-time updates     |
| Queue / Jobs  | BullMQ + Redis              | Handles long AI & posting tasks     |
| Social Posting| Ayrshare                    | One tool posts to all platforms     |
| Blockchain    | Stellar Soroban (Rust)      | Very cheap fees, built-in royalties |
| AI            | Runway Gen-3 + Claude       | Finds the most viral moments        |

## Project Folders (very simple view)
>>>>>>> 751fc7d2643a324601ef187a0a2e77cf8df897de
