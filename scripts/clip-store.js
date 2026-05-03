/**
 * clip-store.js — In-memory Clip store with NFT tracking fields.
 *
 * Issue #166: Track which clips have been minted and their on-chain mint
 * address. Adds mintAddress, mintedAt, and nftStatus to each Clip record
 * and prevents double-minting the same clip.
 *
 * nftStatus values: "none" | "minting" | "minted" | "failed"
 */

/** @type {Map<number, Clip>} */
const clips = new Map();

/**
 * @typedef {Object} Clip
 * @property {number}      id
 * @property {string}      title
 * @property {string|null} mintAddress  - On-chain token/contract address after mint.
 * @property {Date|null}   mintedAt     - Timestamp of successful mint.
 * @property {string}      nftStatus   - "none" | "minting" | "minted" | "failed"
 */

/**
 * Create a new clip record.
 * @param {number} id
 * @param {string} title
 * @returns {Clip}
 */
function createClip(id, title) {
  if (clips.has(id)) throw new Error(`Clip ${id} already exists`);
  const clip = { id, title, mintAddress: null, mintedAt: null, nftStatus: "none" };
  clips.set(id, clip);
  return clip;
}

/**
 * Retrieve a clip by ID.
 * @param {number} id
 * @returns {Clip}
 */
function getClip(id) {
  const clip = clips.get(id);
  if (!clip) throw new Error(`Clip ${id} not found`);
  return clip;
}

/**
 * Mark a clip as currently being minted.
 * Throws if the clip is already minted or in-progress (double-mint prevention).
 * @param {number} id
 */
function beginMint(id) {
  const clip = getClip(id);
  if (clip.nftStatus === "minted") {
    throw new Error(`Clip ${id} has already been minted (mintAddress: ${clip.mintAddress})`);
  }
  if (clip.nftStatus === "minting") {
    throw new Error(`Clip ${id} mint is already in progress`);
  }
  clip.nftStatus = "minting";
}

/**
 * Record a successful mint result.
 * @param {number} id
 * @param {string} mintAddress - On-chain token address / token ID string.
 */
function completeMint(id, mintAddress) {
  const clip = getClip(id);
  clip.mintAddress = mintAddress;
  clip.mintedAt = new Date();
  clip.nftStatus = "minted";
}

/**
 * Mark a mint as failed, allowing a retry.
 * @param {number} id
 */
function failMint(id) {
  const clip = getClip(id);
  clip.nftStatus = "failed";
}

export { createClip, getClip, beginMint, completeMint, failMint };
