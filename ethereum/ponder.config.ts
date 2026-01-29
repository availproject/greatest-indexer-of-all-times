import { createConfig, mergeAbis } from "ponder";
import { BridgeImplAbi } from "./abis/BridgeImplAbi";
import { BridgeProxyAbi } from "./abis/BridgeProxyAbi";
import { Hex } from "viem";

export default createConfig({
  chains: {
    sepolia: {
      id: 11155111,
      rpc: process.env.PONDER_RPC_URL_1,
    },
  },
  database: {
    kind: "postgres",
    connectionString: `${process.env.DATABASE_URL}`,
  },
  contracts: {
    AvailBridgeV1: {
      chain: "sepolia",
      abi: mergeAbis([BridgeImplAbi, BridgeProxyAbi]),
      address:
        (process.env.BRIDGE_PROXY_ETH as Hex) ||
        "0x967F7DdC4ec508462231849AE81eeaa68Ad01389",
      startBlock: Number(process.env.BRIDGE_START_BLOCK),
    },
  },
});
