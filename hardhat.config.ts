import '@nomiclabs/hardhat-solpp';
import '@nomiclabs/hardhat-ethers';
import '@nomiclabs/hardhat-etherscan';
import '@typechain/hardhat';

const config = {
    L1_ASSET_ROUTER: process.env.CONTRACTS_L1_ASSET_ROUTER
};

export default {
    solidity: {
        version: '0.8.18',
        settings: {
            optimizer: {
                enabled: true,
                runs: 200
            },
            outputSelection: {
                '*': {
                    '*': ['storageLayout']
                }
            }
        }
    },
    contractSizer: {
        runOnCompile: false
    },
    paths: {
        sources: './contracts'
    },
    solpp: {
        defs: config
    },
    etherscan: {
        apiKey: process.env.MISC_ETHERSCAN_API_KEY
    }
};
