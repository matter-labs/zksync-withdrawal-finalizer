import { Wallet, utils } from 'ethers';

const hardhat = require('hardhat');

async function main() {
    // Read CLI arg: prefer --nullifier=0x..., fall back to first positional
    const nullifierFlag = process.argv.find(a => a.startsWith('--nullifier='));
    const l1Nullifier = nullifierFlag ? nullifierFlag.split('=')[1] : process.argv[2];

    if (!l1Nullifier || !utils.isAddress(l1Nullifier)) {
      throw new Error(
        'Provide the L1 Nullifier address via --nullifier=0x... or as the first positional arg'
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
      const contract = await contractFactory.deploy(l1Nullifier);
      await contract.deployTransaction.wait();    
      console.log(`CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS=${contract.address}`);
  }
  
  main()
    .then(() => process.exit(0))
    .catch((error) => {
      console.error(error);
      process.exit(1);
    });
