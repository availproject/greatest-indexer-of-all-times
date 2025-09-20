import { createConfig, mergeAbis } from "ponder";
import { BridgeImplAbi } from "./abis/BridgeImplAbi";
import { BridgeProxyAbi } from "./abis/BridgeProxyAbi";

export default createConfig({
  chains: {
    mainnet: {
      id: 1,
      rpc: process.env.PONDER_RPC_URL_1,
    },
  },
  contracts: {
    AvailBridgeV1: {
      chain: "mainnet",
      abi: mergeAbis([BridgeImplAbi, BridgeProxyAbi]),
      address: "0x42CDc5D4B05E8dACc2FCD181cbe0Cc86Ee14c439",
      startBlock: 17942156,
    },
  },
});
