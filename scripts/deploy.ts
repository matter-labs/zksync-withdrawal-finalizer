import { Wallet } from 'ethers';

const hardhat = require('hardhat');

async function main() {
    const provider = new hardhat.ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL as string);
    const wallet  = Wallet.fromMnemonic(
          process.env.MNEMONIC as string,
          "m/44'/60'/0'/0/1"
      ).connect(provider);


      const contractFactory = await hardhat.ethers.getContractFactory("WithdrawalFinalizer", {
          signer: wallet
      });
      const contract = await contractFactory.deploy();
      await contract.deployTransaction.wait();    
      console.log(`CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS=${contract.address}`);
  }
  
  main()
    .then(() => process.exit(0))
    .catch((error) => {
      console.error(error);
      process.exit(1);
    });
