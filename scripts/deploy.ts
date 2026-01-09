import { Wallet, utils } from 'ethers';

const hardhat = require('hardhat');

async function main() {
    // Read CLI arg: prefer --router=0x..., fall back to first positional
    const routerFlag = process.argv.find(a => a.startsWith('--router='));
    const l1AssetRouter = routerFlag ? routerFlag.split('=')[1] : process.argv[2];

    if (!l1AssetRouter || !utils.isAddress(l1AssetRouter)) {
      throw new Error(
        'Provide the L1 asset router address via --router=0x... or as the first positional arg'
      );
    }

    const provider = new hardhat.ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL as string);
    const wallet  = Wallet.fromMnemonic(
          process.env.MNEMONIC as string,
          "m/44'/60'/0'/0/1"
      ).connect(provider);


      const contractFactory = await hardhat.ethers.getContractFactory("WithdrawalFinalizer", {
          signer: wallet
      });
      const contract = await contractFactory.deploy(l1AssetRouter);
      await contract.deployTransaction.wait();    
      console.log(`CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS=${contract.address}`);
  }
  
  main()
    .then(() => process.exit(0))
    .catch((error) => {
      console.error(error);
      process.exit(1);
    });
