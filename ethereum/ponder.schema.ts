import { onchainTable, primaryKey, uniqueIndex } from "ponder";
import { GetProofReturnType } from "viem";
import { STATUS } from "./src/types";

export const bridgeEvent = onchainTable(
  "bridge_event",
  (t) => ({
    messageId: t.bigint().notNull(),
    sender: t.hex().notNull(),
    receiver: t.hex().notNull(),
    amount: t.bigint().notNull(),
    eventType: t.text().notNull(), // "MessageSent" or "MessageReceived"
    //proofs only for MessageSent
    proof: t.jsonb().$type<GetProofReturnType>(),
    blockHash: t.hex(),
    status: t.text().notNull().$type<STATUS>(),
  }),
  (table) => ({
    pk: primaryKey({ columns: [table.messageId, table.eventType] }),
  }),
);
