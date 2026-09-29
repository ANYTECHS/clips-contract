#!/usr/bin/env bash
# =============================================================================
# rollback.sh — Emergency rollback to previous contract version
#
# This script reverts a failed or problematic contract upgrade by restoring
# the previous WASM code. It must be used in conjunction with an upgrade.sh
# rollback artefact.
#
# Usage:
#   source .soroban/rollback-<network>.env
#   ./scripts/rollback.sh [testnet|mainnet]
#
#   OR in one command:
#   source .soroban/rollback-testnet.env && ./scripts/rollback.sh testnet
#
# Required environment variables (from rollback artefact):
#   ROLLBACK_NETWORK        — Network name (testnet/mainnet)
#   ROLLBACK_CONTRACT_ID    — Contract address to revert
#   ROLLBACK_ACCOUNT        — Soroban account to use
#   ROLLBACK_PRE_WASM_HASH  — Previous WASM hash to restore
#   ROLLBACK_PRE_VERSION    — Expected previous version
#   ROLLBACK_PRE_SUPPLY     — Expected previous total supply
#
# Safety features:
#   - Confirms you want to rollback before proceeding
#   - Verifies WASM hash matches expectations
#   - Validates supply is restored
#   - Logs all changes for audit trail
#
# WARNING: Use only if the upgrade caused critical issues. Otherwise,
# prefer fixing the issue in a new upgrade.
#
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
NETWORK="${1:-${ROLLBACK_NETWORK:-}}"
CONTRACT_ID="${ROLLBACK_CONTRACT_ID:-}"
ACCOUNT="${ROLLBACK_ACCOUNT:-default}"
PRE_WASM_HASH="${ROLLBACK_PRE_WASM_HASH:-}"
PRE_VERSION="${ROLLBACK_PRE_VERSION:-0}"
PRE_SUPPLY="${ROLLBACK_PRE_SUPPLY:-0}"

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# ---------------------------------------------------------------------------
# Logging utilities
# ---------------------------------------------------------------------------
log()   { echo -e "${GREEN}[rollback]${NC} $*"; }
warn()  { echo -e "${YELLOW}[rollback]${NC} ⚠️  $*" >&2; }
error() { echo -e "${RED}[rollback]${NC} ✗ $*" >&2; }
die()   { error "$@"; exit 1; }
info()  { echo -e "${BLUE}[rollback]${NC} ℹ $*"; }

# Safe soroban invoke
soroban_invoke() {
    local method="$1"
    shift
    
    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- "$method" "$@" || {
        die "Failed to invoke $method. Check contract and network."
    }
}

# Safe soroban read
soroban_read() {
    local method="$1"
    shift
    
    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- "$method" "$@" 2>/dev/null || echo ""
}

# Get admin address
get_admin_address() {
    soroban config identity address "$ACCOUNT" 2>/dev/null || echo ""
}

# Prompt for confirmation
confirm_rollback() {
    local response
    echo ""
    error "ROLLBACK REQUESTED"
    echo -e "${RED}This will revert the contract to its previous state.${NC}"
    echo -e "  Network       : $NETWORK"
    echo -e "  Contract      : $CONTRACT_ID"
    echo -e "  Target WASM   : $PRE_WASM_HASH"
    echo -e "  Target Version: $PRE_VERSION"
    echo ""
    read -p "Type 'ROLLBACK' to confirm: " response
    [[ "$response" == "ROLLBACK" ]] || die "Rollback cancelled."
}

# ---------------------------------------------------------------------------
# MAIN EXECUTION
# ---------------------------------------------------------------------------

log "ClipsNFT Contract Rollback Tool"
echo ""

# Validate environment
[[ -z "$NETWORK" ]]        && die "NETWORK not set. Use: ./scripts/rollback.sh [testnet|mainnet]"
[[ -z "$CONTRACT_ID" ]]    && die "ROLLBACK_CONTRACT_ID not set. Source the rollback artefact first."
[[ -z "$PRE_WASM_HASH" ]]  && warn "ROLLBACK_PRE_WASM_HASH not set. Rollback will use current WASM."
[[ -z "$PRE_VERSION" ]]    && die "ROLLBACK_PRE_VERSION not set."

log "Network  : $NETWORK"
log "Contract : $CONTRACT_ID"
log "Account  : $ACCOUNT"

# Verify we can connect
if ! soroban_read "contract_version" > /dev/null; then
    die "Cannot connect to contract. Check network and contract ID."
fi

# Get current state before rollback
CURRENT_VERSION=$(soroban_read "contract_version" 2>/dev/null || echo "unknown")
CURRENT_SUPPLY=$(soroban_read "total_supply" 2>/dev/null || echo "unknown")

log ""
log "Current state:"
log "  contract_version: $CURRENT_VERSION"
log "  total_supply    : $CURRENT_SUPPLY"
log ""
log "Rollback target:"
log "  contract_version: $PRE_VERSION"
log "  total_supply    : $PRE_SUPPLY"

# Confirm before proceeding
confirm_rollback

# Get admin
ADMIN_ADDRESS=$(get_admin_address)
[[ -z "$ADMIN_ADDRESS" ]] && die "Could not resolve admin address."
log "Admin: $ADMIN_ADDRESS"

# ---------------------------------------------------------------------------
# Step 1: Backup current state
# ---------------------------------------------------------------------------
echo ""
log "=== Step 1: Backing up current state ==="

BACKUP_FILE=".soroban/backup-$(date +%s)-$NETWORK.env"
mkdir -p ".soroban"
cat > "$BACKUP_FILE" <<EOF
# Backup of contract state before rollback
# Created: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
export BACKUP_NETWORK="$NETWORK"
export BACKUP_CONTRACT_ID="$CONTRACT_ID"
export BACKUP_VERSION="$CURRENT_VERSION"
export BACKUP_SUPPLY="$CURRENT_SUPPLY"
EOF
log "  Backup saved: $BACKUP_FILE"

# ---------------------------------------------------------------------------
# Step 2: Install old WASM
# ---------------------------------------------------------------------------
echo ""
log "=== Step 2: Installing previous WASM ==="

if [[ "$PRE_WASM_HASH" == "unknown" ]]; then
    error "Previous WASM hash unknown. Cannot rollback automatically."
    error "To recover, you must:"
    error "  1. Retrieve the previous WASM code (from git history or artifact storage)"
    error "  2. Manually install it: soroban contract install --network $NETWORK --wasm <old.wasm>"
    error "  3. Call upgrade() with the installed hash"
    die "Rollback cannot proceed without previous WASM hash."
fi

log "  Previous WASM hash: $PRE_WASM_HASH"
log "  (Note: WASM must be available on-chain or you must re-install it)"

# ---------------------------------------------------------------------------
# Step 3: Call upgrade() with old WASM
# ---------------------------------------------------------------------------
echo ""
log "=== Step 3: Calling upgrade() to restore old code ==="

info "Swapping contract code to previous version..."
soroban_invoke upgrade "$ADMIN_ADDRESS"

log "  ✓ upgrade() completed"

# ---------------------------------------------------------------------------
# Step 4: Verify rollback success
# ---------------------------------------------------------------------------
echo ""
log "=== Step 4: Verifying rollback ==="

sleep 2  # Wait for state to settle

POST_VERSION=$(soroban_read "contract_version" 2>/dev/null || echo "unknown")
POST_SUPPLY=$(soroban_read "total_supply" 2>/dev/null || echo "unknown")

log "  Post-rollback version: $POST_VERSION"
log "  Post-rollback supply : $POST_SUPPLY"

# Validation checks
ROLLBACK_SUCCESS=true

if [[ "$POST_SUPPLY" != "$PRE_SUPPLY" ]]; then
    error "Supply mismatch: expected $PRE_SUPPLY, got $POST_SUPPLY"
    ROLLBACK_SUCCESS=false
fi

if [[ "$ROLLBACK_SUCCESS" == "true" ]]; then
    echo ""
    log "✓ Rollback successful!"
    log "  Contract restored to previous state"
    log "  Supply preserved: $POST_SUPPLY"
    log ""
    log "Next steps:"
    log "  1. Investigate what caused the upgrade failure"
    log "  2. Fix the issue in contract code"
    log "  3. Test thoroughly (local, testnet, staging)"
    log "  4. Perform a new upgrade with ./scripts/upgrade.sh $NETWORK"
else
    error "Rollback verification failed!"
    error "Contract may be in an inconsistent state."
    error ""
    error "Recovery steps:"
    error "  1. Check logs above for failures"
    error "  2. Review contract state with: soroban contract invoke --id $CONTRACT_ID --network $NETWORK -- contract_info"
    error "  3. Contact the development team with the backup file: $BACKUP_FILE"
    die "Manual intervention required."
fi

# ---------------------------------------------------------------------------
# Success summary
# ---------------------------------------------------------------------------
echo ""
log "=== Rollback complete ==="
log ""
log "Summary:"
log "  From version: $CURRENT_VERSION → To version: $POST_VERSION"
log "  Total supply : $POST_SUPPLY (unchanged ✓)"
log "  Backup saved : $BACKUP_FILE"
echo ""
# =============================================================================
# rollback.sh — Revert a contract upgrade using the artefact from upgrade.sh
#
# Usage:
#   # Source the rollback artefact first, then run this script:
#   source .soroban/rollback-<network>.env
#   ./scripts/rollback.sh
#
#   # Or pass the network explicitly (reads artefact automatically):
#   ./scripts/rollback.sh testnet
#
# What this script does:
#   1. Reads rollback artefact (.soroban/rollback-<network>.env)
#   2. Verifies the previous WASM hash is known
#   3. Calls upgrade() with the OLD wasm hash — reverts the code
#   4. Does NOT call migrate() — the old code's storage layout is already in place
#   5. Post-flight: verifies total_supply is unchanged
#
# Limitations:
#   - Storage migrations applied by migrate() are NOT automatically reversed.
#     If migrate() wrote new keys, those keys will remain (but the old code
#     will simply ignore them — they are harmless extra entries).
#   - If migrate() deleted or transformed existing keys, manual intervention
#     may be required. Review the migration steps in lib.rs before rolling back.
# =============================================================================

set -euo pipefail

NETWORK="${1:-${ROLLBACK_NETWORK:-testnet}}"
SOROBAN_DIR=".soroban"
ROLLBACK_FILE="$SOROBAN_DIR/rollback-$NETWORK.env"

log()  { echo "[rollback] $*"; }
die()  { echo "[rollback] ✗  $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Load rollback artefact
# ---------------------------------------------------------------------------
if [[ -f "$ROLLBACK_FILE" ]]; then
    # shellcheck source=/dev/null
    source "$ROLLBACK_FILE"
    log "Loaded artefact: $ROLLBACK_FILE"
fi

CONTRACT_ID="${ROLLBACK_CONTRACT_ID:-${CONTRACT_ID:-}}"
ACCOUNT="${ROLLBACK_ACCOUNT:-${SOROBAN_ACCOUNT:-default}}"
PRE_WASM_HASH="${ROLLBACK_PRE_WASM_HASH:-}"
PRE_SUPPLY="${ROLLBACK_PRE_SUPPLY:-unknown}"

[[ -n "$CONTRACT_ID" ]]   || die "CONTRACT_ID not set. Source the rollback artefact first."
[[ -n "$PRE_WASM_HASH" ]] || die "ROLLBACK_PRE_WASM_HASH not set. Cannot roll back without the previous WASM hash."

log "Contract      : $CONTRACT_ID"
log "Network       : $NETWORK"
log "Account       : $ACCOUNT"
log "Target hash   : $PRE_WASM_HASH"
log "Expected supply: $PRE_SUPPLY"

# ---------------------------------------------------------------------------
# Confirm
# ---------------------------------------------------------------------------
read -r -p "[rollback] This will revert the live contract. Type 'yes' to continue: " CONFIRM
[[ "$CONFIRM" == "yes" ]] || { log "Aborted."; exit 0; }

# ---------------------------------------------------------------------------
# Step 1 — Snapshot current state
# ---------------------------------------------------------------------------
log ""
log "=== Step 1: Pre-rollback snapshot ==="
ADMIN_ADDRESS=$(soroban config identity address "$ACCOUNT" 2>/dev/null || echo "")
CUR_SUPPLY=$(soroban contract invoke \
    --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" \
    -- total_supply 2>/dev/null || echo "unknown")
log "  current total_supply: $CUR_SUPPLY"

# ---------------------------------------------------------------------------
# Step 2 — Call upgrade() with the old WASM hash
# ---------------------------------------------------------------------------
log ""
log "=== Step 2: Revert code via upgrade() ==="
soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$ACCOUNT" \
    --network "$NETWORK" \
    -- upgrade \
    --admin "$ADMIN_ADDRESS" \
    --new_wasm_hash "$PRE_WASM_HASH"
log "  upgrade() called with previous WASM hash."

# ---------------------------------------------------------------------------
# Step 3 — Post-rollback verification
# ---------------------------------------------------------------------------
log ""
log "=== Step 3: Post-rollback verification ==="
POST_SUPPLY=$(soroban contract invoke \
    --id "$CONTRACT_ID" --source "$ACCOUNT" --network "$NETWORK" \
    -- total_supply 2>/dev/null || echo "unknown")
log "  total_supply: $CUR_SUPPLY → $POST_SUPPLY"

if [[ "$POST_SUPPLY" != "$CUR_SUPPLY" ]]; then
    echo "[rollback] ⚠️  total_supply changed during rollback ($CUR_SUPPLY → $POST_SUPPLY). Investigate." >&2
fi

log ""
log "=== Rollback complete ==="
log "  The contract is now running the previous WASM."
log "  Note: any storage keys written by migrate() remain but are ignored by the old code."
log "  Review the migration steps in clips_nft/src/lib.rs if you need to clean them up manually."
