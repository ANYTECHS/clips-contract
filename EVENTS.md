# ClipCash Contract Event Reference

This document is the canonical event contract for frontend listeners, indexers, and explorer integrations.

## Event Emission Standard

- Every important state-changing path emits an explicit event.
- Topic 0 is always a short event symbol (`symbol_short!`).
- Additional topic fields are indexed dimensions for efficient filtering (token ID, addresses, admin, etc.).
- Event payload contains full business context.

## Event Catalog

| Symbol | Indexed Topics (topic tuple) | Payload Type | Emitted By |
|---|---|---|---|
| `mint` | `(mint, token_id, to)` | `MintEvent` | `mint` |
| `transfer` | `(transfer, token_id, from, to)` | `TransferEvent` | `mint`, `transfer`, `burn`, `batch_burn` |
| `burn` | `(burn, token_id, owner)` | `BurnEvent` | `burn`, `batch_burn` |
| `approval` | `(approval, token_id, owner, operator)` | `ApprovalEvent` | `approve` |
| `app_all` | `(app_all, owner, operator)` | `ApprovalForAllEvent` | `revoke_all_approvals` |
| `royalty` | `(royalty, token_id, from, to)` | `RoyaltyPaidEvent` | `transfer`, `pay_royalty` |
| `royalty` | `(royalty, token_id, old_recipient, new_recipient)` | `RoyaltyRecipientUpdatedEvent` | `set_royalty`, `update_royalty_recipient` |
| `roy_upd` | `(roy_upd, token_id)` | `RoyaltyUpdatedEvent` | `set_royalty` |
| `roy_clm` | `(roy_clm, token_id, recipient)` | `RoyaltyClaimedEvent` | `claim_royalties` |
| `refunded` | `(refunded, token_id, recipient)` | `RefundedEvent` | burn/refund flow |
| `meta_upd` | `(meta_upd, token_id)` | `MetadataUpdatedEvent` | `refresh_metadata` |
| `blacklist` | `(blacklist, clip_id)` | `BlacklistEvent` | `blacklist_clip` |
| `freeze` | `(freeze, token_id)` | `TokenFrozenEvent` | `freeze` |
| `unfreeze` | `(unfreeze, token_id)` | `TokenUnfrozenEvent` | `unfreeze` |
| `sb_recov` | `(sb_recov, token_id, old_owner, new_owner)` | `SoulboundRecoveredEvent` | `recover_soulbound` |
| `sgn_upd` | `(sgn_upd, admin)` | `SignerUpdatedEvent` | `set_signer` |
| `adm_chg` | `(adm_chg, old_admin, new_admin)` | `AdminChangedEvent` | `set_admin` |
| `upgrade` | `(upgrade, admin)` | `UpgradeEvent` | `upgrade` |
| `pse_sched` | `(pse_sched, admin)` | `PauseScheduledEvent` | `pause` |
| `unpaused` | `(unpaused, admin)` | `()` | `unpause` |
| `with_req` | `(with_req, admin)` | `WithdrawRequestedEvent` | `request_withdraw_asset` |
| `with_exe` | `(with_exe, admin, asset)` | `WithdrawExecutedEvent` | `withdraw_asset` |
| `cb_ena` | `(cb_ena, admin)` | `CircuitBreakerEnabledEvent` | `set_circuit_breaker_enabled` |
| `cb_thr` | `(cb_thr, admin)` | `CircuitBreakerThresholdUpdatedEvent` | `set_circuit_breaker_threshold` |
| `cb_win` | `(cb_win, admin)` | `CircuitBreakerWindowUpdatedEvent` | `set_circuit_breaker_window` |
| `cb_rst` | `(cb_rst, admin)` | `CircuitBreakerResetEvent` | `reset_circuit_breaker` |
| `circuit` | `(circuit, threshold)` | `CircuitBreakerTriggeredEvent` | auto-trigger in mint guard |
| `batch_mnt` | `(batch_mnt, to, first_token_id)` | `BatchMintEvent` | `batch_mint` |

## Consumer Guidance

- Filter by symbol for broad streams (`transfer`, `royalty`, `mint`).
- Filter by indexed token ID to build per-token history efficiently.
- Filter by indexed address fields (`from`, `to`, `owner`, `recipient`, `admin`) for wallet/admin timelines.
- Distinguish the two `royalty` variants by payload type:
  - `RoyaltyPaidEvent`
  - `RoyaltyRecipientUpdatedEvent`

## Frontend/Indexer Checklist

- Subscribe to: `mint`, `transfer`, `burn`, `royalty`, `roy_upd`, `meta_upd`, `pse_sched`, `unpaused`, `upgrade`.
- Treat events as the source of truth for UI state transitions.
- Use idempotent handlers keyed by tx hash + event index to avoid duplicates.
- Rebuild entity snapshots from events for explorer and analytics consistency.
