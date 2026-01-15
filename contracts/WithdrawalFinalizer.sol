// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.0;

import {UncheckedMath} from "@matterlabs/zksync-contracts/contracts/l1-contracts/common/libraries/UncheckedMath.sol";
import {IL1AssetRouter} from "@matterlabs/zksync-contracts/contracts/l1-contracts/bridge/asset-router/IL1AssetRouter.sol";

/// @title Withdrawal Finalizer
/// @author Matter Labs
/// @custom:security-contact security@matterlabs.dev
contract WithdrawalFinalizer {
    using UncheckedMath for uint256;

    IL1AssetRouter public immutable L1_ASSET_ROUTER;

    struct RequestFinalizeWithdrawal {
        uint256 _l2BlockNumber;
        uint256 _l2MessageIndex;
        uint16 _l2TxNumberInBlock;
        bytes _message;
        bytes32[] _merkleProof;
        bool _isEth;
        uint256 _gas;
    }

    struct Result {
        uint256 _l2BlockNumber;
        uint256 _l2MessageIndex;
        uint256 _gas;
        bool success;
    }

    constructor(address l1AssetRouter) {
        L1_ASSET_ROUTER = IL1AssetRouter(l1_asset_router);
    }

    function finalizeWithdrawals(
        uint256 chainId,
        RequestFinalizeWithdrawal[] calldata requests
    ) external returns (Result[] memory) {
        uint256 requestsLength = requests.length;
        Result[] memory results = new Result[](requestsLength);
        for (uint256 i = 0; i < requestsLength; i = i.uncheckedInc()) {
            require(gasleft() >= ((requests[i]._gas * 64) / 63) + 500, "i");
            uint256 gasBefore = gasleft();
            try
                L1_ASSET_ROUTER.finalizeWithdrawal{gas: requests[i]._gas}({
                    _chainId: chainId,
                    _l2BatchNumber: requests[i]._l2BlockNumber,
                    _l2MessageIndex: requests[i]._l2MessageIndex,
                    _l2TxNumberInBatch: requests[i]._l2TxNumberInBlock,
                    _message: requests[i]._message,
                    _merkleProof: requests[i]._merkleProof
                })
            {
                results[i] = Result({
                    _l2BlockNumber: requests[i]._l2BlockNumber,
                    _l2MessageIndex: requests[i]._l2MessageIndex,
                    _gas: gasBefore - gasleft(),
                    success: true
                });
            } catch {
                results[i] = Result({
                    _l2BlockNumber: requests[i]._l2BlockNumber,
                    _l2MessageIndex: requests[i]._l2MessageIndex,
                    _gas: 0,
                    success: false
                });
            }
        }
        return results;
    }
}
