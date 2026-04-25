import { Keypair, Address, xdr, Networks } from '@stellar/stellar-sdk';
import { createHash } from 'crypto';
import { Client } from './dist/index.js'; // Requires 'npm run build' first

/**
 * Demonstrates how a backend can prepare and sign a mint transaction
 * for the Clips NFT contract.
 * 
 * Requirements:
 * 1. Takes clip_id and wallet address
 * 2. Uses generated bindings
 * 3. Outputs signed XDR
 */
async function main() {
  // 1. Setup Configuration
  const CONTRACT_ID = 'CD...'; // Replace with actual contract ID
  const RPC_URL = 'https://soroban-testnet.stellar.org';
  const NETWORK_PASSPHRASE = Networks.TESTNET;
  
  // Backend Signer Secret (Keep this secure!)
  const BACKEND_SECRET = 'S...'; 
  const backendKeypair = Keypair.fromSecret(BACKEND_SECRET);

  // User details for minting - can be passed as CLI arguments
  const userAddress = process.argv[2] || 'GB...'; // The wallet that will own the NFT
  const clipId = parseInt(process.argv[3]) || 12345;
  const metadataUri = process.argv[4] || 'ipfs://Qm...';

  if (!process.argv[2]) {
    console.log('Usage: node example-mint.js <wallet_address> <clip_id> [metadata_uri]');
    console.log('Using default values for demonstration...\n');
  }

  console.log(`Preparing mint for Clip ID: ${clipId}, User: ${userAddress}`);

  // 2. Prepare the Payload for Signing
  // The contract expects: SHA-256(clip_id_le || SHA-256(owner_xdr) || SHA-256(uri_xdr))
  
  // Hash the owner address (SCAddress XDR)
  const ownerXdr = new Address(userAddress).toScAddress().toXDR();
  const ownerHash = createHash('sha256').update(ownerXdr).digest();

  // Hash the metadata URI (SCVal String XDR)
  const uriXdr = xdr.ScVal.scvString(metadataUri).toXDR();
  const uriHash = createHash('sha256').update(uriXdr).digest();

  // Concatenate: clip_id (4 bytes LE) + ownerHash (32 bytes) + uriHash (32 bytes)
  const clipIdBuf = Buffer.alloc(4);
  clipIdBuf.writeUInt32LE(clipId);
  
  const preimage = Buffer.concat([clipIdBuf, ownerHash, uriHash]);
  const message = createHash('sha256').update(preimage).digest();

  // 3. Sign the payload
  const signature = backendKeypair.sign(message);
  console.log('Backend signature generated:', signature.toString('hex'));

  // 4. Use Generated Bindings to Prepare Transaction
  const client = new Client({
    networkPassphrase: NETWORK_PASSPHRASE,
    contractId: CONTRACT_ID,
    rpcUrl: RPC_URL,
  });

  // Define royalty (example: 5% to the user)
  const royalty = {
    asset_address: null, // XLM
    recipients: [
      {
        recipient: userAddress,
        basis_points: 500, // 5%
      }
    ]
  };

  try {
    // Construct the mint transaction
    // Note: This simulates the transaction on-chain to check for errors
    const tx = await client.mint({
      to: userAddress,
      clip_id: clipId,
      metadata_uri: metadataUri,
      royalty: royalty,
      is_soulbound: false,
      signature: signature,
    }, {
      // For backend preparation, we often just want the XDR
      // without necessarily having the source account's signature yet
      // if it's being sent to the user for signing.
      // But here we'll assume the backend/service pays for it.
      publicKey: backendKeypair.publicKey(), 
    });

    // 5. Sign and Output XDR
    // In a real backend scenario, you might sign with a fee-payer account
    await tx.sign(backendKeypair);
    
    const signedXdr = tx.toEnvelope().toXDR('base64');
    
    console.log('\n--- SIGNED XDR ---');
    console.log(signedXdr);
    console.log('------------------\n');
    console.log('You can now submit this XDR to the network or send it to the user.');

  } catch (err) {
    console.error('Error preparing mint transaction:', err);
  }
}

// Run the script
main().catch(console.error);
