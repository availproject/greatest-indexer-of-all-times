import { onchainTable } from "ponder";
import { GetProofReturnType } from "viem";

export const bridgeEvent = onchainTable("event", (t) => ({
  id: t.text().primaryKey(),
  sender: t.hex().notNull(),
  receiver: t.hex().notNull(),
  amount: t.bigint().notNull(),
  messageId: t.bigint().notNull(),
  eventType: t.text().notNull(), // "MessageSent" or "MessageReceived"
  //proofs only for MessageSent
  proof: t.jsonb().$type<GetProofReturnType>(),
  blockHash: t.hex(),
}));
