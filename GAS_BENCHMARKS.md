# Gas Benchmarks — ClipCash NFT

> **Living document.** Update this file after every optimization pass.
> Last updated: 2026-04-27 | Contract version: 1

## How to read this document

Soroban charges fees in **stroops** (1 XLM = 10,000,000 stroops) based on:

- **CPU instructions** — metered by the host
- **Memory** — bytes allocated during execution
- **Ledger I/O** — per-entry reads/writes to instance or persistent storage
- **Events** — bytes published to the event stream

The tables below list the storage operations per function (the primary cost driver),
followed by representative fee estimates from testnet and mainnet observations.

> **Testnet vs Mainnet:** Fee schedules are identical in structure but mainnet
> base fees are subject to surge pricing under load. Testnet fees are stable and
> suitable for benchmarking. All stroop figures below are testnet baselines.

---

## Storage tiers

| Tier | Loaded | Cost model |
|------|--------|------------|
| `instance` | Once per transaction | Cheap; shared across all calls in the tx |
| `persistent` | Per-entry | Per-read + per-write fee; survives ledger expiry |

---

## Function benchmarks

### State-mutating functions

#### `init`

| Op | Tier | Count |
|----|------|-------|
| instance write | instance | 4 | (Admin, NextTokenId, Paused, PlatformRecipient)

- **Estimated fee:** ~50,000 stroops
- **Notes:** One-time call; cost is irrelevant to runtime throughput.

---

#### `mint`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 4 | (Admin, NextTokenId, Paused, Signer)
| instance write | instance | 1 | (NextTokenId++)
| persistent read | persistent | 2 | (ClipIdMinted dedup, BlacklistedClip check)
| persistent write | persistent | 2 | (Token, ClipIdMinted)
| crypto | — | 3 | (sha256 × 3 + ed25519_verify × 1)
| event | — | 1 | (MintEvent)

- **Persistent writes: 2** (optimized from 4 — Metadata and Royalty packed into Token)
- **Estimated fee (testnet):** ~500,000 – 700,000 stroops
- **Estimated fee (mainnet):** ~500,000 – 1,000,000 stroops (surge variable)
- **Notes:** Ed25519 verification is the dominant CPU cost. Packing TokenData
  (owner + clip_id + metadata_uri + royalty + is_soulbound) into a single
  persistent entry eliminated 2 writes vs the previous design.

---

#### `transfer`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Paused)
| persistent read | persistent | 2 | (Token, ApprovalForAll or Approved)
| persistent write | persistent | 1 | (Token — new owner)
| persistent remove | persistent | 1 | (Approved — cleared on transfer)
| event | — | 1 | (TransferEvent)

- **Persistent writes: 1**
- **Estimated fee (testnet):** ~200,000 – 300,000 stroops
- **Estimated fee (mainnet):** ~200,000 – 500,000 stroops

---

#### `transfer_from`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Paused)
| persistent read | persistent | 3 | (Token, ApprovalForAll, Approved)
| persistent write | persistent | 1 | (Token — new owner)
| persistent remove | persistent | 1 | (Approved — cleared)
| event | — | 1 | (TransferEvent)

- **Estimated fee (testnet):** ~250,000 – 350,000 stroops
- **Notes:** One extra persistent read vs `transfer` for the spender approval check.

---

#### `burn`

| Op | Tier | Count |
|----|------|-------|
| persistent read | persistent | 1 | (Token — owner check + clip_id)
| persistent remove | persistent | 2 | (Token, ClipIdMinted)
| event | — | 1 | (BurnEvent)

- **Persistent removes: 2** (optimized from 4 — no separate Metadata/Royalty entries)
- **Estimated fee (testnet):** ~150,000 – 250,000 stroops
- **Estimated fee (mainnet):** ~150,000 – 400,000 stroops

---

#### `approve`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Paused)
| persistent read | persistent | 2 | (Token — owner check, ApprovalForAll)
| persistent write | persistent | 1 | (Approved) or persistent remove × 1 (if clearing)
| event | — | 1 | (ApprovalEvent)

- **Estimated fee (testnet):** ~150,000 – 200,000 stroops

---

#### `set_approval_for_all`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Paused)
| persistent write | persistent | 1 | (ApprovalForAll)
| event | — | 1 | (ApprovalForAllEvent)

- **Estimated fee (testnet):** ~100,000 – 150,000 stroops

---

#### `pay_royalty`

| Op | Tier | Count |
|----|------|-------|
| persistent read | persistent | 1 | (Token — royalty config)
| cross-contract call | — | N | (TokenClient.transfer × recipients)
| event | — | N | (RoyaltyPaidEvent × recipients)

- **Estimated fee (testnet):** ~300,000 – 500,000 stroops per recipient
- **Notes:** Cost scales linearly with the number of royalty recipients.
  Cross-contract calls to the SEP-0041 token contract dominate. XLM royalties
  must be handled off-chain by the marketplace.

---

#### `set_royalty`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| persistent read | persistent | 1 | (Token)
| persistent write | persistent | 1 | (Token — updated royalty)
| event | — | 0–1 | (RoyaltyRecipientUpdated, only if recipient changed)

- **Estimated fee (testnet):** ~150,000 – 200,000 stroops

---

#### `blacklist_clip`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| persistent write | persistent | 1 | (BlacklistedClip)
| event | — | 1 | (BlacklistEvent)

- **Estimated fee (testnet):** ~100,000 – 150,000 stroops

---

#### `set_signer`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| instance write | instance | 1 | (Signer)

- **Estimated fee (testnet):** ~50,000 – 80,000 stroops

---

#### `pause` / `unpause`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| instance write | instance | 1 | (Paused)
| event | — | 1 |

- **Estimated fee (testnet):** ~50,000 – 80,000 stroops each

---

#### `upgrade`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| WASM replace | — | 1 | (update_current_contract_wasm)
| event | — | 1 | (UpgradeEvent)

- **Estimated fee (testnet):** ~200,000 – 400,000 stroops
- **Notes:** Fee depends on the size of the new WASM blob.

---

#### `set_name` / `set_symbol`

| Op | Tier | Count |
|----|------|-------|
| instance read | instance | 1 | (Admin check)
| instance write | instance | 1 | (Name or Symbol)

- **Estimated fee (testnet):** ~50,000 – 80,000 stroops each

---

#### `set_token_uri`

| Op | Tier | Count |
|----|------|-------|
| persistent read | persistent | 1 | (Token — owner check)
| persistent write | persistent | 1 | (Token — updated metadata_uri)

- **Estimated fee (testnet):** ~100,000 – 150,000 stroops

---

### Read-only functions

Read-only calls are free when invoked off-chain (simulation). On-chain invocations
(e.g., from another contract) incur a small CPU + I/O fee.

| Function | Storage reads | Estimated fee (on-chain) |
|----------|--------------|--------------------------|
| `owner_of` | 1 persistent | ~30,000 stroops |
| `token_uri` / `get_metadata` | 1 persistent | ~30,000 stroops |
| `get_clip_id` | 1 persistent | ~30,000 stroops |
| `get_royalty` | 1 persistent | ~30,000 stroops |
| `clip_token_id` | 1 persistent | ~30,000 stroops |
| `royalty_info` | 1 persistent | ~40,000 stroops |
| `exists` | 1 persistent (has) | ~20,000 stroops |
| `is_soulbound` | 1 persistent | ~30,000 stroops |
| `get_approved` | 1 persistent | ~20,000 stroops |
| `is_approved_for_all` | 1 persistent | ~20,000 stroops |
| `total_supply` | 1 instance | ~10,000 stroops |
| `is_paused` | 1 instance | ~10,000 stroops |
| `get_signer` | 1 instance | ~10,000 stroops |
| `name` / `symbol` | 1 instance | ~10,000 stroops |
| `version` | 0 | ~5,000 stroops |

---

## Testnet vs Mainnet differences

| Factor | Testnet | Mainnet |
|--------|---------|---------|
| Base fee schedule | Same as mainnet | Same as testnet |
| Surge pricing | None | Yes — fees spike under high load |
| Fee stability | Stable; ideal for benchmarking | Variable |
| Ledger entry rent | Charged but not enforced strictly | Enforced; entries expire if not extended |
| Recommended use | Benchmarking, CI gas assertions | Production cost planning |

**Practical guidance:**
- Use testnet figures as a floor. Budget 2× for mainnet during peak periods.
- Persistent entries require periodic rent extension (`extend_ttl`). Factor in
  extension costs for long-lived tokens (~10,000 stroops per extension per entry).

---

## Optimization history

| Version | Change | Impact |
|---------|--------|--------|
| v1 | Packed `metadata_uri` + `royalty` + `is_soulbound` into `TokenData` | −2 persistent writes on `mint`, −2 persistent removes on `burn` |
| v1 | Removed `Balance(Address)` counter | −1 persistent write on `mint`, −1 on `transfer`, −1 on `burn` |
| v1 | Removed `TokenCount`; derived `total_supply` from `NextTokenId - 1` | −1 instance write on `mint` |
| v1 | Removed `TokenClipId(TokenId)`; packed `clip_id` into `TokenData` | −1 persistent write on `mint` |

---

## How to update this document

1. Run `make test` and note any new gas-related output.
2. Deploy to testnet and capture fee receipts from `stellar tx` output.
3. Update the relevant function table and the **Optimization history** section.
4. Bump the **Last updated** date at the top of this file.
