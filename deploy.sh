#!/bin/bash

# Exit on error
set -e

# Default values for network and account
NETWORK=${1:-${NETWORK:-"testnet"}}
ACCOUNT=${SOROBAN_ACCOUNT:-"default"}

echo "Deploying to $NETWORK using account $ACCOUNT..."

# Build the contract
echo "Building contract..."
cargo build --target wasm32-unknown-unknown --release -p clips_nft

WASM_PATH="target/wasm32-unknown-unknown/release/clips_nft.wasm"

# Ensure the WASM file exists
if [ ! -f "$WASM_PATH" ]; then
    echo "Error: WASM file not found at $WASM_PATH"
    exit 1
fi

# Deploy the contract WASM
echo "Installing WASM on-chain..."
WASM_HASH=$(soroban contract install --network "$NETWORK" --source "$ACCOUNT" --wasm "$WASM_PATH")
echo "WASM installed with hash: $WASM_HASH"

# Deploy the contract instance
echo "Deploying contract instance..."
CONTRACT_ID=$(soroban contract deploy --network "$NETWORK" --source "$ACCOUNT" --wasm-hash "$WASM_HASH")
echo "----------------------------------------"
echo "CONTRACT_ID: $CONTRACT_ID"
echo "----------------------------------------"

# Save contract ID to a file for later use
mkdir -p .soroban
echo "$CONTRACT_ID" > ".soroban/contract-id-$NETWORK"

# Initialization (optional, depends if it's already initialized)
# Here we initialize with the same account as admin
ADMIN_ADDRESS=$(soroban config identity address "$ACCOUNT")
echo "Initializing contract with admin: $ADMIN_ADDRESS..."
soroban contract invoke --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" -- init --admin "$ADMIN_ADDRESS"

# Verification step using bindings
echo "Generating TypeScript bindings for verification..."
mkdir -p ./bindings
soroban contract bindings generate --id "$CONTRACT_ID" --network "$NETWORK" --output-dir ./bindings/clips_nft --overwrite

# Final verification: Call total_supply
echo "Verifying deployment by calling total_supply..."
TOTAL_SUPPLY=$(soroban contract invoke --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" -- total_supply)
echo "Total supply: $TOTAL_SUPPLY"

if [ "$TOTAL_SUPPLY" == "0" ]; then
    echo "Deployment verified successfully!"
else
    echo "Verification failed: Unexpected total supply ($TOTAL_SUPPLY)"
    exit 1
fi
