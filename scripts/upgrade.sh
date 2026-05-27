#!/bin/bash
set -euo pipefail

NETWORK=${1:-${NETWORK:-"testnet"}}
ACCOUNT=${2:-${SOROBAN_ACCOUNT:-"default"}}
WASM_PATH=${3:-"target/wasm32-unknown-unknown/release/clips_nft.wasm"}
CONTRACT_ID_FILE=".soroban/contract-id-${NETWORK}"

if [ ! -f "$CONTRACT_ID_FILE" ]; then
  echo "Error: contract id file not found: $CONTRACT_ID_FILE"
  echo "Run deploy.sh first or pass a contract id manually."
  exit 1
fi

CONTRACT_ID=$(tr -d '[:space:]' < "$CONTRACT_ID_FILE")

if [ ! -f "$WASM_PATH" ]; then
  echo "WASM not found at $WASM_PATH"
  echo "Building release WASM..."
  cargo build --target wasm32-unknown-unknown --release -p clips_nft
fi

if [ ! -f "$WASM_PATH" ]; then
  echo "Error: WASM file still missing after build: $WASM_PATH"
  exit 1
fi

if ! command -v soroban >/dev/null 2>&1; then
  echo "Error: soroban CLI not found. Install it before upgrading."
  exit 1
fi

echo "Installing new contract WASM to network '$NETWORK'..."
WASM_HASH=$(soroban contract install --network "$NETWORK" --source "$ACCOUNT" --wasm "$WASM_PATH")

echo "New WASM installed with hash: $WASM_HASH"

ADMIN_ADDRESS=$(soroban config identity address "$ACCOUNT")
echo "Using admin address: $ADMIN_ADDRESS"

echo "Upgrading contract instance $CONTRACT_ID to new WASM hash..."
soroban contract invoke --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" -- upgrade --admin "$ADMIN_ADDRESS" --new-wasm-hash "$WASM_HASH"

echo "Upgrade transaction submitted."

echo "Verifying upgraded contract version..."
VERSION=$(soroban contract invoke --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" -- version)

echo "Contract $CONTRACT_ID upgraded successfully. version=$VERSION"

echo "If rollback is required, install the previous WASM hash and call upgrade again with the prior hash."
