/**
 * nft-ownership.js — On-chain NFT ownership verification.
 *
 * Issue #164: Before allowing certain actions (e.g. editing a minted clip),
 * verify the caller still owns the NFT on-chain by querying the Soroban
 * contract's owner_of() function.
 *
 * Environment variables:
 *   CONTRACT_ID        - Deployed ClipCashNFT contract address (required)
 *   RPC_URL            - Soroban RPC endpoint (default: https://soroban-testnet.stellar.org)
 *   NETWORK_PASSPHRASE - Network passphrase (default: Test SDF Network ; September 2015)
 */

import { Client } from "@clipcash/clips-nft";

const CONTRACT_ID = process.env.CONTRACT_ID;
const RPC_URL = process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const NETWORK_PASSPHRASE =
  process.env.NETWORK_PASSPHRASE || "Test SDF Network ; September 2015";

/**
 * Verify that `walletAddress` is the current on-chain owner of the token
 * identified by `tokenId` (the numeric token ID returned at mint time).
 *
 * @param {number} tokenId       - On-chain token ID (u32).
 * @param {string} walletAddress - Stellar G-address to check ownership for.
 * @returns {Promise<{ owned: boolean, error: string|null }>}
 */
async function verifyNFTOwnership(tokenId, walletAddress) {
  if (!CONTRACT_ID) {
    return { owned: false, error: "CONTRACT_ID environment variable is not set" };
  }

  const client = new Client({
    contractId: CONTRACT_ID,
    networkPassphrase: NETWORK_PASSPHRASE,
    rpcUrl: RPC_URL,
  });

  try {
    const tx = await client.owner_of({ token_id: tokenId });
    const result = await tx.simulate();

    if (result.result.isErr()) {
      const errMsg = result.result.unwrapErr().message ?? "InvalidTokenId";
      return { owned: false, error: errMsg };
    }

    const owner = result.result.unwrap();
    return { owned: owner === walletAddress, error: null };
  } catch (err) {
    return { owned: false, error: err.message ?? String(err) };
  }
}

export { verifyNFTOwnership };
