import { ponder } from "ponder:registry";
import schema from "ponder:schema";

ponder.on("AvailBridgeV1:MessageReceived", async ({ event, context }) => {
  await context.db.insert(schema.recieveAvailEvent).values({
    id: event.id,
    sender: event.args.from,
    receiver: event.args.to,
    messageId: event.args.messageId,
  });

  // fetch events, store in db and make sure to console log events with amount more than 100k avail

  console.log("Message received", event);
});
