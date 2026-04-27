# ClipCash NFT — Audit Checklist

Closes #97

## Scope

| File | Description |
|------|-------------|
| `clips_nft/src/lib.rs` | ClipCashNFT Soroban contract (sole in-scope file) |

---

## 1. Access Control

- [ ] `init` — can only be called once; panics on re-initialization
- [ ] `set_signer` — admin only; verify non-admin call is rejected
- [ ] `upgrade` — admin only; verify non-admin call is rejected
- [ ] `pause` / `unpause` — admin only; verify non-admin call is rejected
- [ ] `blacklist_clip` — admin only; verify non-admin call is rejected
- [ ] `set_name` / `set_symbol` — admin only
- [ ] `set_royalty` — admin only; verify non-admin call is rejected
- [ ] `mint` — caller must be `to` (self-mint only); `require_auth` enforced
- [ ] `transfer` — `from` must be owner; `require_auth` enforced
- [ ] `transfer_from` — spender must be approved-for-all or per-token approved
- [ ] `burn` — owner only; `require_auth` enforced
- [ ] `approve` — caller must be owner or approved-for-all
- [ ] `set_approval_for_all` — caller must be owner; `require_auth` enforced
- [ ] `set_token_uri` — owner only; `require_auth` enforced
- [ ] `pay_royalty` — payer must authorize; no admin restriction

---

## 2. Signature Verification

- [ ] `verify_clip_signature` uses `env.crypto().ed25519_verify` (traps on failure)
- [ ] Payload binds `clip_id`, `owner`, and `metadata_uri` — no replay across fields
- [ ] Signer rotation via `set_signer` immediately invalidates old signatures
- [ ] `SignerNotSet` error returned before any state mutation when signer absent
- [ ] Signature over wrong owner address causes trap (not silent pass)
- [ ] Signature over wrong `clip_id` causes trap

---

## 3. Minting

- [ ] `ClipIdMinted` dedup guard prevents double-minting the same `clip_id`
- [ ] Blacklisted `clip_id` cannot be minted (`ClipBlacklisted` error)
- [ ] `NextTokenId` increments atomically; no ID collision possible
- [ ] Platform 1 % royalty appended by `normalize_royalty` if not present
- [ ] Total royalty basis points validated ≤ 10 000

---

## 4. Transfers & Approvals

- [ ] Soulbound tokens (`is_soulbound = true`) cannot be transferred or `transfer_from`'d
- [ ] Per-token approval cleared on every successful transfer
- [ ] `transfer_from` checks both approved-for-all and per-token approval
- [ ] Paused contract blocks `mint`, `transfer`, `transfer_from`, `approve`, `set_approval_for_all`

---

## 5. Royalty Calculation

- [ ] `calculate_royalty` guards against overflow: `sale_price > i128::MAX / 10_000` → `RoyaltyOverflow`
- [ ] Zero or negative `sale_price` returns `InvalidSalePrice`
- [ ] `pay_royalty` only handles SEP-0041 assets; XLM royalties return `InvalidRecipient`
- [ ] Cumulative split math prevents double-paying rounding dust
- [ ] `set_royalty` emits `RoyaltyRecipientUpdatedEvent` only when primary recipient changes

---

## 6. Storage

- [ ] Instance storage keys: `Admin`, `NextTokenId`, `Paused`, `Signer`, `Name`, `Symbol`, `PlatformRecipient`
- [ ] Persistent storage keys: `Token(id)`, `ClipIdMinted(clip_id)`, `Approved(id)`, `ApprovalForAll(owner,op)`, `BlacklistedClip(clip_id)`
- [ ] No unbounded storage growth vectors (no per-address balance counters)
- [ ] `burn` removes both `Token(id)` and `ClipIdMinted(clip_id)` — no orphaned entries

---

## 7. Events

| Event topic | Struct | Emitted by |
|-------------|--------|------------|
| `"mint"` | `MintEvent` | `mint` |
| `"burn"` | `BurnEvent` | `burn` |
| `"transfer"` | `TransferEvent` | `transfer`, `transfer_from` |
| `"paused"` | `()` | `pause` |
| `"unpaused"` | `()` | `unpause` |
| `"blacklist"` | `BlacklistEvent` | `blacklist_clip` |
| `"approve"` | `ApprovalEvent` | `approve` |
| `"appr_all"` | `ApprovalForAllEvent` | `set_approval_for_all` |
| `"royalty"` | `RoyaltyPaidEvent` | `pay_royalty` |
| `"royalty"` | `RoyaltyRecipientUpdatedEvent` | `set_royalty` |
| `"upgrade"` | `UpgradeEvent` | `upgrade` |

- [ ] All events verified to emit correct fields
- [ ] No sensitive data (private keys, secrets) emitted in events

---

## 8. Upgradeability

- [ ] `upgrade` uses `env.deployer().update_current_contract_wasm` — preserves storage
- [ ] Only admin can trigger upgrade
- [ ] `UpgradeEvent` emitted with new WASM hash for off-chain tracking

---

## 9. Integer Safety

- [ ] `total_supply` uses `saturating_sub` — no underflow on empty contract
- [ ] `normalize_royalty` uses `saturating_add` for basis point accumulation
- [ ] `calculate_royalty` uses `saturating_mul` after overflow pre-check
- [ ] No unchecked arithmetic in hot paths

---

## 10. Known Limitations / Out of Scope

- `total_supply` counts minted tokens from `NextTokenId - 1` and does **not** decrease on burn (by design — no separate counter).
- XLM royalty payments must be handled off-chain by the marketplace; `pay_royalty` only supports SEP-0041 assets.
- No on-chain enumeration of tokens per owner (no `Balance` index by design — reduces storage cost).
- `upgrade` does not enforce a timelock or multisig — admin key security is critical.
