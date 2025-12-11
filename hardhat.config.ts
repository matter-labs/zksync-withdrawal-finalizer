import '@nomiclabs/hardhat-solpp';
import '@nomiclabs/hardhat-ethers';
import '@nomiclabs/hardhat-etherscan';
import '@typechain/hardhat';

const config = {
    BRIDGE_HUB: process.env.CONTRACTS_BRIDGE_HUB_ADDR
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
