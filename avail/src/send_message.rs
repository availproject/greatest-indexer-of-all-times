use std::time::Duration;

use crate::{
	configuration::TaskConfig,
	fetch_block_timestamp_and_failed_txs, get_block_height,
	parse::{SendMsgOrExecute, Target},
	send_message_db::{self, SendMessageDb},
};
use avail_rust::{
	HasHeader, MultiAddress,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		vector::{tx::SendMessage, types::Message},
	},
	block::{BlockEventsQuery, extrinsic_options::Options},
	subscription::EncodedExtrinsicSub,
};
use tracing::info;
use tracing::{error as terror, warn};

pub async fn run_indexer(config: TaskConfig) {
	let mut restart_block_height: Option<u32> = None;

	loop {
		let result = task(&config, &mut restart_block_height).await;
		if let Err(err) = result {
			terror!("Send Message Indexer returned an error. Error: {}. Restarting the indexer in 30 seconds.", err);
			tokio::time::sleep(Duration::from_secs(30)).await;
			continue;
		}

		warn!("Send Message Indexer finished. Existing.");

		return;
	}
}

async fn task(config: &TaskConfig, restart_block_height: &mut Option<u32>) -> Result<(), String> {
	let db = send_message_db::Database::new(&config.db_url, config.table_name.clone())
		.await
		.map_err(|e| std::format!("Failed to establish a connection with db. Reason: {}", e))?;
	db.create_table().await?;

	let node = avail_rust::Client::new(&config.avail_url)
		.await
		.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;
	let block_height = get_block_height(config.block_height, &db, &node).await?;

	// Here we define what extrinsics we will follow
	let tracked_calls: Vec<(u8, u8)> = vec![SendMessage::HEADER_INDEX, AsMulti::HEADER_INDEX, Proxy::HEADER_INDEX];

	// Create a subscription
	let opts = Options { filter: Some(tracked_calls.into()), ..Default::default() };
	let mut sub = EncodedExtrinsicSub::new(node.clone(), opts);
	sub.set_block_height(block_height);

	// Run subscription
	// For testing we will fetch the next 10 instances
	loop {
		let value = match sub.next().await {
			Ok(x) => x,
			Err(err) => {
				return Err(std::format!("Failed to fetch extrinsics from submission. Error: {}", err.to_string()));
			},
		};

		*restart_block_height = Some(value.block_height);

		let (timestamp, failed_txs) = fetch_block_timestamp_and_failed_txs(node.clone(), value.block_hash).await?;

		// If this fails, it means we failed to decode SendMessage, Execute, AsMulti or Proxy.
		let targets = crate::parse::parse_transactions(&value.list)?;
		let targets: Vec<Target> = targets
			.into_iter()
			.filter(|x| !failed_txs.contains(&x.ext_index))
			.collect();

		let iter = targets.iter().filter(|x| x.is_send_message_and_fungible());
		let count = iter.clone().count();
		if count == 0 {
			continue;
		}

		info!("✉️  Found {} Fungible Token Send Message transactions at height: {}", count, value.block_height);

		let events_query = BlockEventsQuery::new(node.clone(), value.block_height);
		for target in iter {
			let SendMsgOrExecute::Send(sm) = &target.call else {
				continue;
			};

			let Message::FungibleToken { asset_id, amount } = &sm.message else {
				continue;
			};

			let MultiAddress::Id(id) = &target.address else {
				continue;
			};

			// Fetch events
			let events = events_query.extrinsic(target.ext_index).await.map_err(|e| {
				std::format!(
					"Failed to fetch events for Send Message transaction. Block Height: {}, Tx Index: {}, Reason: {}",
					value.block_height,
					target.ext_index,
					e.to_string()
				)
			})?;

			if events.is_extrinsic_failed_present() {
				warn!(
					"Send Message transaction has ExtrinsicFailed event. Skipping this transaction. Block Height: {}, Tx Index: {}",
					value.block_height, target.ext_index
				);
				continue;
			}

			if let Some(success) = events.multisig_executed_successfully()
				&& !success
			{
				warn!(
					"Send Message transaction is inside Multisig and MultisigExecuted resulted in an error. Skipping this transaction. Block Height: {}, Tx Index: {}",
					value.block_height, target.ext_index
				);
				continue;
			}

			if let Some(success) = events.proxy_executed_successfully()
				&& !success
			{
				warn!(
					"Send Message transaction is inside Proxy and Proxy resulted in an error. Skipping this transaction. Block Height: {}, Tx Index: {}",
					value.block_height, target.ext_index
				);
				continue;
			}

			info!("✉️  Fungible Token Send Message: Message: {:?}, To: {:?}, Domain: {}", sm.message, sm.to, sm.domain);

			let sm = SendMessageDb::new(
				value.block_height,
				target.ext_index,
				value.block_hash,
				target.ext_hash,
				timestamp,
				*asset_id,
				id.clone(),
				sm.to,
				*amount,
			);

			let exists = db.row_exists(sm.id).await?;
			if exists {
				info!(
					"✉️  Fetched Send Message already in db. Block Height: {}, Tx Index: {}",
					value.block_height, target.ext_index
				);
				continue;
			}

			db.store_send_message(&sm).await?;
			info!("✉️  Send Message added to db. Block Height: {}, Tx Index: {}", value.block_height, target.ext_index);
		}
	}

	Ok(())
}
