import { onchainTable } from "ponder";

export const recieveAvailEvent = onchainTable("avail_to_eth", (t) => ({
  id: t.text().primaryKey(),
  sender: t.hex().notNull(),
  receiver: t.hex().notNull(),
  amount: t.bigint(),
  timestamp: t.timestamp(),
  messageId: t.bigint().notNull(),
}));

export const sentAvailEvent = onchainTable("eth_to_avail", (t) => ({
  id: t.text().primaryKey(),
  sender: t.hex().notNull(),
  receiver: t.hex().notNull(),
  amount: t.bigint(),
  timestamp: t.timestamp(),
  messageId: t.bigint().notNull(),
}));
