#!/usr/bin/env node
/**
 * Backend Mint Transaction Preparer
 *
 * This script shows how a backend Node.js service can:
 * 1. Accept a clip_id and wallet address
 * 2. Compute the canonical Ed25519 mint payload signature
 * 3. Build a mint transaction using the generated TypeScript bindings
 * 4. Sign the transaction envelope with the backend's Stellar key
 * 5. Output the signed XDR for the frontend/wallet to finalize
 *
 * Usage:
 *   node backend-mint.js <clip_id> <wallet_address>
 *
 * Prerequisites:
 *   - Build the bindings: cd ../contracts/clips_nft && npm install && npm run build
 *   - Install script deps: npm install
 *   - Set all required environment variables below
 *
 * Environment variables:
 *   CONTRACT_ID            - Deployed contract address (required)
 *   RPC_URL                - Soroban RPC endpoint (default: https://soroban-testnet.stellar.org)
 *   NETWORK_PASSPHRASE     - Network passphrase (default: Test SDF Network ; September 2015)
 *   BACKEND_STELLAR_SECRET - Backend Stellar secret key (required for envelope signing)
 *   BACKEND_ED25519_SECRET - Optional hex-encoded 32-byte Ed25519 seed. If omitted,
 *                            the seed is derived from BACKEND_STELLAR_SECRET.
 *   METADATA_URI_TEMPLATE  - URI template with {clip_id} placeholder
 *                            (default: ipfs://QmClip{clip_id})
 *   ROYALTY_BPS            - Default royalty basis points (default: 500 = 5%)
 *   IS_SOULBOUND           - Whether token is non-transferable (default: false)
 */

import { Client } from "@clipcash/clips-nft";
import {
  Address,
  Keypair,
  TransactionBuilder,
  nativeToScVal,
} from "@stellar/stellar-sdk";
import { createHash } from "crypto";
import nacl from "tweetnacl";

/* ------------------------------------------------------------------ */
/*  Input validation                                                  */
/* ------------------------------------------------------------------ */

const [clipIdArg, walletAddress] = process.argv.slice(2);

if (!clipIdArg || !walletAddress) {
  console.error("Usage: node backend-mint.js <clip_id> <wallet_address>");
  process.exit(1);
}

const clipId = Number(clipIdArg);
if (!Number.isInteger(clipId) || clipId < 0) {
  console.error("Invalid clip_id: must be a non-negative integer");
  process.exit(1);
}

const CONTRACT_ID = process.env.CONTRACT_ID;
if (!CONTRACT_ID) {
  console.error("Missing required env var: CONTRACT_ID");
  process.exit(1);
}

const RPC_URL = process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const NETWORK_PASSPHRASE =
  process.env.NETWORK_PASSPHRASE || "Test SDF Network ; September 2015";

const BACKEND_STELLAR_SECRET = process.env.BACKEND_STELLAR_SECRET;
if (!BACKEND_STELLAR_SECRET) {
  console.error("Missing required env var: BACKEND_STELLAR_SECRET");
  process.exit(1);
}

const METADATA_URI_TEMPLATE =
  process.env.METADATA_URI_TEMPLATE || "ipfs://QmClip{clip_id}";
const metadataUri = METADATA_URI_TEMPLATE.replace("{clip_id}", String(clipId));

const ROYALTY_BPS = Number(process.env.ROYALTY_BPS || "500");
const IS_SOULBOUND = String(process.env.IS_SOULBOUND || "false").toLowerCase() === "true";

/* ------------------------------------------------------------------ */
/*  Ed25519 payload signing (matches on-chain verify_clip_signature)  */
/* ------------------------------------------------------------------ */

/**
 * Build the 32-byte mint payload hash exactly as the contract does:
 *
 *   payload = SHA-256(
 *     clip_id_le_4_bytes
 *     || SHA-256( ScVal::Address(owner).to_xdr() )
 *     || SHA-256( ScVal::String(uri).to_xdr() )
 *   )
 *
 * In the Rust contract:
 *   owner_hash = sha256(owner.clone().to_xdr(env))   // ScVal XDR
 *   uri_hash   = sha256(metadata_uri.to_xdr(env))    // ScVal XDR
 */
function buildMintPayload(clipId, walletAddress, metadataUri) {
  const ownerXdr = new Address(walletAddress).toScVal().toXDR("raw");
  const uriXdr = nativeToScVal(metadataUri).toXDR("raw");

  const ownerHash = createHash("sha256").update(ownerXdr).digest();
  const uriHash = createHash("sha256").update(uriXdr).digest();

  const clipIdBuf = Buffer.allocUnsafe(4);
  clipIdBuf.writeUInt32LE(clipId, 0);

  const preimage = Buffer.concat([clipIdBuf, ownerHash, uriHash]);
  return createHash("sha256").update(preimage).digest();
}

/**
 * Sign the 32-byte payload with the backend Ed25519 key.
 */
function signMintPayload(payloadHash, secretSeed) {
  const seed = Buffer.isBuffer(secretSeed)
    ? secretSeed
    : Buffer.from(secretSeed, "hex");
  if (seed.length !== 32) {
    throw new Error(
      `Invalid Ed25519 secret seed length: expected 32, got ${seed.length}`
    );
  }
  const keypair = nacl.sign.keyPair.fromSeed(seed);
  const sig = nacl.sign.detached(payloadHash, keypair.secretKey);
  return Buffer.from(sig);
}

/* Derive the Ed25519 seed (backend signer for clip payload). */
let ed25519Seed;
if (process.env.BACKEND_ED25519_SECRET) {
  ed25519Seed = Buffer.from(process.env.BACKEND_ED25519_SECRET, "hex");
} else {
  // Stellar keys are Ed25519; the 32-byte raw seed works for both.
  ed25519Seed = Keypair.fromSecret(BACKEND_STELLAR_SECRET).rawSecretKey();
}

const payloadHash = buildMintPayload(clipId, walletAddress, metadataUri);
const signature = signMintPayload(payloadHash, ed25519Seed);

console.error(`[backend] clip_id=${clipId} owner=${walletAddress}`);
console.error(`[backend] metadata_uri=${metadataUri}`);
console.error(`[backend] payload_hash=${payloadHash.toString("hex")}`);
console.error(`[backend] signature=${signature.toString("hex")}`);

/* ------------------------------------------------------------------ */
/*  Build mint transaction via generated bindings                     */
/* ------------------------------------------------------------------ */

const backendKeypair = Keypair.fromSecret(BACKEND_STELLAR_SECRET);
const backendPublicKey = backendKeypair.publicKey();

const client = new Client({
  contractId: CONTRACT_ID,
  networkPassphrase: NETWORK_PASSPHRASE,
  rpcUrl: RPC_URL,
  publicKey: backendPublicKey,
});

const royalty = {
  recipients: [
    {
      recipient: walletAddress,
      basis_points: ROYALTY_BPS,
    },
  ],
  asset_address: undefined, // undefined => XLM (None)
};

try {
  const tx = await client.mint({
    to: walletAddress,
    clip_id: clipId,
    metadata_uri: metadataUri,
    royalty,
    is_soulbound: IS_SOULBOUND,
    signature,
  });

  /* ---------------------------------------------------------------- */
  /*  Sign envelope with backend key and output signed XDR            */
  /* ---------------------------------------------------------------- */

  const unsignedXdr = await tx.toXDR();
  const transaction = TransactionBuilder.fromXDR(unsignedXdr, NETWORK_PASSPHRASE);
  transaction.sign(backendKeypair);
  const signedXdr = transaction.toEnvelope().toXDR("base64");

  /*
   * NOTE: The output XDR has the transaction envelope signed by the backend
   * source account. However, because the mint() function calls
   * to.require_auth(), the wallet address (the 'to' argument) must still
   * sign its Soroban authorization entry before submission.
   *
   * Frontend flow:
   *   1. Pass this signedXDR to the wallet (e.g. Freighter)
   *   2. Wallet signs the auth entry for 'to'
   *   3. Wallet submits the fully-signed transaction
   *
   * If the wallet IS the backend (same address), the transaction is already
   * fully signed and can be submitted directly.
   */
  console.log(signedXdr);
} catch (err) {
  console.error("[backend] Failed to build or sign transaction:", err);
  process.exit(1);
}
