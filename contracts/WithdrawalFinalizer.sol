// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.0;

import {UncheckedMath} from "@matterlabs/zksync-contracts/contracts/l1-contracts/common/libraries/UncheckedMath.sol";
import {FinalizeL1DepositParams, IL1Nullifier} from "@matterlabs/zksync-contracts/contracts/l1-contracts/bridge/interfaces/IL1Nullifier.sol";

/// @title Withdrawal Finalizer
/// @author Matter Labs
/// @custom:security-contact security@matterlabs.dev
contract WithdrawalFinalizer {
    using UncheckedMath for uint256;

    IL1Nullifier public immutable L1_NULLIFIER;

    struct RequestFinalizeWithdrawal {
        address sender;
        uint256 l2BlockNumber;
        uint256 l2MessageIndex;
        uint16 l2TxNumberInBlock;
        bytes message;
        bytes32[] merkleProof;
        bool isEth;
        uint256 gas;
    }

    struct Result {
        uint256 l2BlockNumber;
        uint256 l2MessageIndex;
        uint256 gas;
        bool success;
    }

    constructor(address _l1Nullifier) {
        L1_NULLIFIER = IL1Nullifier(_l1Nullifier);
    }

    function finalizeWithdrawals(
        uint256 _chainId,
        RequestFinalizeWithdrawal[] calldata _requests
    ) external returns (Result[] memory) {
        uint256 requestsLength = _requests.length;
        Result[] memory results = new Result[](requestsLength);
        for (uint256 i = 0; i < requestsLength; i = i.uncheckedInc()) {
            require(gasleft() >= ((_requests[i].gas * 64) / 63) + 500, "i");
            uint256 gasBefore = gasleft();
            try
                L1_NULLIFIER.finalizeDeposit{gas: _requests[i].gas}(
                    FinalizeL1DepositParams({
                        chainId: _chainId,
                        l2BatchNumber: _requests[i].l2BlockNumber,
                        l2MessageIndex: _requests[i].l2MessageIndex,
                        l2Sender: _requests[i].sender,
                        l2TxNumberInBatch: _requests[i].l2TxNumberInBlock,
                        message: _requests[i].message,
                        merkleProof: _requests[i].merkleProof
                    })
                )
            {
                results[i] = Result({
                    l2BlockNumber: _requests[i].l2BlockNumber,
                    l2MessageIndex: _requests[i].l2MessageIndex,
                    gas: gasBefore - gasleft(),
                    success: true
                });
            } catch {
                results[i] = Result({
                    l2BlockNumber: _requests[i].l2BlockNumber,
                    l2MessageIndex: _requests[i].l2MessageIndex,
                    gas: 0,
                    success: false
                });
            }
        }
        return results;
    }
}
