import '@nomiclabs/hardhat-solpp';
import '@nomicfoundation/hardhat-ethers';
import '@nomiclabs/hardhat-etherscan';
import '@typechain/hardhat';

const config = {
    L1_ASSET_ROUTER_ADDRESS: process.env.CONTRACTS_L1_ASSET_ROUTER_ADDR
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
    },
    typechain: {
        outDir: 'typechain-types',
        target: 'ethers-v6'
    }
};
