mod configuration;
mod db;
mod error;
mod parse;
mod schema;
mod sync;

use crate::{
	configuration::Configuration,
	db::SendMessageDb,
	parse::{SendMsgOrExecute, Target},
};
use avail_rust::{
	BlockExtrinsic, BlockInfo, BlockWithRawExt, Client, H256, HasHeader, MultiAddress,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		timestamp::tx::Set,
		vector::{
			tx::{Execute, FailedSendMessageTxs, SendMessage},
			types::Message,
		},
	},
	block_api::BlockExtOptionsExpanded,
	subscription::RawExtrinsicSub,
};
use sqlx::types::chrono::DateTime;
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::{error as terror, info};
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

	loop {
		tracing::info!("Starting runtime.");
		let result = runtime.block_on(main_task(&config));
		if let Err(err) = result {
			terror!("Execution stopped. Reason: {}", err);
			tracing::warn!("Waiting 2 minutes then the runtime will be restarted.");
			std::thread::sleep(Duration::from_secs(2 * 60));
			continue;
		}

		return;
	}
}

fn setup_tracing() {
	let builder = tracing_subscriber::fmt::SubscriberBuilder::default();
	_ = builder.json().finish().try_init();
}

async fn setup_db(config: &Configuration) -> Result<db::Database, String> {
	let mut retires = 5;
	loop {
		let err =
			match db::Database::new(&config.db_url, config.avail_table_name.clone(), config.eth_table_name.clone())
				.await
			{
				Ok(x) => return Ok(x),
				Err(err) => err,
			};

		if retires == 0 {
			tracing::warn!("Failed to establish db connection. No retires left. Error: {}", err);
		}
		tracing::warn!("Failed to establish db connection. Error: {}. Retrying. Retires left: {}", err, retires);
		retires -= 1;
		tokio::time::sleep(Duration::from_secs(10)).await;
	}
}

async fn setup_avail_client(config: &Configuration) -> Result<avail_rust::Client, String> {
	let mut retires = 3;
	loop {
		let err = match Client::new(&config.avail_url).await {
			Ok(x) => return Ok(x),
			Err(err) => err,
		};

		if retires == 0 {
			tracing::warn!("Failed to establish node connection. No retires left. Error: {}", err);
		}
		tracing::warn!("Failed to establish node connection. Error: {}. Retrying. Retires left: {}", err, retires);
		retires -= 1;
		tokio::time::sleep(Duration::from_secs(10)).await;
	}
}

async fn get_block_height(config: &Configuration, db: &db::Database, node: &avail_rust::Client) -> Result<u32, String> {
	if let Some(block_height) = config.block_height {
		return Ok(block_height);
	}

	if let Some(block_height) = db.find_highest_source_block_number().await? {
		return Ok(block_height + 1);
	}

	node.finalized().block_height().await.map_err(|e| e.to_string())
}

async fn main_task(config: &Configuration) -> Result<(), String> {
	let db = setup_db(config).await?;
	db.create_table().await?;
	let node = setup_avail_client(config).await?;
	let block_height = get_block_height(config, &db, &node).await?;

	// Here we define what extrinsics we will follow
	let tracked_calls: Vec<(u8, u8)> = vec![
		SendMessage::HEADER_INDEX,
		Execute::HEADER_INDEX,
		AsMulti::HEADER_INDEX,
		Proxy::HEADER_INDEX,
	];

	// Create a subscription
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };
	let mut sub = RawExtrinsicSub::new(node.clone(), opts);
	sub.set_block_height(block_height);

	// Run subscription
	// For testing we will fetch the next 10 instances
	for _ in 0..10 {
		let (list, block_info) = match sub.next().await {
			Ok(x) => x,
			Err(err) => {
				return Err(std::format!("Failed to fetch extrinsics from submission. Error: {}", err.to_string()));
			},
		};

		// If the sub returns no elements then something is wrong with the subscription
		// TODO remove before pushing to Turing or Mainnet
		assert!(!list.is_empty());

		dbg!(block_info);
		let (timestamp, failed_txs) = fetch_block_timestamp_and_failed_txs(node.clone(), block_info.hash).await?;

		// If this fails, it means we failed to decode SendMessage, Execute, AsMulti or Proxy.
		let targets = parse::parse_transactions(&list)?;
		let targets: Vec<Target> = targets
			.into_iter()
			.filter(|x| !failed_txs.contains(&x.ext_index))
			.collect();

		for target in targets {
			if let SendMsgOrExecute::Send(sm) = &target.call {
				println!("✉️  Send Message: Message: {:?}, To: {:?}, Domain: {}", sm.message, sm.to, sm.domain);
				let Message::FungibleToken { asset_id, amount } = &sm.message else {
					continue;
				};
				let MultiAddress::Id(id) = &target.address else {
					continue;
				};

				let sm = SendMessageDb::new(
					block_info.height,
					target.ext_index,
					block_info.hash,
					target.ext_hash,
					timestamp,
					*asset_id,
					id.clone(),
					sm.to,
					*amount,
				);

				db.store_send_message(&sm).await?;
				info!("Stored message. :)");
			}
		}

		// From this point on we should only fail in writing to database.
		// If we fail to write we must return an error and restart everything.
	}

	Ok(())
}

async fn fetch_block_timestamp_and_failed_txs(
	node: avail_rust::Client,
	block_hash: H256,
) -> Result<(u64, Vec<u32>), String> {
	let tracked_calls: Vec<(u8, u8)> = vec![Set::HEADER_INDEX, FailedSendMessageTxs::HEADER_INDEX];

	let block = BlockWithRawExt::new(node, block_hash);
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };
	let raw_exts = block.all(opts).await.map_err(|e| {
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
			return Err(std::format!("Failed convert raw Timestamp::Set to normal exttrinsic. Reason: {}", err));
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
				"Failed convert raw Vector::FailedSendMessageTxs to normal exttrinsic. Reason: {}",
				err
			));
		},
	};

	Ok((set_tx.call.now / 1000, failed_tx.call.failed_txs))
}
