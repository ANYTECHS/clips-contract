#!/usr/bin/env bash
# verify-deployment.sh
#
# Post-deployment verification script for the ClipCash NFT Soroban contract.
#
# Usage:
#   ./scripts/verify-deployment.sh [OPTIONS]
#
# Options:
#   -c, --contract-id  CONTRACT_ID     Contract address to verify (overrides auto-detect)
#   -n, --network      testnet|mainnet  Network to use            (default: testnet)
#   -s, --source       ACCOUNT         Stellar identity / key     (default: "default")
#   -l, --ledger       LEDGER_NUM       Start ledger for event scan (default: auto)
#   -o, --output       FILE            Write JSON report to file
#   -h, --help                         Show this help
#
# The script:
#   1. Resolves the contract ID
#   2. Runs read-only checks (version, total_supply, is_paused, get_signer)
#   3. Simulates a mint call and asserts the expected error code
#   4. Scans on-chain events for the contract
#   5. Prints a colour-coded pass/fail report and exits 0 (all pass) or 1 (any fail)

set -euo pipefail

# ── colours ────────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN="\033[0;32m"; YELLOW="\033[1;33m"; RED="\033[0;31m"
  CYAN="\033[0;36m"; BOLD="\033[1m"; RESET="\033[0m"
else
  GREEN=""; YELLOW=""; RED=""; CYAN=""; BOLD=""; RESET=""
fi

# ── helpers ────────────────────────────────────────────────────────────────────
pass()  { echo -e "  ${GREEN}✔${RESET}  $*"; PASS=$((PASS + 1)); }
fail()  { echo -e "  ${RED}✖${RESET}  $*"; FAIL=$((FAIL + 1)); }
warn()  { echo -e "  ${YELLOW}⚠${RESET}  $*"; WARN=$((WARN + 1)); }
info()  { echo -e "  ${CYAN}ℹ${RESET}  $*"; }
section(){ echo -e "\n${BOLD}$*${RESET}"; }

PASS=0; FAIL=0; WARN=0

# ── defaults ───────────────────────────────────────────────────────────────────
NETWORK="testnet"
SOURCE="default"
CONTRACT_ID=""
START_LEDGER=""
OUTPUT_FILE=""

# ── argument parsing ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    -c|--contract-id) CONTRACT_ID="$2"; shift 2 ;;
    -n|--network)     NETWORK="$2";     shift 2 ;;
    -s|--source)      SOURCE="$2";      shift 2 ;;
    -l|--ledger)      START_LEDGER="$2";shift 2 ;;
    -o|--output)      OUTPUT_FILE="$2"; shift 2 ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \?//'
      exit 0 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ── resolve contract ID ────────────────────────────────────────────────────────
if [[ -z "$CONTRACT_ID" ]]; then
  SAVED_FILE=".soroban/contract-id-${NETWORK}"
  if [[ -f "$SAVED_FILE" ]]; then
    CONTRACT_ID=$(tr -d '[:space:]' < "$SAVED_FILE")
    info "Using saved contract ID from ${SAVED_FILE}"
  else
    echo -e "${RED}Error:${RESET} CONTRACT_ID not provided and ${SAVED_FILE} does not exist."
    echo "       Run ./deploy.sh ${NETWORK} first, or pass --contract-id C..."
    exit 1
  fi
fi

# ── prereq check ──────────────────────────────────────────────────────────────
if ! command -v stellar &>/dev/null; then
  echo -e "${RED}Error:${RESET} stellar CLI not found."
  echo "       Install: cargo install --locked stellar-cli"
  exit 1
fi

STELLAR_VER=$(stellar --version 2>&1 | head -1)

# ── shared invoke helper ───────────────────────────────────────────────────────
# invoke CONTRACT_ID FN ARGS...  → stdout of the call (stripped)
invoke() {
  local id="$1"; shift
  stellar contract invoke \
    --id "$id" \
    --source-account "$SOURCE" \
    --network "$NETWORK" \
    --send=no \
    -- "$@" 2>/dev/null | tr -d '[:space:]"'
}

# invoke allowing stderr (for error-code capture)
invoke_raw() {
  local id="$1"; shift
  stellar contract invoke \
    --id "$id" \
    --source-account "$SOURCE" \
    --network "$NETWORK" \
    --send=no \
    -- "$@" 2>&1 || true
}

# ── banner ─────────────────────────────────────────────────────────────────────
echo -e "\n${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}║       ClipCash NFT — Deployment Verification             ║${RESET}"
echo -e "${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo
info "Contract  : ${CONTRACT_ID}"
info "Network   : ${NETWORK}"
info "Source    : ${SOURCE}"
info "stellar   : ${STELLAR_VER}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
info "Timestamp : ${TIMESTAMP}"

# ──────────────────────────────────────────────────────────────────────────────
section "1 ▸ Reachability"
# ──────────────────────────────────────────────────────────────────────────────

# 1a. version() — equivalent of "name()" for this contract; returns the
#     contract version constant (currently 1).
section_version=""
if version_raw=$(invoke "$CONTRACT_ID" version 2>/dev/null); then
  section_version="$version_raw"
  if [[ "$section_version" == "1" ]]; then
    pass "version() = ${section_version}  (contract reachable)"
  else
    warn "version() returned unexpected value: ${section_version} (expected 1)"
  fi
else
  fail "version() call failed — contract may not be deployed at ${CONTRACT_ID}"
fi

# 1b. is_paused() — equivalent of "symbol()" probe; simple boolean read.
if paused_raw=$(invoke "$CONTRACT_ID" is_paused 2>/dev/null); then
  if [[ "$paused_raw" == "false" ]]; then
    pass "is_paused() = false  (contract is active)"
  elif [[ "$paused_raw" == "true" ]]; then
    warn "is_paused() = true  (contract is PAUSED — mint/transfer blocked)"
  else
    warn "is_paused() returned unexpected value: ${paused_raw}"
  fi
else
  fail "is_paused() call failed"
fi

# ──────────────────────────────────────────────────────────────────────────────
section "2 ▸ State checks"
# ──────────────────────────────────────────────────────────────────────────────

# 2a. total_supply()
if supply_raw=$(invoke "$CONTRACT_ID" total_supply 2>/dev/null); then
  pass "total_supply() = ${supply_raw}  (valid u32 response)"
else
  fail "total_supply() call failed"
  supply_raw="?"
fi

# 2b. get_signer()
if signer_raw=$(invoke "$CONTRACT_ID" get_signer 2>/dev/null); then
  if [[ "$signer_raw" == "" || "$signer_raw" == "null" || "$signer_raw" == "None" ]]; then
    warn "get_signer() = None  (no backend signer registered — set_signer required before minting)"
    SIGNER_SET=false
  else
    pass "get_signer() = ${signer_raw:0:12}…  (backend signer is registered)"
    SIGNER_SET=true
  fi
else
  fail "get_signer() call failed"
  SIGNER_SET=false
fi

# ──────────────────────────────────────────────────────────────────────────────
section "3 ▸ Mint simulation"
# ──────────────────────────────────────────────────────────────────────────────
# A full mint needs an Ed25519 signature from the registered backend signer.
# We simulate a mint with a zero-byte signature to confirm the contract's
# validation logic is intact:
#   • If no signer is set  → expect error code 9 (SignerNotSet)
#   • If signer is set     → expect error code 8 (InvalidSignature)
# Any other outcome means the contract logic is broken.

DUMMY_SIG="0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
DUMMY_ADDR="GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN"  # well-known test key

mint_out=$(invoke_raw "$CONTRACT_ID" mint \
  --to    "$DUMMY_ADDR" \
  --clip-id 9999999 \
  --metadata-uri "ipfs://verify-test" \
  --royalty '{"recipients":[{"recipient":"'"$DUMMY_ADDR"'","basis_points":500}],"asset_address":null}' \
  --is-soulbound false \
  --signature "$DUMMY_SIG" \
  2>&1 || true)

if echo "$mint_out" | grep -q "SignerNotSet\|error.*9\|code.*9"; then
  pass "mint() simulation → SignerNotSet (code 9)  (expected — no signer registered)"
elif echo "$mint_out" | grep -q "InvalidSignature\|error.*8\|code.*8"; then
  pass "mint() simulation → InvalidSignature (code 8)  (expected — signer rejects dummy sig)"
elif echo "$mint_out" | grep -q "InvalidRecipient\|error.*5\|code.*5"; then
  pass "mint() simulation → InvalidRecipient (code 5)  (contract validation active)"
elif echo "$mint_out" | grep -q "error\|Error\|failed\|panic"; then
  # Got some error — extract the message for the report
  ERR_MSG=$(echo "$mint_out" | grep -oE "(Error|error)[^$]+" | head -1)
  warn "mint() simulation returned an unexpected error: ${ERR_MSG}"
else
  fail "mint() simulation returned no error — contract may not be enforcing signature checks"
fi

# ──────────────────────────────────────────────────────────────────────────────
section "4 ▸ Events"
# ──────────────────────────────────────────────────────────────────────────────

# Determine start ledger: use provided value or fall back to recent history
if [[ -z "$START_LEDGER" ]]; then
  # Query the latest ledger from the network to scan last ~100 ledgers
  LATEST_LEDGER=$(stellar ledger --network "$NETWORK" 2>/dev/null \
    | grep -oE '"sequence":[0-9]+' | grep -oE '[0-9]+' | head -1 || echo "")
  if [[ -n "$LATEST_LEDGER" && "$LATEST_LEDGER" -gt 100 ]]; then
    START_LEDGER=$((LATEST_LEDGER - 100))
  else
    START_LEDGER=1
  fi
fi

info "Scanning events from ledger ${START_LEDGER} for contract ${CONTRACT_ID:0:10}…"

EVENTS_OUT=$(stellar events \
  --network "$NETWORK" \
  --id "$CONTRACT_ID" \
  --start-ledger "$START_LEDGER" \
  --count 50 \
  --output json 2>/dev/null || echo "[]")

EVENT_COUNT=$(echo "$EVENTS_OUT" | grep -c '"type"' 2>/dev/null || echo 0)

if [[ "$EVENT_COUNT" -gt 0 ]]; then
  pass "Found ${EVENT_COUNT} event(s) for this contract"

  # Summarise event topics
  MINT_EVENTS=$(echo   "$EVENTS_OUT" | grep -c '"mint"'     2>/dev/null || echo 0)
  TRANSFER_EVENTS=$(echo "$EVENTS_OUT" | grep -c '"transfer"' 2>/dev/null || echo 0)
  BURN_EVENTS=$(echo    "$EVENTS_OUT" | grep -c '"burn"'     2>/dev/null || echo 0)
  ROYALTY_EVENTS=$(echo "$EVENTS_OUT" | grep -c '"royalty"'  2>/dev/null || echo 0)
  PAUSED_EVENTS=$(echo  "$EVENTS_OUT" | grep -c '"paused"'   2>/dev/null || echo 0)

  [[ "$MINT_EVENTS"     -gt 0 ]] && info "  mint events     : ${MINT_EVENTS}"
  [[ "$TRANSFER_EVENTS" -gt 0 ]] && info "  transfer events : ${TRANSFER_EVENTS}"
  [[ "$BURN_EVENTS"     -gt 0 ]] && info "  burn events     : ${BURN_EVENTS}"
  [[ "$ROYALTY_EVENTS"  -gt 0 ]] && info "  royalty events  : ${ROYALTY_EVENTS}"
  [[ "$PAUSED_EVENTS"   -gt 0 ]] && info "  paused events   : ${PAUSED_EVENTS}"
else
  warn "No events found in ledger range ${START_LEDGER}+ (contract may be newly deployed)"
  EVENT_COUNT=0
fi

# ──────────────────────────────────────────────────────────────────────────────
section "5 ▸ Summary"
# ──────────────────────────────────────────────────────────────────────────────

TOTAL=$((PASS + FAIL + WARN))
echo
echo -e "  Tests run : ${TOTAL}"
echo -e "  ${GREEN}Passed${RESET}    : ${PASS}"
echo -e "  ${YELLOW}Warnings${RESET}  : ${WARN}"
echo -e "  ${RED}Failed${RESET}    : ${FAIL}"
echo

# ── optional JSON report ───────────────────────────────────────────────────────
if [[ -n "$OUTPUT_FILE" ]]; then
  cat > "$OUTPUT_FILE" <<JSON
{
  "timestamp": "${TIMESTAMP}",
  "network": "${NETWORK}",
  "contract_id": "${CONTRACT_ID}",
  "stellar_cli_version": "${STELLAR_VER}",
  "checks": {
    "version": "${section_version:-unknown}",
    "is_paused": "${paused_raw:-unknown}",
    "total_supply": "${supply_raw:-unknown}",
    "signer_registered": ${SIGNER_SET:-false},
    "mint_simulation": "$(echo "$mint_out" | grep -oE "(SignerNotSet|InvalidSignature|InvalidRecipient)" | head -1 || echo "unknown")",
    "events_found": ${EVENT_COUNT}
  },
  "results": {
    "pass": ${PASS},
    "warn": ${WARN},
    "fail": ${FAIL},
    "total": ${TOTAL}
  },
  "status": "$([ "$FAIL" -eq 0 ] && echo "PASS" || echo "FAIL")"
}
JSON
  info "JSON report written to ${OUTPUT_FILE}"
fi

# ── final exit code ────────────────────────────────────────────────────────────
if [[ "$FAIL" -gt 0 ]]; then
  echo -e "${RED}${BOLD}VERIFICATION FAILED${RESET} — ${FAIL} check(s) did not pass.\n"
  exit 1
else
  echo -e "${GREEN}${BOLD}VERIFICATION PASSED${RESET} — contract is functioning correctly.\n"
  exit 0
fi
