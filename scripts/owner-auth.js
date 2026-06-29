/**
 * owner-auth.js — Reusable authorization helper for owner-only functions.
 *
 * Provides a single, consistent place to enforce that only the current owner
 * of a resource (NFT token, wallet, royalty record, etc.) may invoke
 * operations that are exclusive to that owner (burn, transfer, update
 * royalty recipient, disconnect wallet, etc.).
 *
 * Three consumption patterns are supported — mirroring the shape of
 * admin-auth.js so that both helpers stay ergonomically consistent:
 *
 *   1. Inline guard       → `assertOwner(callerId, ownerId)`
 *      Call inside any service function before executing owner-gated logic.
 *      Throws `OwnerAuthError` on failure so callers can catch and translate
 *      it into whatever response format they need.
 *
 *   2. Express middleware factory → `makeRequireOwner(getOwnerId)`
 *      Accepts a resolver function `getOwnerId(req)` that derives the
 *      expected owner ID from the request (e.g. by looking up the token in a
 *      database). Reads `req.user.id` (set by upstream auth middleware) and
 *      responds with 403 if the caller does not own the resource.
 *
 *   3. Typed error class  → `OwnerAuthError`
 *      Lets catch blocks distinguish owner-authorization failures from other
 *      errors without relying on string matching.
 *
 * @module owner-auth
 */

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/**
 * Thrown / forwarded whenever a caller attempts an owner-only operation
 * on a resource they do not own.
 */
export class OwnerAuthError extends Error {
  /**
   * @param {string} [message]
   */
  constructor(message = "Not authorized: caller is not the resource owner") {
    super(message);
    this.name = "OwnerAuthError";
    this.statusCode = 403;
  }
}

// ---------------------------------------------------------------------------
// Core assertion (framework-agnostic)
// ---------------------------------------------------------------------------

/**
 * Assert that `callerId` matches the expected `ownerId`.
 *
 * This is the lowest-level helper and carries no framework dependency.
 * Use it inside service functions, CLI scripts, or anywhere Express is not
 * involved — for example inside `burn`, `transfer`, or
 * `update_royalty_recipient` handlers.
 *
 * @param {string | undefined} callerId - The ID / address of the caller.
 * @param {string | undefined} ownerId  - The ID / address of the resource owner.
 * @throws {OwnerAuthError} If `callerId` does not equal `ownerId`, or if
 *   either value is missing / empty.
 *
 * @example
 * import { assertOwner } from "./owner-auth.js";
 *
 * function burnToken(callerId, token) {
 *   assertOwner(callerId, token.owner);
 *   // ... proceed with burn
 * }
 *
 * @example
 * import { assertOwner } from "./owner-auth.js";
 *
 * function transferToken(callerId, token, newOwner) {
 *   assertOwner(callerId, token.owner);
 *   token.owner = newOwner;
 * }
 */
export function assertOwner(callerId, ownerId) {
  if (!callerId || !ownerId) {
    throw new OwnerAuthError(
      "Not authorized: caller identity or owner identity is missing"
    );
  }
  if (callerId !== ownerId) {
    throw new OwnerAuthError(
      `Not authorized: caller '${callerId}' does not own this resource`
    );
  }
}

// ---------------------------------------------------------------------------
// Express middleware factory
// ---------------------------------------------------------------------------

/**
 * Create an Express middleware that enforces owner-only access.
 *
 * Because the owner of a resource is usually determined by the resource
 * itself (e.g. a token record in a database), you supply a `getOwnerId`
 * resolver that receives the current request and returns the expected owner
 * ID — either directly or as a Promise.
 *
 * The middleware reads `req.user.id` (set by an upstream auth middleware such
 * as JWT verification) and compares it against the resolved owner ID.
 * On mismatch it short-circuits with a 403 JSON response; on success it
 * calls `next()`.
 *
 * @param {(req: import('express').Request) => string | Promise<string>} getOwnerId
 *   A resolver that derives the expected owner ID from the request.
 *   Receives the full Express `Request` object so it can read route params,
 *   query strings, or attached state.
 * @returns {import('express').RequestHandler}
 *
 * @example
 * import express from "express";
 * import { makeRequireOwner } from "./owner-auth.js";
 * import { getTokenById } from "./token-service.js";
 *
 * const app = express();
 *
 * // Protect the burn endpoint — only the token owner may burn.
 * app.delete(
 *   "/tokens/:tokenId",
 *   makeRequireOwner(async (req) => {
 *     const token = await getTokenById(req.params.tokenId);
 *     return token?.owner;
 *   }),
 *   burnHandler,
 * );
 *
 * @example
 * // When the owner ID is already attached to the request by prior middleware:
 * app.post(
 *   "/tokens/:tokenId/transfer",
 *   makeRequireOwner((req) => req.resource?.owner),
 *   transferHandler,
 * );
 */
export function makeRequireOwner(getOwnerId) {
  /**
   * @param {import('express').Request}      req
   * @param {import('express').Response}     res
   * @param {import('express').NextFunction}  next
   */
  return async function requireOwner(req, res, next) {
    const callerId = req.user?.id;
    try {
      const ownerId = await getOwnerId(req);
      assertOwner(callerId, ownerId);
      next();
    } catch (err) {
      if (err instanceof OwnerAuthError) {
        return res.status(err.statusCode).json({ error: err.message });
      }
      next(err);
    }
  };
}
