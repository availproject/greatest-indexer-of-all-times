use std::time::Duration;

use crate::{
	configuration::TaskConfig,
	db::{self, SendMessageDb},
	fetch_block_timestamp_and_failed_txs, get_block_height,
	parse::{SendMsgOrExecute, Target},
};
use avail_rust::{
	BlockEvents, HasHeader, MultiAddress,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		vector::{tx::Execute, types::Message},
	},
	block_api::BlockExtOptionsExpanded,
	subscription::RawExtrinsicSub,
};
use tracing::info;
use tracing::{error as terror, warn};

pub async fn run_indexer(config: TaskConfig) {
	let mut restart_block_height: Option<u32> = None;

	loop {
		let result = task(&config, &mut restart_block_height).await;
		if let Err(err) = result {
			terror!("Execute Indexer returned an error. Error: {}. Restarting the indexer in 30 seconds.", err);
			tokio::time::sleep(Duration::from_secs(30)).await;
			continue;
		}

		warn!("Execute Indexer finished. Existing.");

		return;
	}
}

async fn task(config: &TaskConfig, restart_block_height: &mut Option<u32>) -> Result<(), String> {
	let db = db::Database::new(&config.db_url, config.table_name.clone())
		.await
		.map_err(|e| std::format!("Failed to establish a connection with db. Reason: {}", e))?;
	db.create_table().await?;

	let node = avail_rust::Client::new(&config.avail_url)
		.await
		.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;
	let block_height = get_block_height(config.block_height, &db, &node).await?;

	// Here we define what extrinsics we will follow
	let tracked_calls: Vec<(u8, u8)> = vec![Execute::HEADER_INDEX, AsMulti::HEADER_INDEX, Proxy::HEADER_INDEX];

	// Create a subscription
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };
	let mut sub = RawExtrinsicSub::new(node.clone(), opts);
	sub.set_block_height(block_height);

	// Run subscription
	// For testing we will fetch the next 10 instances
	loop {
		let (list, block_info) = match sub.next().await {
			Ok(x) => x,
			Err(err) => {
				return Err(std::format!("Failed to fetch extrinsics from submission. Error: {}", err.to_string()));
			},
		};

		*restart_block_height = Some(block_info.height);

		let (timestamp, _) = fetch_block_timestamp_and_failed_txs(node.clone(), block_info.hash).await?;

		// If this fails, it means we failed to decode SendMessage, Execute, AsMulti or Proxy.
		let targets = crate::parse::parse_transactions(&list)?;
		let iter = targets.iter().filter(|x| x.is_execute());
		let count = iter.clone().count();
		if count == 0 {
			continue;
		}

		info!("☠️  Found {} Execute transactions at height: {}", count, block_info.height);

		let block_events = BlockEvents::new(node.clone(), block_info.height);
		for target in iter {
			let SendMsgOrExecute::Execute(ex) = &target.call else {
				continue;
			};

			// Fetch events
			let events = block_events.ext(target.ext_index).await.map_err(|e| {
				std::format!(
					"Failed to fetch events for Execute transaction. Block Height: {}, Tx Index: {}, Reason: {}",
					block_info.height,
					target.ext_index,
					e.to_string()
				)
			})?;

			let Some(events) = events else {
				return Err(std::format!(
					"Failed to find events for Execute transaction. Block Height: {}, Tx Index: {}",
					block_info.height,
					target.ext_index,
				));
			};

			if events.is_extrinsic_failed_present() {
				warn!(
					"Execute transaction has ExtrinsicFailed event. Skipping this transaction. Block Height: {}, Tx Index: {}",
					block_info.height, target.ext_index
				);
				continue;
			}

			if let Some(success) = events.multisig_executed_successfully()
				&& !success
			{
				warn!(
					"Execute transaction is inside Multisig and MultisigExecuted resulted in an error. Skipping this transaction. Block Height: {}, Tx Index: {}",
					block_info.height, target.ext_index
				);
				continue;
			}

			if let Some(success) = events.proxy_executed_successfully()
				&& !success
			{
				warn!(
					"Execute transaction is inside Proxy and Proxy resulted in an error. Skipping this transaction. Block Height: {}, Tx Index: {}",
					block_info.height, target.ext_index
				);
				continue;
			}
		}
	}

	Ok(())
}
