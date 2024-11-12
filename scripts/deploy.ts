import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import * as fs from "fs";
import * as path from "path";
import * as dotenv from "dotenv";
import config from "../config/archway.config.ts"; // Ensure config is correctly resolved

dotenv.config(); // Load environment variables from .env

// Function to deploy a contract
async function deployContract(
  wasmFilePath: string,
  client: SigningCosmWasmClient,
  senderAddress: string,
  label: string,
  initMsg: any
) {
  try {
    const wasmCode = fs.readFileSync(wasmFilePath);
    console.log(`Uploading contract: ${label}...`);

    // Upload contract code
    const uploadResponse = await client.upload(senderAddress, wasmCode, "auto", `Upload ${label}`);
    const codeId = uploadResponse.codeId;
    console.log(`Contract ${label} uploaded successfully with code ID: ${codeId}`);

    console.log(`Instantiating contract: ${label}...`);

    // Instantiate the contract
    const initResponse = await client.instantiate(
      senderAddress,
      codeId,
      initMsg,
      label,
      "auto",
      { admin: senderAddress } // You may modify admin for contract upgradeability
    );
    console.log(`Contract ${label} instantiated at address: ${initResponse.contractAddress}`);

    return initResponse.contractAddress;
  } catch (error) {
    console.error(`Error during deployment of contract ${label}:`, error);
    throw error; // Rethrow the error to be handled at the higher level
  }
}

// Main function to deploy multiple contracts
async function main() {
  const network = process.env.NETWORK === "mainnet" ? config.mainnet : config.testnet;

  // Step 1: Set up wallet and client
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(process.env.MNEMONIC || "", {
    prefix: network.prefix,
  });

  const client = await SigningCosmWasmClient.connectWithSigner(network.endpoints.rpc, wallet);
  const senderAddress = (await wallet.getAccounts())[0].address;

  console.log(`Deploying contracts from address: ${senderAddress}`);

  // Step 2: Deploy contracts and store addresses
  const deployedContracts: Record<string, string> = {};

  try {
    // Deploy LiquidityWrapper contract
    deployedContracts["liquidity-wrapper"] = await deployContract(
      path.join(__dirname, "../contracts/liquidity-wrapper/artifacts/liquidity_wrapper.wasm"),
      client,
      senderAddress,
      "LiquidityWrapper",
      { /* initial message for LiquidityWrapper contract */ }
    );

    // Deploy Marketplace contract
    deployedContracts["marketplace"] = await deployContract(
      path.join(__dirname, "../contracts/marketplace/artifacts/marketplace.wasm"),
      client,
      senderAddress,
      "Marketplace",
      { /* initial message for Marketplace contract */ }
    );

    // Deploy ULNFTCore contract
    deployedContracts["ul-nft-core"] = await deployContract(
      path.join(__dirname, "../contracts/ul-nft-core/artifacts/ul_nft_core.wasm"),
      client,
      senderAddress,
      "ULNFTCore",
      { /* initial message for ULNFTCore contract */ }
    );

    // Log the deployed contract addresses
    console.log("Deployment complete. Deployed contract addresses:");
    console.log(deployedContracts);

    // Optionally save the addresses to a JSON file for later reference
    fs.writeFileSync("deployed_contracts.json", JSON.stringify(deployedContracts, null, 2));
    console.log("Deployed contract addresses saved to deployed_contracts.json");

  } catch (error) {
    console.error("Error during deployment:", error);
    process.exit(1); // Exit with failure
  }
}

// Execute deployment
main().catch((error) => {
  console.error("Unexpected error during deployment:", error);
  process.exit(1); // Exit with failure
});
