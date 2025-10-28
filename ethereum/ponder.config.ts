import { createConfig, mergeAbis } from "ponder";
import { BridgeImplAbi } from "./abis/BridgeImplAbi";
import { BridgeProxyAbi } from "./abis/BridgeProxyAbi";
import { Hex } from "viem";

export default createConfig({
  chains: {
    mainnet: {
      id: 1,
      rpc: process.env.PONDER_RPC_URL_1,
    },
  },
  database: {
    kind: "postgres",
    connectionString: `${process.env.DATABASE_URL}`,
  },
  contracts: {
    AvailBridgeV1: {
      chain: "mainnet",
      abi: mergeAbis([BridgeImplAbi, BridgeProxyAbi]),
      address:
        (process.env.BRIDGE_PROXY_ETH as Hex) ||
        "0x054fd961708D8E2B9c10a63F6157c74458889F0a",
      startBlock: 23000000,
    },
  },
});
