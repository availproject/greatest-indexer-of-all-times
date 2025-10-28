import {
  decodeFunctionData,
  decodeAbiParameters,
  Hex,
  toHex,
  keccak256,
  concat,
  GetProofReturnType,
} from "viem";
import { ponder } from "ponder:registry";
import schema from "ponder:schema";
import { BridgeImplAbi } from "../abis/BridgeImplAbi";
import { DecodedResult } from "./types";
import { replaceBigInts } from "ponder";

ponder.on("AvailBridgeV1:MessageReceived", async ({ event, context }) => {
  const tx = await context.client.getTransaction({
    hash: event.transaction.hash,
  });

  //these are the idle finance execTransaction events, which break our flow.
  if (tx.input.startsWith("0x6a761202")) {
    console.log("Skipping unknown function signature 0x6a761202");
    return;
  }

  const decoded = decodeFunctionData({
    abi: BridgeImplAbi,
    data: tx.input,
  }) as unknown as DecodedResult<Hex>;

  const assetStructEncoded = decoded.args[0]!.data;
  const [assetId, amount] = decodeAbiParameters(
    [
      { name: "assetId", type: "bytes32" },
      { name: "amount", type: "uint256" },
    ],
    assetStructEncoded,
  );

  await context.db.insert(schema.bridgeEvent).values({
    id: event.id,
    sender: event.args.from,
    receiver: event.args.to,
    messageId: event.args.messageId,
    amount: amount,
    eventType: "MessageReceived",
  });
});

ponder.on("AvailBridgeV1:MessageSent", async ({ event, context }) => {
  const tx = await context.client.getTransaction({
    hash: event.transaction.hash,
  });

  const messageId = event.args.messageId;
  const storageSlot = 1n;

  const messageIdBytes = toHex(messageId, { size: 32 });
  const storageSlotBytes = toHex(storageSlot, { size: 32 });
  const storageKey = keccak256(concat([messageIdBytes, storageSlotBytes]));

  const proof = await context.client.getProof({
    address: context.contracts.AvailBridgeV1.address,
    storageKeys: [storageKey],
    blockNumber: event.block.number,
  });

  const decoded = decodeFunctionData({
    abi: BridgeImplAbi,
    data: tx.input,
  });

  //no choice but to typecast since jsonb does not support bigint natively
  const bigIntAdjustedProof = replaceBigInts(proof, (v) =>
    String(v),
  ) as unknown as GetProofReturnType;

  await context.db.insert(schema.bridgeEvent).values({
    id: event.id,
    sender: event.args.from,
    receiver: event.args.to,
    messageId: event.args.messageId,
    amount: decoded.args[1] as bigint,
    eventType: "MessageSent",
    proof: bigIntAdjustedProof,
  });
});
