# zksync-withdrawal-finalizer

A Withdrawal Finalizer in Rust.

## Purpose

Withdrawal Finalizer is a component of `zksync-era` responsible for monitoring and finalizing [L2->L1 withdrawals](https://github.com/matter-labs/zksync-era/blob/a98e454221da7d6ecad9b317cf44b0786e819659/docs/advanced/03_withdrawals.md). It does so by continiously monitoring events happening on both L2 and L1, keeping some state in persisting storage (which is PostgreSQL) and sending withdrawal finalization transactions whenever necessary.

## Building

Building the project is straightforward:

```
cargo build
```

## Deploying

To deploy this service you will need the following prerequisites:

1. Websocket RPC endpoint on Ethereum.
2. Websocket RPC endpoint on zkSync Era.
3. An instance of PostgreSQL database.

### Running DB migrations

Prior to deployment of the service the database migrations have to be run with [`sqlx-cli`](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli) component of [`sqlx`](https://github.com/launchbadge/sqlx):

```
$ cd ./storage
$ env DATABASE_URL=postgres://mycreds@myhost/mydb sqlx database create
$ env DATABASE_URL=postgres://mycreds@myhost/mydb sqlx migrate run
```
### Configuration

Configuration is done via enviromnent variables that can also be read from `.env` file if it is present. 
Deployment is done by deploying a dockerized image of the service.

| Variable | Description |
| -------- | ----------- |
| `ETH_CLIENT_WS_URL` | The address of Ethereum WebSocket RPC endpoint |
| `CONTRACTS_L1_ERC20_BRIDGE_PROXY_ADDR` | Address of the L1 ERC20 bridge contract** |
| `CONTRACTS_L2_ERC20_BRIDGE_ADDR` | Address of the L2 ERC20 bridge contract** |
| `CONTRACTS_DIAMOND_PROXY_ADDR` | Address of the L1 diamond proxy contract** |
| `CONTRACTS_WITHDRAWAL_FINALIZER_CONTRACT` | Address of the Withdrawal Finalizer contract ** |
| `API_WEB3_JSON_RPC_WS_URL` | Address of the zkSync Era WebSocket RPC endpoint |
| `DATABSE_URL` | The url of PostgreSQL database the service stores its state into |
| `GAS_LIMIT` | The gas limit of a single withdrawal finalization within the batch of withdrawals finalized in a call to `finalizeWithdrawals` in WithdrawalFinalizerContract |
| `BATCH_FINALIZATION_GAS_LIMIT` | The gas limit of the finalizastion of the whole batch in a call to `finalizeWithdrawals` in Withdrawal Finalizer Contract |
| `WITHDRAWAL_FINALIZER_ACCOUNT_PRIVATE_KEY` | The private key of the account that is going to be submit finalization transactions |
| `TX_RETRY_TIMEOUT_SECS` | Number of seconds to wait for a potentially stuck finalization transaction before readjusting its fees |

The configuration structure describing the service config can be found in [`config.rs`](https://github.com/matter-labs/zksync-withdrawal-finalizer/blob/main/bin/withdrawal-finalizer/src/config.rs)

** more about zkSync contracts can be found [here](https://github.com/matter-labs/era-contracts/blob/main/docs/Overview.md)



## License

zkSync Withdrawal Finalizer is distributed under the terms of either

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
