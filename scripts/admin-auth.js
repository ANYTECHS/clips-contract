/**
 * admin-auth.js — Reusable authorization helper for administrator functions.
 *
 * Provides a single, consistent place to enforce that only the registered
 * contract admin may invoke privileged operations (pause, unpause, set_signer,
 * set_royalty, blacklist_clip, upgrade, etc.).
 *
 * Three consumption patterns are supported:
 *
 *   1. Express middleware  → `requireAdmin`
 *      Mount before any admin route handler. Reads `req.user.id` (set by the
 *      upstream auth middleware) and the stored admin address, and responds
 *      with 403 if the caller is not the admin.
 *
 *   2. Inline guard       → `assertAdmin(callerId, adminId)`
 *      Call inside any service function before executing privileged logic.
 *      Throws `AdminAuthError` on failure so callers can catch and translate
 *      it into whatever response format they need.
 *
 *   3. Typed error class  → `AdminAuthError`
 *      Lets catch blocks distinguish admin-authorization failures from other
 *      errors without relying on string matching.
 *
 * @module admin-auth
 */

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/**
 * Thrown / forwarded whenever a caller attempts an administrator operation
 * without holding the admin role.
 */
export class AdminAuthError extends Error {
  /**
   * @param {string} [message]
   */
  constructor(message = "Not authorized: caller is not the contract administrator") {
    super(message);
    this.name = "AdminAuthError";
    this.statusCode = 403;
  }
}

// ---------------------------------------------------------------------------
// Core assertion (framework-agnostic)
// ---------------------------------------------------------------------------

/**
 * Assert that `callerId` matches the expected `adminId`.
 *
 * This is the lowest-level helper and carries no framework dependency.
 * Use it inside service functions, CLI scripts, or anywhere Express is not
 * involved.
 *
 * @param {string | undefined} callerId   - The ID / address of the caller.
 * @param {string | undefined} adminId    - The registered administrator ID / address.
 * @throws {AdminAuthError} If `callerId` does not equal `adminId`, or if
 *   either value is missing / empty.
 *
 * @example
 * import { assertAdmin } from "./admin-auth.js";
 *
 * function pauseContract(callerId) {
 *   assertAdmin(callerId, process.env.ADMIN_ADDRESS);
 *   // ... proceed with privileged operation
 * }
 */
export function assertAdmin(callerId, adminId) {
  if (!callerId || !adminId) {
    throw new AdminAuthError(
      "Not authorized: caller identity or admin identity is missing"
    );
  }
  if (callerId !== adminId) {
    throw new AdminAuthError(
      `Not authorized: caller '${callerId}' is not the contract administrator`
    );
  }
}

// ---------------------------------------------------------------------------
// Express middleware factory
// ---------------------------------------------------------------------------

/**
 * Create an Express middleware that enforces admin-only access.
 *
 * The middleware reads `req.user.id` (populated by an upstream auth
 * middleware such as JWT verification) and compares it against `adminId`.
 * On failure it short-circuits with a 403 JSON response; on success it
 * calls `next()`.
 *
 * @param {string} adminId - The registered administrator ID / address.
 *   Typically loaded once from an environment variable or contract state
 *   and reused for the lifetime of the server process.
 * @returns {import('express').RequestHandler}
 *
 * @example
 * import express from "express";
 * import { makeRequireAdmin } from "./admin-auth.js";
 *
 * const app = express();
 * const requireAdmin = makeRequireAdmin(process.env.ADMIN_ADDRESS);
 *
 * // All routes under /admin are protected:
 * app.use("/admin", requireAdmin, adminRouter);
 *
 * // Or protect a single route:
 * app.post("/admin/pause", requireAdmin, pauseHandler);
 */
export function makeRequireAdmin(adminId) {
  /**
   * @param {import('express').Request}  req
   * @param {import('express').Response} res
   * @param {import('express').NextFunction} next
   */
  return function requireAdmin(req, res, next) {
    const callerId = req.user?.id;
    try {
      assertAdmin(callerId, adminId);
      next();
    } catch (err) {
      if (err instanceof AdminAuthError) {
        return res.status(err.statusCode).json({ error: err.message });
      }
      next(err);
    }
  };
}

// ---------------------------------------------------------------------------
// Convenience re-export for single-admin servers
// ---------------------------------------------------------------------------

/**
 * Pre-built Express middleware that reads the admin address from the
 * `ADMIN_ADDRESS` environment variable at import time.
 *
 * Use `makeRequireAdmin` instead if you need to supply the admin address
 * dynamically (e.g. from a database or contract state).
 *
 * @type {import('express').RequestHandler}
 *
 * @example
 * import { requireAdmin } from "./admin-auth.js";
 *
 * app.post("/admin/unpause", requireAdmin, unpauseHandler);
 */
export const requireAdmin = makeRequireAdmin(process.env.ADMIN_ADDRESS ?? "");
