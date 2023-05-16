#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Etherscan API related functionality.
use ethers::{
    abi::{AbiDecode, Address},
    prelude::account::{Sort, TxListParams},
    types::{Chain, U256},
};

use crate::Result;

/// Get the last processed L1 batch from Etherscan
///
/// This function queries the transactions that were sent
/// from the withdrawer account to determine what was the
/// last processed l1 batch. In the past withdrawal transactions
/// were sent to the bridge contracts instead of the standalone
/// contract, we would also want to query those transactions and so
/// the addresses of the bridges are among function parameters.
///
/// # Arguments
///
/// * `chain`: Which chain to query
/// * `withdrawer_account`: Address of the withdrawer wallet
/// * `withdrawal_contract`: Address of finalizer contract
/// * `l1_eth_bridge`: Address of L1 eth bridge contract
/// * `l1_erc20_bridge`: Address of L1 ERC20 bridge contract
/// * `api_key`: Etherscan API key
pub async fn last_processed_l1_batch(
    chain: Chain,
    withdrawer_accout: Address,
    withdrawal_contract: Address,
    l1_eth_bridge: Address,
    l1_erc20_bridge: Address,
    api_key: impl Into<String>,
) -> Result<Option<U256>> {
    let client = ethers::etherscan::Client::new(chain, api_key)?;

    let params = TxListParams {
        start_block: 0,
        end_block: 99999999,
        page: 1,
        offset: 100,
        sort: Sort::Desc,
    };

    let transactions = client
        .get_transactions(&withdrawer_accout, Some(params))
        .await?;

    for tx in &transactions {
        if let Ok(wf_tx) = crate::withdrawal_finalizer::FinalizeWithdrawalsCall::decode(&tx.input) {
            if tx.to == Some(withdrawal_contract) {
                if let Some(request) = wf_tx.requests.get(0) {
                    return Ok(Some(request.l_2_block_number));
                }
            }
        }

        if let Ok(legacy_tx) = crate::zksync_contract::FinalizeEthWithdrawalCall::decode(&tx.input)
        {
            if tx.to == Some(l1_eth_bridge) || tx.to == Some(l1_erc20_bridge) {
                return Ok(Some(legacy_tx.l_2_block_number));
            }
        }
    }

    Ok(None)
}
