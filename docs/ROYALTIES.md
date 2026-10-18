# ClipCash Royalty Guide

This document describes how royalties are configured, assigned, calculated, and
paid by the `clips_nft` Soroban contract. Amounts are expressed in the payment
asset's smallest unit.

## Royalty Configuration

Each token stores a `Royalty` value:

```rust
pub struct Royalty {
    pub recipients: Vec<RoyaltyRecipient>,
    pub asset_address: Option<Address>,
}

pub struct RoyaltyRecipient {
    pub recipient: Address,
    pub basis_points: u32,
}
```

`get_royalty(token_id)` reads the stored configuration. The configured asset
must be a supported SEP-0041 token for a payment with a non-zero amount.

## Basis Points

Basis points provide integer percentages without floating-point arithmetic:

| Basis points | Percentage |
|---:|---:|
| 0 | 0% |
| 100 | 1% |
| 500 | 5% |
| 10,000 | 100% |

The royalty limit is `MAX_ROYALTY_BPS = 10_000`. Values above it are rejected.
Multiple recipient values are summed for deduction validation.

## Default and Maximum Royalty

The contract-wide default is `DEFAULT_ROYALTY_BPS = 500` (5%) when no explicit
default has been stored. An administrator can call
`set_default_royalty_bps(admin, bps)` and read it with
`get_default_royalty_bps()`. A value of zero is valid and opts out of royalties.

The maximum per-recipient royalty is 10,000 bps. In addition, the total royalty
and platform-fee deductions for a payment may not exceed 10,000 bps or the sale
price. This prevents deductions from exceeding 100% of a sale.

## Royalty Assignment and Creator Attribution

During minting, the mint request supplies the royalty configuration and the
creator address. The atomic mint flow validates the royalty, stores it for the
new token, and records the creator association. The creator and initial owner
may be different, so creator attribution must be read from the creator data or
the mint event rather than inferred from current ownership.

`set_royalty(admin, token_id, royalty)` replaces a token's royalty
configuration. It is an administrative operation; marketplaces should call
`get_royalty` or `royalty_info` before relying on a value.

## Royalty Calculation

For a sale price `P` and basis points `B`, the amount is:

```text
royalty_amount = P * B / 10,000
```

The contract uses checked arithmetic. `royalty_info(token_id, sale_price)` is a
read-only preview that returns the receiver, calculated amount, and asset. The
multi-recipient payment path calculates each recipient's amount from its own
basis points and returns a `RoyaltyPaymentResult` containing total royalty,
platform fee, and payment records.

## Platform Fees

The platform fee is configured in basis points and defaults to zero. Its hard
limit is `MAX_PLATFORM_FEE_BPS = 1_000` (10%). The payment path reads the fee,
adds it to the total royalty deduction, and rejects the payment if the combined
deduction exceeds the sale price. The configured platform recipient is the
treasury wallet; cumulative fees are tracked as platform revenue.

## Payment Distribution

1. The payer authorizes the payment.
2. The contract loads and validates the token royalty and payment asset.
3. It calculates total royalty and platform fee amounts.
4. It transfers each positive royalty amount to its configured recipient.
5. It records payment history, updates cumulative earnings, and emits events.
6. It records applicable platform revenue for the configured treasury.

The payer supplies `sale_price` to `pay_royalty`. The amount is not a sale
escrow: the payer must hold and authorize the configured asset transfer.

## Payment History

Each payment is appended to `DataKey::RoyaltyHistory(token_id)` as a
`RoyaltyPayment` containing `token_id`, `recipient`, `amount`, and the ledger
`timestamp`. Use `get_royalty_history(token_id)` for the complete per-token
history and `get_cumulative_earnings(token_id)` for the cumulative royalty
amount. Zero-amount recipients do not create payment records.

## Royalty Events

Successful payments emit `RoyaltyPaidEvent` with:

- `token_id`
- `payer`
- `receiver`
- `amount`
- `asset_address`
- `timestamp`

Minting also emits `RoyaltyAssignedEvent` for the assigned recipient, basis
points, token ID, and assignment timestamp. Indexers should use event topics
and event data as the real-time feed, with storage reads for reconciliation.

## Authorization

Read-only queries do not require authorization. Configuration changes require
the configured administrator through `config_guard::require_config_admin`.
Royalty payments require authorization from the `payer`; a marketplace can
submit the call only when it has the payer's authorization. Minting separately
requires the configured mint authorization and valid backend signature.

Payment processing also derives a payment identifier from payer, token ID, and
sale price and rejects a replayed identifier. Integrators should use a unique
sale/payment flow and handle contract errors rather than retrying blindly.

## Security Considerations

- Treat recipient and asset addresses as untrusted input and validate them.
- Never assume a royalty is paid merely because `royalty_info` returned an
  amount; observe the transaction result and emitted event.
- Check both total recipient bps and the platform fee before displaying seller
  proceeds.
- Use checked integer arithmetic and reject non-positive or overflowing sale
  prices.
- Keep administrator and treasury keys secured; they can change configuration.
- Reconcile payment history, cumulative earnings, token transfers, and events.

## Emergency Controls

The administrator can pause the contract. While paused, state-changing
royalty operations guarded by the royalty pause guard must return
`Error::ContractPaused`; the global pause state is shared with other contract
operations. Unpause only after the cause has been investigated. Emergency
administrator support, where configured, should be treated as a privileged
operational key and audited like the primary administrator.

## Testing Strategy

Tests should cover:

- default, zero, boundary, and over-limit basis points;
- single and multi-recipient calculations, including rounding and overflow;
- unsupported assets and insufficient payer authorization;
- combined royalty plus platform fee limits;
- payment history, cumulative earnings, and event fields;
- replayed payment rejection;
- admin-only configuration and paused-contract behavior; and
- creator attribution when creator and owner differ.

The existing royalty tests in `clips_nft/tests/` provide examples for payment,
event, earnings, invalid asset, replay, and large-amount behavior. Run them
with:

```bash
cargo test -p clips_nft
```

## Example Royalty Workflow

For a 5% royalty and a 1,000,000-unit sale:

```rust
let info = client.royalty_info(&token_id, &1_000_000i128)?;
assert_eq!(info.royalty_amount, 50_000);

let result = client.pay_royalty(&payer, &token_id, &1_000_000i128)?;
assert_eq!(result.total_royalty, 50_000);
```

The marketplace first reads `royalty_info`, confirms that the asset and amount
match its sale, then submits `pay_royalty`. After confirmation it indexes the
`RoyaltyPaidEvent` and can reconcile it with `get_royalty_history` and
`get_cumulative_earnings`.