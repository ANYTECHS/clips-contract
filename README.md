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
| `NextTokenId` | `u32` | Auto-increment token ID counter |
| `Paused` | `bool` | Pause flag |
| `Token(token_id)` | `TokenData` | Packed owner address and clip_id |
| `Metadata(token_id)` | `String` | Metadata URI (IPFS / Arweave) |
| `Royalty(token_id)` | `Royalty` | Royalty config for the token |
| `ClipIdMinted(clip_id)` | `TokenId` | Prevents double-minting same clip |
| `Signer` | `BytesN<32>` | Backend Ed25519 public key |

### Storage audit for mint cost

The contract uses compact enum keys and `u32` identifiers for token and clip indexes.
This avoids string-based storage keys in hot mint paths.

Estimated `mint` storage operations:
- `instance` reads: 4 (`Admin`, `NextTokenId`, `Paused`, `Signer`)
- `instance` writes: 1 (`NextTokenId`)
- `persistent` reads: 1 (`ClipIdMinted` dedup check)
- `persistent` writes: 2 (`TokenData`, `ClipIdMinted`)

Estimated persistent writes per mint: **2**.

### Contract ABI and Public Functions

#### `init`
Initialize the contract and set the admin.
- **Signature:** `init(env: Env, admin: Address)`
- **Auth:** —
- **Parameters:**
  - `admin`: The `Address` that will be the contract administrator.
- **Returns:** `()`

#### `set_signer`
Register (or rotate) the backend Ed25519 public key used to verify clip ownership before minting.
- **Signature:** `set_signer(env: Env, admin: Address, pubkey: BytesN<32>) -> Result<(), Error>`
- **Auth:** `admin`
- **Parameters:**
  - `admin`: The contract admin `Address`.
  - `pubkey`: 32-byte Ed25519 public key of the trusted backend signer.
- **Returns:** `Result<(), Error>`

#### `get_signer`
Return the currently registered backend signer public key, if any.
- **Signature:** `get_signer(env: Env) -> Option<BytesN<32>>`
- **Auth:** —
- **Returns:** `Option<BytesN<32>>`

#### `pause`
Pause the contract. Blocks `mint` and `transfer` until unpaused.
- **Signature:** `pause(env: Env, admin: Address) -> Result<(), Error>`
- **Auth:** `admin`
- **Parameters:**
  - `admin`: The contract admin `Address`.
- **Returns:** `Result<(), Error>`

#### `unpause`
Unpause the contract, re-enabling `mint` and `transfer`.
- **Signature:** `unpause(env: Env, admin: Address) -> Result<(), Error>`
- **Auth:** `admin`
- **Parameters:**
  - `admin`: The contract admin `Address`.
- **Returns:** `Result<(), Error>`

#### `is_paused`
Returns `true` if the contract is currently paused.
- **Signature:** `is_paused(env: Env) -> bool`
- **Auth:** —
- **Returns:** `bool`

#### `mint`
Mint a new NFT for a video clip. Requires a valid Ed25519 signature from the registered backend signer.
- **Signature:** `mint(env: Env, admin: Address, to: Address, clip_id: u32, metadata_uri: String, royalty: Royalty, signature: BytesN<64>) -> Result<TokenId, Error>`
- **Auth:** `admin`
- **Parameters:**
  - `admin`: The contract admin `Address`.
  - `to`: `Address` that will own the NFT.
  - `clip_id`: `u32` unique off-chain clip identifier.
  - `metadata_uri`: `String` (IPFS or Arweave URI).
  - `royalty`: `Royalty` configuration for secondary sales.
  - `signature`: `BytesN<64>` Ed25519 signature from the backend signer.
- **Returns:** `Result<TokenId, Error>`

#### `transfer`
Transfer NFT ownership from `from` to `to`.
- **Signature:** `transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error>`
- **Auth:** `from`
- **Parameters:**
  - `from`: Current owner `Address`.
  - `to`: New owner `Address`.
  - `token_id`: `TokenId` (`u32`) to transfer.
- **Returns:** `Result<(), Error>`

#### `burn`
Burn (destroy) an NFT. Only the current owner may burn.
- **Signature:** `burn(env: Env, owner: Address, token_id: TokenId) -> Result<(), Error>`
- **Auth:** `owner`
- **Parameters:**
  - `owner`: Current owner `Address`.
  - `token_id`: `TokenId` (`u32`) to destroy.
- **Returns:** `Result<(), Error>`

#### `owner_of`
Returns the owner of a given token ID.
- **Signature:** `owner_of(env: Env, token_id: TokenId) -> Result<Address, Error>`
- **Auth:** —
- **Parameters:**
  - `token_id`: `TokenId` (`u32`).
- **Returns:** `Result<Address, Error>`

#### `token_uri` (and `get_metadata`)
Returns the metadata URI for a given token ID.
- **Signature:** `token_uri(env: Env, token_id: TokenId) -> Result<String, Error>`
- **Auth:** —
- **Parameters:**
  - `token_id`: `TokenId` (`u32`).
- **Returns:** `Result<String, Error>`

#### `clip_token_id`
Look up the on-chain token ID for a given `clip_id`.
- **Signature:** `clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error>`
- **Auth:** —
- **Parameters:**
  - `clip_id`: The off-chain clip identifier (`u32`).
- **Returns:** `Result<TokenId, Error>`

#### `get_royalty`
Returns the stored `Royalty` struct for a token.
- **Signature:** `get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error>`
- **Auth:** —
- **Parameters:**
  - `token_id`: `TokenId` (`u32`).
- **Returns:** `Result<Royalty, Error>`

#### `royalty_info`
Returns the royalty receiver, amount, and payment asset for a given sale price.
- **Signature:** `royalty_info(env: Env, token_id: TokenId, sale_price: i128) -> Result<RoyaltyInfo, Error>`
- **Auth:** —
- **Parameters:**
  - `token_id`: `TokenId` (`u32`).
  - `sale_price`: The sale price in the asset's smallest unit (`i128`).
- **Returns:** `Result<RoyaltyInfo, Error>`

#### `pay_royalty`
Pay royalties for a token sale using the asset configured in the royalty (handles SEP-0041 assets).
- **Signature:** `pay_royalty(env: Env, payer: Address, token_id: TokenId, sale_price: i128) -> Result<(), Error>`
- **Auth:** `payer`
- **Parameters:**
  - `payer`: Address making the payment.
  - `token_id`: `TokenId` (`u32`).
  - `sale_price`: The sale price (`i128`).
- **Returns:** `Result<(), Error>`

#### `set_royalty`
Update the royalty configuration for a token. Admin only.
- **Signature:** `set_royalty(env: Env, admin: Address, token_id: TokenId, new_royalty: Royalty) -> Result<(), Error>`
- **Auth:** `admin`
- **Parameters:**
  - `admin`: The contract admin `Address`.
  - `token_id`: `TokenId` (`u32`).
  - `new_royalty`: The updated `Royalty` config.
- **Returns:** `Result<(), Error>`

#### `total_supply`
Returns the total number of minted tokens.
- **Signature:** `total_supply(env: Env) -> u32`
- **Auth:** —
- **Returns:** `u32`

#### `exists`
Returns true if the token exists.
- **Signature:** `exists(env: Env, token_id: TokenId) -> bool`
- **Auth:** —
- **Parameters:**
  - `token_id`: `TokenId` (`u32`).
- **Returns:** `bool`

### Events

| Topic | Data type | Emitted by | Description |
|-------|-----------|------------|-------------|
| `"mint"` | `MintEvent` | `mint()` | Emitted when a new NFT is minted. |
| `"paused"` | `()` | `pause()` | Emitted when the contract is paused. |
| `"unpaused"` | `()` | `unpause()` | Emitted when the contract is unpaused. |
| `"royalty"` | `(TokenId, Address, i128, Address)` | `pay_royalty()` | Emitted when a royalty is paid for a SEP-0041 asset. Data: `(token_id, receiver, amount, asset_address)`. |

`MintEvent` fields: 
- `to`: `Address`
- `clip_id`: `u32`
- `token_id`: `TokenId` (`u32`)
- `metadata_uri`: `String`

### Custom Types

**`TokenData`**
```rust
pub struct TokenData {
    pub owner: Address,
    pub clip_id: u32,
}
```

**`Royalty`**
```rust
pub struct Royalty {
    pub recipient: Address,
    pub basis_points: u32,
    pub asset_address: Option<Address>,
}
```

**`RoyaltyInfo`**
```rust
pub struct RoyaltyInfo {
    pub receiver: Address,
    pub royalty_amount: i128,
    pub asset_address: Option<Address>,
}
```

### Usage Examples

```rust
// 1. Initialize and Set Signer
client.init(&admin);
client.set_signer(&admin, &backend_pubkey);

// 2. Mint
let token_id = client.mint(
    &admin,
    &creator,
    &42u32,                                          // clip_id
    &String::from_str(&env, "ipfs://QmXyz..."),      // metadata URI
    &Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None }, // 5% XLM
    &signature,                                      // Ed25519 signature from backend
);

// 3. Query
let owner   = client.owner_of(&token_id);
let uri     = client.token_uri(&token_id);
let supply  = client.total_supply();

// 4. Royalty for a 1 XLM sale (in stroops: 10_000_000)
let info = client.royalty_info(&token_id, &10_000_000i128);
// info.royalty_amount == 500_000 stroops (5%)
// info.receiver == creator

// 5. Transfer
client.transfer(&creator, &buyer, &token_id);

// 6. Burn
client.burn(&buyer, &token_id);
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
