# ClipCash NFT — Frontend Wallet Integration Guide

This document explains how to connect the generated TypeScript bindings to a
Next.js / React frontend using Freighter (browser wallet) or any Stellar-compatible
signer.

---

## 1. Package overview

The bindings live in `contracts/clips_nft/` and are published as
`@clipcash/clips-nft`.  They expose a typed `Client` class that wraps every
contract entry-point and returns `AssembledTransaction` objects — thin wrappers
that let you **simulate** a call before optionally **signing and sending** it.

### Build the package locally

```bash
cd contracts/clips_nft
npm install
npm run build   # emits dist/index.js + dist/index.d.ts
```

### Add it to your Next.js app

```bash
# From the repo root
npm install ./contracts/clips_nft
```

Or add to `package.json`:

```json
"dependencies": {
  "@clipcash/clips-nft": "./contracts/clips_nft"
}
```

---

## 2. Initialise the client

```ts
// lib/contractClient.ts
import { Client } from "@clipcash/clips-nft";

const TESTNET_RPC   = "https://soroban-testnet.stellar.org";
const TESTNET_PASS  = "Test SDF Network ; September 2015";
const CONTRACT_ID   = process.env.NEXT_PUBLIC_CONTRACT_ID!; // set in .env.local

export function getClient(signerPublicKey?: string) {
  return new Client({
    contractId:        CONTRACT_ID,
    networkPassphrase: TESTNET_PASS,
    rpcUrl:            TESTNET_RPC,
    // publicKey is optional for read-only calls
    ...(signerPublicKey ? { publicKey: signerPublicKey } : {}),
  });
}
```

---

## 3. Read-only calls (no wallet needed)

```ts
import { getClient } from "@/lib/contractClient";

// Total tokens minted
const client = getClient();
const tx     = await client.total_supply();
const supply: number = tx.result;           // u32 — no signing required

// Token owner
const ownerTx = await client.owner_of({ token_id: 1 });
const owner: string = ownerTx.result.unwrap();

// Metadata URI
const uriTx   = await client.token_uri({ token_id: 1 });
const uri: string = uriTx.result.unwrap();

// Full royalty struct
const royaltyTx = await client.get_royalty({ token_id: 1 });
const royalty   = royaltyTx.result.unwrap();

// Look up token by off-chain clip id
const tokenIdTx = await client.clip_token_id({ clip_id: 42 });
const tokenId   = tokenIdTx.result.unwrap();

// Average synthetic gas cost (0 = mint, 1 = transfer)
const gasTx = await client.get_avg_gas_cost({ op_type: 0 });
const avgGas: bigint = gasTx.result;
```

---

## 4. Signed transactions with Freighter

Install the Freighter API:

```bash
npm install @stellar/freighter-api
```

### 4a. Connect wallet helper

```ts
// lib/wallet.ts
import {
  isConnected,
  getPublicKey,
  signTransaction,
} from "@stellar/freighter-api";

export async function connectFreighter(): Promise<string | null> {
  if (!(await isConnected())) return null;
  return getPublicKey();
}

export { signTransaction };
```

### 4b. Mint an NFT

```ts
import { getClient }       from "@/lib/contractClient";
import { connectFreighter, signTransaction } from "@/lib/wallet";
import type { Royalty }    from "@clipcash/clips-nft";

const TESTNET_PASS = "Test SDF Network ; September 2015";

async function mintClip(
  clipId:      number,
  metadataUri: string,
  royalty:     Royalty,
  isSoulbound: boolean,
  signature:   Buffer,   // 64-byte Ed25519 sig from your backend
) {
  const publicKey = await connectFreighter();
  if (!publicKey) throw new Error("Wallet not connected");

  const client = getClient(publicKey);

  // Build + simulate
  const tx = await client.mint({
    to:           publicKey,
    clip_id:      clipId,
    metadata_uri: metadataUri,
    royalty,
    is_soulbound: isSoulbound,
    signature,
  });

  // Sign with Freighter, then send
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });

  const result = await tx.sign({ signedAuthEntries: signedXdr });
  return result.result.unwrap(); // returns TokenId (number)
}
```

### 4c. Transfer a token

```ts
async function transferToken(to: string, tokenId: number) {
  const publicKey = await connectFreighter();
  const client    = getClient(publicKey!);

  const tx = await client.transfer({ from: publicKey!, to, token_id: tokenId });
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });
  await tx.sign({ signedAuthEntries: signedXdr });
}
```

### 4d. Burn a token

```ts
async function burnToken(tokenId: number) {
  const publicKey = await connectFreighter();
  const client    = getClient(publicKey!);

  const tx = await client.burn({ owner: publicKey!, token_id: tokenId });
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });
  await tx.sign({ signedAuthEntries: signedXdr });
}
```

---

## 5. Royalty helpers

### Get royalty info for a sale price

```ts
const infoTx = await client.royalty_info({ token_id: 1, sale_price: 1_000_000n });
const info   = infoTx.result.unwrap();
// info.receiver         — primary recipient address
// info.royalty_amount   — amount owed (same unit as sale_price)
// info.asset_address    — undefined → XLM; string → SEP-0041 token contract
```

### Pay royalties on a sale

```ts
async function payRoyalties(tokenId: number, salePrice: bigint) {
  const publicKey = await connectFreighter();
  const client    = getClient(publicKey!);

  const tx = await client.pay_royalty({
    payer:      publicKey!,
    token_id:   tokenId,
    sale_price: salePrice,
  });
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });
  await tx.sign({ signedAuthEntries: signedXdr });
}
```

---

## 6. Admin operations

These require the contract admin address to authorise.

```ts
// Pause / unpause minting and transfers
await (await client.pause({ admin })).signAndSend();
await (await client.unpause({ admin })).signAndSend();

// Register or rotate the backend Ed25519 signer
await (await client.set_signer({ admin, pubkey: Buffer.from(pubkeyBytes) })).signAndSend();

// Update royalty configuration for a token
await (await client.set_royalty({ admin, token_id: 1, new_royalty: royalty })).signAndSend();

// Blacklist a clip ID (e.g. copyright violation) — prevents future minting
await (await client.blacklist_clip({ admin, clip_id: 42 })).signAndSend();
```

---

## 7. Creator operations

### Update royalty recipient (Issue #73)

Creators can update their royalty recipient address (e.g. if they change wallets).
Only the **current primary royalty recipient** may call this.

```ts
async function updateRoyaltyRecipient(tokenId: number, newRecipient: string) {
  const publicKey = await connectFreighter();
  const client    = getClient(publicKey!);

  const tx = await client.update_royalty_recipient({
    caller:        publicKey!,
    token_id:      tokenId,
    new_recipient: newRecipient,
  });
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });
  await tx.sign({ signedAuthEntries: signedXdr });
}
```

### Batch mint

Mint multiple clips in a single transaction.

```ts
async function batchMint(
  clipIds:      number[],
  metadataUris: string[],
  royalty:      Royalty,
  isSoulbound:  boolean,
  signatures:   Buffer[],
) {
  const publicKey = await connectFreighter();
  const client    = getClient(publicKey!);

  const tx = await client.batch_mint({
    to:            publicKey!,
    clip_ids:      clipIds,
    metadata_uris: metadataUris,
    royalty,
    is_soulbound:  isSoulbound,
    signatures,
  });
  const signedXdr = await signTransaction(tx.toXDR(), {
    networkPassphrase: TESTNET_PASS,
  });
  const result = await tx.sign({ signedAuthEntries: signedXdr });
  return result.result.unwrap(); // returns TokenId[]
}
```

### Query tokens owned by an address

```ts
const tokensTx = await client.tokens_of_owner({ owner: publicKey });
const tokenIds: number[] = tokensTx.result; // up to 1000 token IDs
```

### Calculate royalty amount

```ts
const amountTx = await client.calculate_royalty_amount({
  token_id:   1,
  sale_price: 1_000_000n,
});
const royaltyAmount: bigint = amountTx.result.unwrap();
```

---

## 7. Type reference

| TypeScript type | Soroban type | Notes |
|---|---|---|
| `u32` / `number` | `u32` | Token IDs, clip IDs, basis points |
| `u64` / `bigint` | `u64` | Gas counters |
| `i128` / `bigint` | `i128` | Sale prices, royalty amounts |
| `string` | `Address` | Stellar account / contract address |
| `Buffer` | `BytesN<32>` / `BytesN<64>` | Ed25519 pub key (32 B) & signature (64 B) |
| `Royalty` | struct | `{ recipients: RoyaltyRecipient[], asset_address?: string }` |
| `RoyaltyRecipient` | struct | `{ recipient: string, basis_points: number }` |
| `RoyaltyInfo` | struct | `{ receiver, royalty_amount, asset_address? }` |
| `TokenData` | struct | `{ owner, clip_id, is_soulbound, metadata_uri, royalty }` |

---

## 8. Error codes

| Code | Name | When thrown |
|---|---|---|
| 1 | `Unauthorized` | Caller is not admin / owner |
| 2 | `InvalidTokenId` | Token does not exist |
| 3 | `TokenAlreadyMinted` | `clip_id` already has a token |
| 4 | `RoyaltyTooHigh` | Total basis points > 10 000 |
| 5 | `InvalidRecipient` | Recipient address is invalid |
| 6 | `InvalidSalePrice` | `sale_price ≤ 0` |
| 7 | `ContractPaused` | Minting / transfers blocked |
| 8 | `InvalidSignature` | Backend Ed25519 signature failed |
| 9 | `SignerNotSet` | No signer registered via `set_signer` |
| 10 | `InvalidRoyaltySplit` | Empty or malformed royalty split |
| 11 | `SoulboundTransferBlocked` | Token is non-transferable |
| 12 | `RoyaltyOverflow` | `sale_price` too large for safe calculation |
| 13 | `ClipBlacklisted` | Clip ID has been blacklisted by admin |
| 14 | `NotAuthorizedToApprove` | Caller cannot approve this token |
| 15 | `WithdrawalStillLocked` | 24h timelock has not elapsed |
| 16 | `NoWithdrawalRequest` | No pending withdrawal request |

Handle errors like this:

```ts
const result = tx.result;
if (result.isErr()) {
  const errCode = result.error.value; // number (1–12)
  console.error("Contract error", errCode);
}
```

---

## 9. Environment variables

Add to `.env.local` (never commit):

```bash
NEXT_PUBLIC_CONTRACT_ID=C...               # deployed contract address
NEXT_PUBLIC_STELLAR_RPC=https://soroban-testnet.stellar.org
NEXT_PUBLIC_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
```

---

## 10. Regenerating bindings

After any contract change, rebuild the WASM and regenerate:

```bash
# From repo root
cargo build --target wasm32v1-none --release -p clips_nft

stellar contract bindings typescript \
  --wasm target/wasm32v1-none/release/clips_nft.wasm \
  --output-dir contracts/clips_nft \
  --overwrite

# Then rebuild the JS package
cd contracts/clips_nft && npm run build
```

Or use the Makefile shortcut:

```bash
make bindings
```
