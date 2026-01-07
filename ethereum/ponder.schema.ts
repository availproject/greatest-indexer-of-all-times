import { onchainEnum, onchainTable, primaryKey } from "ponder";
import { GetProofReturnType } from "viem";
import { STATUS } from "./src/types";

export const status = onchainEnum("status", [
  STATUS.INITIATED,
  STATUS.IN_PROGRESS,
  STATUS.CLAIM_READY,
  STATUS.BRIDGED,
]);

export const bridgeEvent = onchainTable(
  "bridge_event",
  (t) => ({
    messageId: t.int8({ mode: "bigint" }).notNull(),
    sender: t.hex().notNull(),
    receiver: t.hex().notNull(),
    amount: t.text().notNull(),
    eventType: t.text().notNull(), // "MessageSent" or "MessageReceived"
    //proofs only for MessageSent
    proof: t.jsonb().$type<GetProofReturnType>(),
    sourceBlockHash: t.hex().notNull(),
    blockNumber: t.integer().notNull(),
    status: status().notNull(),
    sourceTransactionHash: t.hex().notNull(),
  }),
  (table) => ({
    pk: primaryKey({ columns: [table.messageId, table.eventType] }),
  }),
);
