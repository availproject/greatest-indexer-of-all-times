mod conversion;
mod error;
mod schema;
mod sync;
mod types;

use avail_rust::{
	BlockRawExtrinsic, BlockRef, BlockSignedExtrinsic, BlockWithExt, Client, HasHeader, MAINNET_ENDPOINT,
	avail::vector::{
		tx::{Execute, FailedSendMessageTxs, SendMessage},
		types::Message::FungibleToken,
	},
	avail_rust_core::types::TxRef,
	block::BlockExtOptionsExpanded,
	subscription::RawExtrinsicSub,
};
use error::Error;
use schema::{BasicTableOperations, SendMessageEntry};
use sqlx::{Connection, PgConnection};
use tokio::runtime::Runtime;
use tracing::error as lerr;
use tracing_subscriber::util::SubscriberInitExt;

// This is the main thread.
// It will init our Runtime and run a block task
fn main() {
	// Enable logs
	let builder = tracing_subscriber::fmt::SubscriberBuilder::default();
	builder.finish().init();

	// Create runtime
	let runtime = match Runtime::new() {
		Ok(r) => r,
		Err(err) => {
			lerr!("Failed to create runtime. Existing program. Reason: {}", err);
			return;
		},
	};
	let result = runtime.block_on(main_task());
	if let Err(err) = result {
		lerr!("Execution stopped. Existing program. Reason: {}", err);
	}
}

async fn test_task() -> Result<(), Error> {
	let url = "t";
	let mut conn = PgConnection::connect(url).await?;

	// schema::create_table(&mut conn).await;
	// schema::list_table_names(&mut conn).await;
	// schema::list_cars_table(&mut conn).await;

	//let e = schema::CarsEntry::new("Rimac", "Nevera", 2025);
	//e.table_insert_entry(&mut conn).await;
	//schema::CarsEntry::table_create(&mut conn).await;
	schema::SendMessageEntry::table_create(&mut conn).await.unwrap();

	println!("PG worked");
	Ok(())
}

async fn main_task() -> Result<(), Error> {
	// Create a db connection.
	let url = "t";
	let mut conn = PgConnection::connect(url).await?;
	reset_db(&mut conn).await?;

	// Create a connection.
	// TODO Make endpoint CLI or ENV
	let client = Client::new(MAINNET_ENDPOINT).await?;

	// Sync testing
	//sync::SyncManager::run(client.clone()).await;

	// Here we define what extrinsics we will follow
	// SendMessage and Execute are signed
	// We don't need to track FailedSendMessageTxs
	let tracked_calls: Vec<(u8, u8)> = vec![
		SendMessage::HEADER_INDEX,
		Execute::HEADER_INDEX,
		//FailedSendMessageTxs::HEADER_INDEX,
	];

	// Here we define from what block heigh to start indexing.
	// For testing purposes let's use a fixed height
	// TODO Make endpoint CLI or ENV
	let block_height = 1905204;

	// Create a subscription
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };
	let mut sub = RawExtrinsicSub::new(client.clone(), opts);
	sub.set_block_height(block_height);

	// Run subscription
	// For testing we will fetch the next 10 instances
	for _ in 0..10 {
		let Ok((list, block_info)) = sub.next().await else {
			// TODO What to do if we cannot reach our endpoint after X retires? Probably sleep for X seconds and try again.
			// Let's just panic for now.
			panic!("Failed to fetch next extrinsics.")
		};

		// If the sub returns no elements then something is wrong with the subscription
		assert!(!list.is_empty());

		// Find all SendMessage, Execute and FailedSendMessageTxs extrinsics.
		// There must be just one FailedSendMessageTxs extrinsic.
		let send_message_exts: Vec<BlockRawExtrinsic> = list
			.iter()
			.filter(|x| (x.metadata.pallet_id, x.metadata.variant_id) == SendMessage::HEADER_INDEX)
			.cloned()
			.collect();
		let execute_exts: Vec<BlockRawExtrinsic> = list
			.iter()
			.filter(|x| (x.metadata.pallet_id, x.metadata.variant_id) == Execute::HEADER_INDEX)
			.cloned()
			.collect();

		println!(
			"Block height: {}. Send Message Count: {} Execute Txs Count: {}",
			block_info.height,
			send_message_exts.len(),
			execute_exts.len()
		);

		if !send_message_exts.is_empty() {
			let block = BlockWithExt::new(client.clone(), block_info.height);
			let Ok(failed_ext) = block.first::<FailedSendMessageTxs>(Default::default()).await else {
				// TODO better logging
				panic!("Failed to fetch failed send message Txs");
			};

			// This should never happen.
			let Some(failed_ext) = failed_ext else {
				// TODO better logging
				panic!("No failed send message tx was found in a block.");
			};

			// Now handle them
			handle_send_message_ext(send_message_exts, failed_ext.call.failed_txs, block_info, &mut conn).await?;
		}

		if !execute_exts.is_empty() {
			handle_execute_ext(execute_exts);
		}
	}

	Ok(())
}

async fn reset_db(conn: &mut PgConnection) -> Result<(), Error> {
	//reset db
	if SendMessageEntry::table_exists(conn).await? {
		SendMessageEntry::table_drop(conn).await?;
		println!("Removed old AvailSendMessage table <3")
	}
	SendMessageEntry::table_create(conn).await?;
	println!("Created new AvailSendMessage table <3");

	Ok(())
}

async fn handle_send_message_ext(
	list: Vec<BlockRawExtrinsic>,
	failed_list: Vec<u32>,
	block_ref: BlockRef,
	conn: &mut PgConnection,
) -> Result<(), Error> {
	// TODO don't include TXs that failed
	assert_eq!(failed_list.len(), 0);

	// For testing reason let's just print them for now.
	let list: Result<Vec<BlockSignedExtrinsic<SendMessage>>, _> = list
		.into_iter()
		.map(BlockSignedExtrinsic::<SendMessage>::try_from)
		.collect();
	let Ok(list) = list else {
		// TODO proper error handling
		panic!("Failed to convert one Send Message from Raw to Ext");
	};
	for ext in list {
		println!(
			"✉️  Send Message: Message: {:?}, To: {:?}, Domain: {}",
			ext.call.message, ext.call.to, ext.call.domain
		);

		let (asset_id, amount) = match ext.call.message {
			FungibleToken { asset_id, amount } => (asset_id, amount),
			_ => continue,
		};

		let tx_ref: TxRef = (ext.metadata.ext_hash, ext.metadata.ext_index).into();
		let from = match ext.signature.address {
			avail_rust::MultiAddress::Id(x) => x,
			_ => panic!("Ohh, account is not of type ID. TODO"),
		};

		let entry = SendMessageEntry::new(block_ref, tx_ref, asset_id, amount, ext.call.to, from);
		entry.table_insert_entry(conn).await?;
		println!("✉️  Send Message: Added to table <3",);
		SendMessageEntry::table_list_entries(conn).await?;
	}
	Ok(())
}

fn handle_execute_ext(list: Vec<BlockRawExtrinsic>) {
	// For testing reason let's just print them for now.
	let list: Result<Vec<BlockSignedExtrinsic<Execute>>, _> = list
		.into_iter()
		.map(BlockSignedExtrinsic::<Execute>::try_from)
		.collect();
	let Ok(list) = list else {
		// TODO proper error handling
		panic!("Failed to convert one Execute from Raw to Ext");
	};
	for ext in list {
		println!("☠️  Execute: From: {:?}, To: {:?}", ext.call.addr_message.from, ext.call.addr_message.to,)
	}
}
