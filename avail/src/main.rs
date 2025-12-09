mod configuration;
mod error;
mod execute;
mod execute_db;
mod parse;
mod send_message;
mod send_message_db;
mod sync;

use avail_rust::{
	H256, HasHeader,
	avail::{timestamp::tx::Set, vector::tx::FailedSendMessageTxs},
	block::{BlockEncodedExtrinsicsQuery, BlockExtrinsic, extrinsic_options::Options},
};
use tokio::runtime::Runtime;
use tracing::error as terror;
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
	setup_tracing();

	// Load configuration
	// There is no point in retrying. We will get the same error back each time.
	let config = match configuration::Configuration::new() {
		Ok(x) => x,
		Err(err) => {
			terror!("Failed to load configuration. Existing program. Reason: {}", err);
			return;
		},
	};

	// Create runtime
	// There is no point in retrying. We will get the same error back each time.
	let runtime = match Runtime::new() {
		Ok(r) => r,
		Err(err) => {
			terror!("Failed to create runtime. Existing program. Reason: {}", err);
			return;
		},
	};

	runtime.block_on(async move {
		let mut tasks = Vec::with_capacity(2);
		// Run Send Message Indexer
		if let Some(config) = config.send_message {
			let t = tokio::spawn(async move { send_message::run_indexer(config).await });
			tasks.push(t);
		}

		// Run Execute Indexer
		if let Some(config) = config.execute {
			let t = tokio::spawn(async move { execute::run_indexer(config).await });
			tasks.push(t);
		}

		for task in tasks {
			let _ = task.await;
		}
	});
}

fn setup_tracing() {
	let builder = tracing_subscriber::fmt::SubscriberBuilder::default();
	_ = builder.json().finish().try_init();
}

async fn get_block_height(
	block_height: Option<u32>,
	db: &send_message_db::Database,
	node: &avail_rust::Client,
) -> Result<u32, String> {
	if let Some(block_height) = block_height {
		return Ok(block_height);
	}

	if let Some(block_height) = db.find_highest_source_block_number().await? {
		return Ok(block_height);
	}

	node.finalized().block_height().await.map_err(|e| e.to_string())
}

async fn fetch_block_timestamp_and_failed_txs(
	node: avail_rust::Client,
	block_hash: H256,
) -> Result<(u64, Vec<u32>), String> {
	let tracked_calls: Vec<(u8, u8)> = vec![Set::HEADER_INDEX, FailedSendMessageTxs::HEADER_INDEX];

	let query = BlockEncodedExtrinsicsQuery::new(node, block_hash.into());
	let opts = Options { filter: Some(tracked_calls.into()), ..Default::default() };
	let raw_exts = query.all(opts).await.map_err(|e| {
		std::format!("Failed to fetch block timestamp and failed txs extrinsics. Error: {}", e.to_string())
	})?;

	let set_tx = raw_exts
		.iter()
		.find(|x| (x.metadata.pallet_id, x.metadata.variant_id) == Set::HEADER_INDEX);
	let Some(set_tx) = set_tx else {
		return Err(std::format!("Failed to fetch and find Timestamp::Set extrinsic"));
	};
	let set_tx = match BlockExtrinsic::<Set>::try_from(set_tx.clone()) {
		Ok(x) => x,
		Err(err) => {
			return Err(std::format!("Failed convert raw Timestamp::Set to normal extrinsic. Reason: {}", err));
		},
	};

	let failed_tx = raw_exts
		.iter()
		.find(|x| (x.metadata.pallet_id, x.metadata.variant_id) == FailedSendMessageTxs::HEADER_INDEX);
	let Some(failed_tx) = failed_tx else {
		return Err(std::format!("Failed to fetch and find Vector::FailedSendMessageTxs extrinsic"));
	};
	let failed_tx = match BlockExtrinsic::<FailedSendMessageTxs>::try_from(failed_tx.clone()) {
		Ok(x) => x,
		Err(err) => {
			return Err(std::format!(
				"Failed convert raw Vector::FailedSendMessageTxs to normal extrinsic. Reason: {}",
				err
			));
		},
	};

	Ok((set_tx.call.now / 1000, failed_tx.call.failed_txs))
}
