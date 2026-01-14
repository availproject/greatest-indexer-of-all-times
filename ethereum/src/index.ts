import {
    decodeFunctionData,
    decodeAbiParameters,
    Hex,
    toHex,
    keccak256,
    concat,
    GetProofReturnType,
} from "viem";
import {ponder} from "ponder:registry";
import schema from "ponder:schema";
import {BridgeImplAbi} from "../abis/BridgeImplAbi";
import {DecodedResult, STATUS} from "./types";
import {replaceBigInts} from "ponder";
import {ethers} from "ethers";
import { BigNumber } from "bignumber.js";

function parseAmount(numberString: string): string {
    try {
        const number = BigNumber(numberString).toFixed();
        return ethers.formatEther(number).toString();
    } catch (e) {
        return "";
    }
}


ponder.on("AvailBridgeV1:MessageReceived", async ({event, context}) => {
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
            {name: "assetId", type: "bytes32"},
            {name: "amount", type: "uint256"},
        ],
        assetStructEncoded,
    );

    console.log({
        type: "MessageReceived",
        "amount": parseAmount(amount.toString()),
        "from": event.args.from,
        "to": event.args.to
    });

    await context.db.insert(schema.bridgeEvent).values({
        sender: event.args.from,
        receiver: event.args.to,
        messageId: event.args.messageId,
        //for compatibility with the rust indexer, we need to store it as text (numeric(78) represenation of bigint vs native bigint)
        amount: amount.toString(),
        eventType: "MessageReceived",
        status: STATUS.BRIDGED,
        blockNumber: Number(event.block.number),
        sourceBlockHash: event.block.hash,
        sourceTransactionHash: event.transaction.hash,
    });
});

ponder.on("AvailBridgeV1:MessageSent", async ({event, context}) => {
    const tx = await context.client.getTransaction({
        hash: event.transaction.hash,
    });

    const messageId = event.args.messageId;
    const storageSlot = 1n;

    const messageIdBytes = toHex(messageId, {size: 32});
    const storageSlotBytes = toHex(storageSlot, {size: 32});
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

    let amount = decoded.args[1] as string;
    console.log({
        type: "MessageSent",
        "amount": parseAmount(amount),
        "from": event.args.from,
        "to": event.args.to
    });

    await context.db
        .insert(schema.bridgeEvent)
        .values({
            messageId: event.args.messageId,
            sender: event.args.from,
            receiver: event.args.to,
            amount: amount,
            eventType: "MessageSent",
            proof: bigIntAdjustedProof,
            status: STATUS.IN_PROGRESS,
            blockNumber: Number(event.block.number),
            sourceBlockHash: event.block.hash,
            sourceTransactionHash: event.transaction.hash,
        })
        .onConflictDoUpdate((existing) => ({
            sender: event.args.from,
            receiver: event.args.to,
            amount: decoded.args[1] as string,
            eventType: "MessageSent",
            proof: bigIntAdjustedProof,
            status: STATUS.IN_PROGRESS,
            blockNumber: Number(event.block.number),
            sourceBlockHash: event.block.hash,
            sourceTransactionHash: event.transaction.hash,
        }));
});
