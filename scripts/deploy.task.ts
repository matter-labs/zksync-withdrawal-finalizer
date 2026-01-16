import { task } from "hardhat/config";
import type { HardhatRuntimeEnvironment } from "hardhat/types";
import { HDNodeWallet, isAddress, JsonRpcProvider } from "ethers";

export async function deployWithdrawalFinalizer(
  hre: HardhatRuntimeEnvironment,
  l1Nullifier: string
) {
  if (!isAddress(l1Nullifier)) {
    throw new Error("Invalid --nullifier address");
  }

  // Resolve RPC (prefer explicit env; else take Hardhat network URL)
  const rpcUrl =
    process.env.ETH_CLIENT_WEB3_URL ||
    (typeof hre.network.config.url === "string" ? hre.network.config.url : "");
  if (!rpcUrl) throw new Error("No RPC URL. Set ETH_CLIENT_WEB3_URL or pass --network.");

  const provider = new JsonRpcProvider(rpcUrl);
  console.log("RPC:", rpcUrl);

  // Signer from mnemonic
  const mnemonic = process.env.MNEMONIC;
  if (!mnemonic) throw new Error("Set MNEMONIC to a funded key on your fork.");
  const wallet = HDNodeWallet.fromPhrase(mnemonic, "m/44'/60'/0'/0/1").connect(provider);
  console.log("Signer:", await wallet.getAddress());

  // Deploy
  const Factory = await hre.ethers.getContractFactory("WithdrawalFinalizer", wallet);
  const contract = await Factory.deploy(l1Nullifier);
  await contract.waitForDeployment();
  
  const addr = await contract.getAddress();
  console.log(`CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS=${addr}`);
  return addr;
}

/** Hardhat task: npx hardhat deploy --nullifier 0x... [--network anvil_era] */
task("deploy", "Deploys WithdrawalFinalizer")
  .addParam("nullifier", "L1 nullifier address (0x...)")
  .setAction(async (args, hre) => {
    await deployWithdrawalFinalizer(hre, String(args.nullifier));
  });

/**
 * Direct runner for `hardhat run scripts/deploy.task.ts`
 * You can use env NULLIFIER when running directly.
 */
async function main() {
  const hre = (await import("hardhat")).default;
  const l1Nullifier =
    process.env.NULLIFIER ||
    ""; // keep simple for direct runs; prefer the task for CLI param parsing
  if (!l1Nullifier) throw new Error("Set NULLIFIER env or use the Hardhat task.");
  await deployWithdrawalFinalizer(hre, l1Nullifier);
}

// Only execute `main()` if invoked directly via `node`/`hardhat run`
// (Hardhat just imports task files; this prevents auto-exec on import)
if (require.main === module) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}