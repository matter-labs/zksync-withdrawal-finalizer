#!/usr/bin/env bash

# Assuming that you are running anvil in another terminal anvil --fork-url https://sepolia.drpc.org

echo -e "\n[>] Setting env vars"
L1_NULLIFIER_SEPOLIA="0xd349295957c1b6Ad5213e1070DBF44D07b7D7D7F"
export MNEMONIC="test test test test test test test test test test test junk"
export ETH_CLIENT_WEB3_URL="http://localhost:8545"

set -a
source run_helpers_test/test_sepolia.env
set +a

echo -e "\n[>] Deploy contract"
cast rpc anvil_setBalance 0x1A24e5C53B1438f15B25c819fEe1F894e6D131f2 0x3635C9ADC5DEA000000
yarn contracts:build
# This line sets the CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS env var
CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS="$(
  npx hardhat deploy --nullifier "$L1_NULLIFIER_SEPOLIA" \
  | awk -F= '/^CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS=/{print $2; exit}'
)"

# Guard + export
if [ -z "$CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS" ]; then
  echo "\n\t[X]Failed to capture CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS from deploy output" >&2
  exit 1
fi
export CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS

echo -e "\n[>] Starting DB"
docker start pg-finalizer

echo -e "\n[>] Running WF"
RUST_BACKTRACE=1 RUST_LOG=info cargo run --bin withdrawal-finalizer

docker stop pg-finalizer
