import { decodeFunctionData, decodeAbiParameters } from "viem";
import { ponder } from "ponder:registry";
import schema from "ponder:schema";
import { BridgeImplAbi } from "../abis/BridgeImplAbi";
import { DecodedResult } from "./types";

ponder.on("AvailBridgeV1:MessageReceived", async ({ event, context }) => {
  const tx = await context.client.getTransaction({
    hash: event.transaction.hash,
  });

  // The function args structure:
  // [
  //   {
  //     "0x02",
  //     from,
  //     to,
  //     originDomain,
  //     destinationDomain,
  //     data: encodedAssetStruct, // <-- This is what we want
  //     messageId
  //   },
  //   [ ...merkleProofs ]
  // ]

  //these are the idle finance execTransaction events, not sure if we need to index
  if (tx.input.startsWith("0x6a761202")) {
    console.log("Skipping unknown function signature 0x6a761202");
    return;
  }

  //TODO:do better type handling, even if ts is a bitch
  const decoded = decodeFunctionData({
    abi: BridgeImplAbi,
    data: tx.input,
  }) as unknown as DecodedResult<`0x${string}`>;

  const assetStructEncoded = decoded.args[0]!.data;
  const [assetId, amount] = decodeAbiParameters(
    [
      { name: "assetId", type: "bytes32" },
      { name: "amount", type: "uint256" },
    ],
    assetStructEncoded,
  );

  await context.db.insert(schema.recieveAvailEvent).values({
    id: event.id,
    sender: event.args.from,
    receiver: event.args.to,
    messageId: event.args.messageId,
    amount: amount,
    assetId: assetId,
  });
});

ponder.on("AvailBridgeV1:MessageSent", async ({ event, context }) => {
  await context.db.insert(schema.sentAvailEvent).values({
    id: event.id,
    sender: event.args.from,
    receiver: event.args.to,
    messageId: event.args.messageId,
  });
});
