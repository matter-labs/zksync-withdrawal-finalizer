#!/usr/bin/env bash

echo -e "\n[>] Deploy contract"
cast rpc anvil_setBalance 0x1A24e5C53B1438f15B25c819fEe1F894e6D131f2 0x3635C9ADC5DEA000000
MNEMONIC="test test test test test test test test test test test junk" ETH_CLIENT_WEB3_URL="http://localhost:8545" npx hardhat deploy --nullifier 0xD7f9f54194C633F36CCD5F3da84ad4a1c38cB2cB

echo -e "\n[>] Starting DB"
docker stop pg-finalizer2
docker start pg-finalizer

echo -e "\n[>] Running WF"
RUST_BACKTRACE=1 RUST_LOG=info cargo run --bin withdrawal-finalizer

docker stop pg-finalizer
