/**
 * subscription-listener.js — Stellar asset payment verification listener.
 *
 * Issue #167: Listen for incoming Stellar payments for subscriptions and
 * auto-activate the plan.
 *
 * Behaviour:
 *   - Polls Horizon for new payments to the platform's receiving address.
 *   - Matches each payment by memo (subscription ID) and expected amount.
 *   - Updates the matching Subscription record to status "active".
 *   - Creates a Payout record for platform revenue (stub for future use).
 *   - Tracks the last-seen cursor so restarts don't reprocess old payments.
 *
 * Environment variables:
 *   HORIZON_URL          - Horizon base URL (default: https://horizon-testnet.stellar.org)
 *   PLATFORM_ADDRESS     - Stellar G-address that receives subscription payments (required)
 *   POLL_INTERVAL_MS     - Polling interval in ms (default: 10000)
 */

const HORIZON_URL = process.env.HORIZON_URL || "https://horizon-testnet.stellar.org";
const PLATFORM_ADDRESS = process.env.PLATFORM_ADDRESS;
const POLL_INTERVAL_MS = Number(process.env.POLL_INTERVAL_MS || "10000");

/** @type {Map<string, Subscription>} */
const subscriptions = new Map();

/** @type {Payout[]} */
const payouts = [];

/**
 * @typedef {Object} Subscription
 * @property {string} id
 * @property {string} userId
 * @property {string} status       - "pending" | "active" | "cancelled"
 * @property {string} expectedAmount - Amount in XLM (e.g. "10.0000000")
 */

/**
 * @typedef {Object} Payout
 * @property {string} subscriptionId
 * @property {string} amount
 * @property {Date}   createdAt
 */

/** Cursor for Horizon pagination — tracks last processed payment. */
let cursor = "now";

/**
 * Register a pending subscription (called when user initiates checkout).
 * @param {string} id
 * @param {string} userId
 * @param {string} expectedAmount - e.g. "10.0000000"
 * @returns {Subscription}
 */
function registerSubscription(id, userId, expectedAmount) {
  const sub = { id, userId, status: "pending", expectedAmount };
  subscriptions.set(id, sub);
  return sub;
}

/**
 * Fetch one page of payments from Horizon for the platform address.
 * @returns {Promise<Array>}
 */
async function fetchPayments() {
  const url =
    `${HORIZON_URL}/accounts/${PLATFORM_ADDRESS}/payments` +
    `?order=asc&limit=50&cursor=${cursor}&include_failed=false`;

  const res = await fetch(url);
  if (!res.ok) throw new Error(`Horizon error ${res.status}: ${await res.text()}`);

  const body = await res.json();
  return body._embedded?.records ?? [];
}

/**
 * Process a single Horizon payment record.
 * @param {Object} payment
 */
function processPayment(payment) {
  // Only handle native XLM payments (type "payment", asset_type "native")
  if (payment.type !== "payment" || payment.asset_type !== "native") return;
  if (payment.to !== PLATFORM_ADDRESS) return;

  const memo = payment.transaction?.memo ?? payment.memo;
  const amount = payment.amount;

  const sub = subscriptions.get(memo);
  if (!sub) return; // No matching subscription

  if (sub.status !== "pending") return; // Already processed

  if (sub.expectedAmount && sub.expectedAmount !== amount) {
    console.warn(
      `[subscription-listener] Amount mismatch for sub ${memo}: ` +
      `expected ${sub.expectedAmount}, got ${amount}`
    );
    return;
  }

  sub.status = "active";
  console.log(`[subscription-listener] Activated subscription ${memo} for user ${sub.userId}`);

  // Stub: create a payout record for platform revenue tracking
  payouts.push({ subscriptionId: memo, amount, createdAt: new Date() });
}

/**
 * Run one polling cycle: fetch new payments and process each one.
 */
async function poll() {
  try {
    const records = await fetchPayments();
    for (const payment of records) {
      processPayment(payment);
      cursor = payment.paging_token;
    }
  } catch (err) {
    console.error("[subscription-listener] Poll error:", err.message);
  }
}

/** @type {ReturnType<typeof setInterval>|null} */
let timer = null;

/**
 * Start the subscription payment listener.
 * @param {string} [startCursor="now"] - Horizon paging cursor to start from.
 */
function startListener(startCursor = "now") {
  if (!PLATFORM_ADDRESS) {
    throw new Error("PLATFORM_ADDRESS environment variable is required");
  }
  cursor = startCursor;
  console.log(`[subscription-listener] Starting — polling every ${POLL_INTERVAL_MS}ms`);
  timer = setInterval(poll, POLL_INTERVAL_MS);
  // Run immediately on start
  poll();
}

/** Stop the listener. */
function stopListener() {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}

export {
  startListener,
  stopListener,
  registerSubscription,
  subscriptions,
  payouts,
  // exported for testing
  processPayment,
  poll,
};
