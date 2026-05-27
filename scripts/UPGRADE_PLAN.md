# Contract Upgrade Plan

This document describes the safe upgrade path for the `clips_nft` Soroban contract.
It preserves all existing NFT state and royalty configuration while switching the contract implementation.

## Upgrade procedure

1. Build the new contract WASM.
   - `cargo build --target wasm32-unknown-unknown --release -p clips_nft`

2. Install the new WASM on-chain.
   - `soroban contract install --network <network> --source <account> --wasm target/wasm32-unknown-unknown/release/clips_nft.wasm`

3. Invoke the contract `upgrade` entrypoint on the existing contract instance.
   - `soroban contract invoke --id <contract-id> --source <account> --network <network> -- upgrade --admin <admin-address> --new-wasm-hash <new-wasm-hash>`

4. Verify the existing storage is intact.
   - Confirm minted token ownership with `owner_of`.
   - Confirm royalty configuration with `get_royalty`.
   - Confirm `total_supply` matches the pre-upgrade contract state.
   - Optionally confirm `version` if the new implementation increments it.

## Why this is safe

- The `upgrade` entrypoint uses Soroban's built-in contract deployer API.
- All instance and persistent storage remains attached to the same contract ID.
- Existing NFTs, clip IDs, and royalty structures are preserved.
- The contract ID does not change, so frontend and marketplace integrations remain valid.

## Rollback plan

1. Keep the previous WASM hash and the old WASM artifact.
2. If the upgrade causes a problem, install the previous WASM again:
   - `soroban contract install --network <network> --source <account> --wasm target/wasm32-unknown-unknown/release/clips_nft.wasm`
3. Call the same `upgrade` entrypoint with the old WASM hash:
   - `soroban contract invoke --id <contract-id> --source <account> --network <network> -- upgrade --admin <admin-address> --new-wasm-hash <old-wasm-hash>`

### Important rollback notes

- The rollback uses the same contract ID and the same storage.
- Do not redeploy a new contract ID unless the current contract is irrecoverably broken.
- Verify admin access before attempting rollback.
- If the contract is paused, use the admin pause/unpause controls to freeze user operations safely during remediation.

## Regression testing

- Add a contract upgrade regression test that mints a token before upgrade and verifies the same token and royalty fields after the upgrade.
- Confirm `total_supply` and `owner_of(token_id)` still return expected values.
- Confirm the contract remains callable after the upgrade.
