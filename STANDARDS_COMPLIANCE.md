# Standards Compliance — ClipCash NFT

This document outlines the compliance of the ClipCash NFT smart contracts with existing and emerging blockchain standards.

## ERC-721 Style Compliance (Soroban NFT Standard)

While Soroban does not have a finalized ERC-721 equivalent yet, ClipCash implements the core interface patterns expected by modern marketplaces and wallets.

### Core Interface
- **`balance_of(owner)`**: Returns the number of tokens owned by a specific address.
- **`owner_of(token_id)`**: Returns the current owner of a specific token.
- **`transfer(from, to, token_id)`**: Standard transfer function with authorization checks.
- **`approve(caller, operator, token_id)`**: Allows an operator to manage a specific token.
- **`set_approval_for_all(owner, operator, approved)`**: Allows an operator to manage all tokens of an owner.
- **`exists(token_id)`**: Check if a token has been minted and not burned.

### Enumerable Extension
- **`total_supply()`**: Returns the total number of tokens minted.
- **`token_by_index(index)`**: Allows iterating through all tokens on-chain.
- **`tokens_of_owner(owner)`**: Returns a list of tokens owned by a specific address (bounded by gas limits).

### Metadata Extension
- **`token_uri(token_id)`**: Returns the metadata URI (IPFS/Arweave).
- **`name()`**: Returns "ClipCash Clips".
- **`symbol()`**: Returns "CLIP".

### Event Emission
ClipCash emits `TransferEvent` for all ownership changes, including:
- **Minting**: `from` is the contract address (representing the "zero" address in Soroban).
- **Burning**: `to` is the contract address.
- **Transfers**: Standard `from` -> `to` emission.

## Royalty Standard (EIP-2981 Adaptation)

ClipCash implements a robust royalty system inspired by EIP-2981, adapted for the Soroban environment.

### Basis Points (BPS)
- Precision: 0.01% (1 BPS = 0.01%).
- 10,000 BPS = 100%.

### View Functions
- **`get_royalty(token_id)`**: Returns the raw royalty configuration (recipient and BPS).
- **`royalty_info(token_id, sale_price)`**: Returns the calculated royalty amount and recipient for a given sale price.

### Features
- **Rounding**: Implements "nearest" rounding to handle fractional units.
- **Overflow Protection**: Safe math checks for large sale prices.
- **Asset Agnostic**: Supports any SEP-0041 compliant asset (including XLM/stroops).

## Implementation Details

- **Language**: Rust
- **Framework**: Soroban SDK
- **Storage Strategy**: Uses compact storage keys to minimize gas costs during hot paths (minting/transferring).
- **Security**: Ed25519 backend signatures required for minting to verify off-chain clip ownership.
