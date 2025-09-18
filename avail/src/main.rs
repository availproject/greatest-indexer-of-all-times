mod schema;
mod types;

use avail_rust::{
	BlockRawExtrinsic, BlockSignedExtrinsic, BlockWithExt, Client, HasHeader, MAINNET_ENDPOINT,
	avail::vector::tx::{Execute, FailedSendMessageTxs, SendMessage},
	block::BlockExtOptionsExpanded,
	subscription::{RawExtrinsicSub, SubBuilder},
};
use tokio::runtime::Runtime;

// This is the main thread.
// It will init our Runtime and run a block task
fn main() {
	let Ok(runtime) = Runtime::new() else {
		// TODO proper log
		panic!("Failed to ini Runtime");
	};
	let result = runtime.block_on(main_task());
	if let Err(err) = result {
		// TODO proper log
		panic!("Program crashed because: {}", err.to_string());
	}
}

async fn main_task() -> Result<(), avail_rust::Error> {
	// Create a connection.
	// TODO Make endpoint CLI or ENV
	let client = Client::new(MAINNET_ENDPOINT).await?;

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
	let sub = SubBuilder::new()
		.block_height(block_height)
		.follow(false)
		.build(&client)
		.await?;
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };
	let mut sub = RawExtrinsicSub::new(client.clone(), sub, opts);

	// Run subscription
	// For testing we will fetch the next 10 instances
	for _ in 0..10 {
		let Ok((list, block_info)) = sub.next().await else {
			// TODO What to do if we cannot reach our endpoint after X retires? Probably sleep for X seconds and try again.
			// Let's just panic for now.
			panic!("Failed to fetch next extrinsics.")
		};

		// If the sub returns no elements then something is wrong with the subscription
		assert!(list.len() > 0);

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
			handle_send_message_ext(send_message_exts, failed_ext.call.failed_txs);
		}

		if !execute_exts.is_empty() {
			handle_execute_ext(execute_exts);
		}
	}

	Ok(())
}

fn handle_send_message_ext(list: Vec<BlockRawExtrinsic>, failed_list: Vec<u32>) {
	// TODO don't include TXs that failed
	assert_eq!(failed_list.len(), 0);

	// For testing reason let's just print them for now.
	let list: Result<Vec<BlockSignedExtrinsic<SendMessage>>, _> = list
		.into_iter()
		.map(|x| BlockSignedExtrinsic::<SendMessage>::try_from(x))
		.collect();
	let Ok(list) = list else {
		// TODO proper error handling
		panic!("Failed to convert one Send Message from Raw to Ext");
	};
	for ext in list {
		println!(
			"✉️  Send Message: Message: {:?}, To: {:?}, Domain: {}",
			ext.call.message, ext.call.to, ext.call.domain
		)
	}
}

fn handle_execute_ext(list: Vec<BlockRawExtrinsic>) {
	// For testing reason let's just print them for now.
	let list: Result<Vec<BlockSignedExtrinsic<Execute>>, _> = list
		.into_iter()
		.map(|x| BlockSignedExtrinsic::<Execute>::try_from(x))
		.collect();
	let Ok(list) = list else {
		// TODO proper error handling
		panic!("Failed to convert one Send Message from Raw to Ext");
	};
	for ext in list {
		println!("☠️  Execute: From: {:?}, To: {:?}", ext.call.addr_message.from, ext.call.addr_message.to,)
	}
}
