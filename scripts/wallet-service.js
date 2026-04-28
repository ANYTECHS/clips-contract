/**
 * wallet-service.js — Wallet management service.
 *
 * Issue #165: Users should be able to disconnect a previously connected
 * Stellar wallet.
 *
 * DELETE /wallets/:id
 *   - Ownership check: only the authenticated user who owns the wallet may
 *     disconnect it.
 *   - Guard: refuses disconnect if the wallet has active (minted) NFTs or
 *     pending payouts.
 *   - Soft-delete: sets deletedAt timestamp rather than removing the record.
 *   - Returns a JSON success message on success.
 *
 * This module exports a plain handler function so it can be mounted on any
 * Express-compatible router.
 */

/** @type {Map<string, Wallet>} */
const wallets = new Map();

/**
 * @typedef {Object} Wallet
 * @property {string}    id
 * @property {string}    userId        - Owner's user ID.
 * @property {string}    address       - Stellar G-address.
 * @property {boolean}   hasActiveNFTs - True if wallet holds minted NFTs.
 * @property {boolean}   hasPendingPayouts - True if payouts are pending.
 * @property {Date|null} deletedAt     - Set on soft-delete.
 */

/**
 * Register a wallet (used in tests / seeding).
 * @param {string} id
 * @param {string} userId
 * @param {string} address
 * @returns {Wallet}
 */
function registerWallet(id, userId, address) {
  const wallet = {
    id,
    userId,
    address,
    hasActiveNFTs: false,
    hasPendingPayouts: false,
    deletedAt: null,
  };
  wallets.set(id, wallet);
  return wallet;
}

/**
 * Express-style handler for DELETE /wallets/:id.
 *
 * Expects `req.user.id` to be set by upstream auth middleware.
 *
 * @param {import('express').Request}  req
 * @param {import('express').Response} res
 */
function disconnectWallet(req, res) {
  const { id } = req.params;
  const requestingUserId = req.user?.id;

  const wallet = wallets.get(id);

  if (!wallet || wallet.deletedAt) {
    return res.status(404).json({ error: "Wallet not found" });
  }

  if (wallet.userId !== requestingUserId) {
    return res.status(403).json({ error: "Not authorized to disconnect this wallet" });
  }

  if (wallet.hasActiveNFTs) {
    return res.status(409).json({ error: "Cannot disconnect wallet with active NFTs" });
  }

  if (wallet.hasPendingPayouts) {
    return res.status(409).json({ error: "Cannot disconnect wallet with pending payouts" });
  }

  wallet.deletedAt = new Date();
  return res.status(200).json({ message: "Wallet disconnected successfully" });
}

export { registerWallet, disconnectWallet, wallets };
