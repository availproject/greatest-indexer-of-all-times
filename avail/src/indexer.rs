use std::time::{Duration, Instant};

use crate::{
	configuration::Configuration,
	db::{self, Database, DbEntry, execute_table::ExecuteTable, send_message_table::SendMessageTable},
	syncer::Syncer,
};
use avail_rust::block;
use avail_rust::{
	ExtrinsicDecodable, H256, HasHeader,
	avail::{
		timestamp::tx::Set,
		vector::{
			tx::{Execute, FailedSendMessageTxs, SendMessage},
			types::{AddressedMessage, Message},
		},
	},
	block::{BlockEncodedExtrinsicsQuery, BlockExtrinsic, extrinsic_options::Options},
	ext::const_hex,
	subscription::{EncodedExtrinsicSub, extrinsic::EncodedExtrinsicSubValue},
};
use tracing::info;
use tracing::{error as terror, warn};

const DISPLAY_MESSAGE_INTERVAL_SECS: u64 = 60 * 10;

pub async fn run_indexer(config: Configuration) -> Result<(), String> {
	let db = Database::new(
		&config.db_url,
		config.table_name.clone(),
		config.send_message_table_name.clone(),
		config.execute_table_name.clone(),
	)
	.await
	.map_err(|e| std::format!("Failed to establish a connection with db. Reason: {}", e))?;

	db.create_table().await?;
	ExecuteTable::create_table(&db).await?;
	SendMessageTable::create_table(&db).await?;

	let node = avail_rust::Client::new(&config.avail_url)
		.await
		.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;
	let start_block_height = get_block_height(config.block_height, &db, &node).await?;
	let finalized_height = node.finalized().block_height().await.unwrap_or(0);
	let diff = finalized_height.saturating_sub(start_block_height);

	info!(
		start_block_height = start_block_height,
		latest_finalized_block_height = finalized_height,
		diff = diff,
		task_count = config.task_count,
		"Init Params"
	);

	// Here we define what extrinsics we will follow
	let tracked_calls: Vec<(u8, u8)> = vec![SendMessage::HEADER_INDEX, Execute::HEADER_INDEX];
	let filter = Options::default().filter(tracked_calls);

	let syncer = Syncer::new(start_block_height, finalized_height, config.task_count);
	let mut start_height = syncer.run(filter.clone(), &config.avail_url, &db).await?;
	let mut restart_block_height: u32 = start_height;

	loop {
		let result = task(&node, &db, start_height, &mut restart_block_height, filter.clone()).await;
		if let Err(err) = result {
			start_height = restart_block_height;
			terror!(error = err, "Indexer returned an error. Restarting indexer in 30 seconds.");
			tokio::time::sleep(Duration::from_secs(30)).await;
			continue;
		}

		warn!("Indexer finished. Exiting.");
		return Ok(());
	}
}

async fn task(
	node: &avail_rust::Client,
	db: &Database,
	start_height: u32,
	restart_block_height: &mut u32,
	filter: Options,
) -> Result<(), String> {
	info!("Running regular indexing");

	let mut checkpoint = Instant::now();

	// Create a subscription
	let mut sub = EncodedExtrinsicSub::new(node.clone(), filter);
	sub.set_block_height(start_height);

	// Run subscription
	loop {
		let value = match sub.next_step().await {
			Ok(x) => x,
			Err(err) => {
				return Err(std::format!("Failed to fetch extrinsics from subscription. Error: {}", err.to_string()));
			},
		};
		*restart_block_height = value.block_height;

		if checkpoint.elapsed().as_secs() > DISPLAY_MESSAGE_INTERVAL_SECS {
			let finalized_height = node.finalized().block_height().await.unwrap_or(0);
			info!(
				current_height = value.block_height,
				finalized_height = finalized_height,
				"❤️  Indexer is still alive"
			);
			checkpoint = Instant::now();
		}

		if value.list.is_empty() {
			*restart_block_height = *restart_block_height + 1;
			continue;
		}

		let (timestamp, failed_txs) = fetch_block_timestamp_and_failed_txs(node.clone(), value.block_hash).await?;
		let (db_entries, execute_entries, send_message_entries) = convert_extrinsics_to_table_entries(
			node, value.list, value.block_height, value.block_hash, timestamp, failed_txs,
		)
		.await?;
		add_to_db(db, db_entries, execute_entries, send_message_entries).await?;

		*restart_block_height = *restart_block_height + 1;
	}
}

/// Possible types of Messages allowed by Avail to bridge to other chains.
#[derive(Debug, Clone, serde::Serialize)]
#[repr(u8)]
pub enum SerializedMessage {
	ArbitraryMessage(String) = 0,
	FungibleToken { asset_id: H256, amount: u128 } = 1,
}

impl From<Message> for SerializedMessage {
	fn from(value: Message) -> Self {
		match value {
			Message::ArbitraryMessage(items) => Self::ArbitraryMessage(const_hex::encode_prefixed(items)),
			Message::FungibleToken { asset_id, amount } => Self::FungibleToken { asset_id, amount },
		}
	}
}

impl SerializedMessage {
	pub fn kind(&self) -> &str {
		match self {
			SerializedMessage::ArbitraryMessage(_) => "ArbitraryMessage",
			SerializedMessage::FungibleToken { asset_id: _, amount: _ } => "FungibleToken",
		}
	}

	pub fn amount(&self) -> Option<u128> {
		match self {
			SerializedMessage::ArbitraryMessage(_) => None,
			SerializedMessage::FungibleToken { asset_id: _, amount } => Some(amount.clone()),
		}
	}
}

/// Message type used to bridge between Avail & other chains
#[derive(Debug, Clone, serde::Serialize)]
pub struct SerializedAddressedMessage {
	pub message: SerializedMessage,
	pub from: H256,
	pub to: H256,
	pub origin_domain: u32,
	pub destination_domain: u32,
	pub id: u64,
}

impl From<AddressedMessage> for SerializedAddressedMessage {
	fn from(value: AddressedMessage) -> Self {
		Self {
			message: value.message.into(),
			from: value.from,
			to: value.to,
			origin_domain: value.origin_domain,
			destination_domain: value.destination_domain,
			id: value.id,
		}
	}
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SerializeSendMessage {
	pub message: SerializedMessage,
	pub to: H256,
	pub domain: u32,
}

impl From<SendMessage> for SerializeSendMessage {
	fn from(value: SendMessage) -> Self {
		Self {
			message: value.message.into(),
			to: value.to,
			domain: value.domain,
		}
	}
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SerializeExecute {
	pub slot: u64,
	pub addr_message: SerializedAddressedMessage,
	pub account_proof: Vec<String>,
	pub storage_proof: Vec<String>,
}

impl From<Execute> for SerializeExecute {
	fn from(value: Execute) -> Self {
		Self {
			slot: value.slot,
			addr_message: value.addr_message.into(),
			account_proof: value
				.account_proof
				.into_iter()
				.map(|x| const_hex::encode_prefixed(x))
				.collect(),
			storage_proof: value
				.storage_proof
				.into_iter()
				.map(|x| const_hex::encode_prefixed(x))
				.collect(),
		}
	}
}

pub async fn add_to_db(
	db: &Database,
	db_entries: Vec<DbEntry>,
	execute_entries: Vec<db::execute_table::TableEntry>,
	send_message_entries: Vec<db::send_message_table::TableEntry>,
) -> Result<(), String> {
	for entry in db_entries {
		db.insert(entry).await?;
	}

	for entry in execute_entries {
		ExecuteTable::insert(entry, &db).await?;
	}

	for entry in send_message_entries {
		SendMessageTable::insert(entry, &db).await?;
	}

	Ok(())
}

pub async fn convert_extrinsics_to_table_entries(
	node: &avail_rust::Client,
	list: Vec<block::BlockEncodedExtrinsic>,
	block_height: u32,
	block_hash: H256,
	timestamp: u64,
	failed_txs: Vec<u32>,
) -> Result<(Vec<DbEntry>, Vec<db::execute_table::TableEntry>, Vec<db::send_message_table::TableEntry>), String> {
	let mut db_entries: Vec<DbEntry> = Vec::with_capacity(list.len());
	let mut send_message_entries: Vec<db::send_message_table::TableEntry> = Vec::with_capacity(list.len());
	let mut execute_entries: Vec<db::execute_table::TableEntry> = Vec::with_capacity(list.len());

	for ext in list {
		let mut main_entry = DbEntry {
			id: (block_height as u64) << 32 | ext.metadata.ext_index as u64,
			block_height,
			block_hash,
			block_timestamp: timestamp,
			ext_index: ext.metadata.ext_index,
			ext_hash: ext.metadata.ext_hash,
			signature_address: ext.ss58_address(),
			pallet_id: ext.metadata.pallet_id,
			variant_id: ext.metadata.variant_id,
			ext_success: None,
			ext_call: String::new(),
		};

		match ext.events(node.clone()).await {
			Ok(events) => main_entry.ext_success = Some(events.is_extrinsic_success_present()),
			_ => (),
		}

		if let Ok(send_message) = SendMessage::from_call(&ext.call) {
			if failed_txs.contains(&ext.metadata.ext_index) {
				warn!(
					block_height = main_entry.block_height,
					extrinsic_index = main_entry.ext_index,
					"✉️  Send Message found but skipped as it ext index is in failed txs list",
				);
				continue;
			}

			info!(block_height = main_entry.block_height, extrinsic_index = main_entry.ext_index, "✉️  Send Message",);
			let serialized_call = SerializeSendMessage::from(send_message);

			let extra_entry = db::send_message_table::TableEntry {
				id: main_entry.id,
				kind: serialized_call.message.kind().to_string(),
				amount: serialized_call.message.amount(),
				to: serialized_call.to,
			};

			let serialized_call = match serde_json::to_string(&serialized_call) {
				Ok(x) => x,
				Err(err) => {
					return Err(std::format!("Failed to serialize Send Message. Error: {}", err.to_string()));
				},
			};
			main_entry.ext_call = serialized_call;
			db_entries.push(main_entry);
			send_message_entries.push(extra_entry);

			continue;
		}

		if let Ok(execute) = Execute::from_call(&ext.call) {
			info!(block_height = main_entry.block_height, extrinsic_index = main_entry.ext_index, "☠️  Execute",);
			let serialized_call = SerializeExecute::from(execute);

			let extra_entry = db::execute_table::TableEntry {
				id: main_entry.id,
				kind: serialized_call.addr_message.message.kind().to_string(),
				amount: serialized_call.addr_message.message.amount(),
				to: serialized_call.addr_message.to,
				slot: serialized_call.slot,
				message_id: serialized_call.addr_message.id,
			};

			let serialized_call = match serde_json::to_string(&serialized_call) {
				Ok(x) => x,
				Err(err) => {
					return Err(std::format!("Failed to serialize Execute. Error: {}", err.to_string()));
				},
			};
			main_entry.ext_call = serialized_call;
			db_entries.push(main_entry);
			execute_entries.push(extra_entry);

			continue;
		}
	}

	Ok((db_entries, execute_entries, send_message_entries))
}

pub async fn fetch_block_timestamp_and_failed_txs(
	node: avail_rust::Client,
	block_hash: H256,
) -> Result<(u64, Vec<u32>), String> {
	let tracked_calls: Vec<(u8, u8)> = vec![Set::HEADER_INDEX, FailedSendMessageTxs::HEADER_INDEX];

	let query = BlockEncodedExtrinsicsQuery::new(node, block_hash.into());
	let opts = Options::default().filter(tracked_calls);
	let raw_exts = query.all(opts).await.map_err(|e| {
		std::format!("Failed to fetch block timestamp and failed txs extrinsics. Error: {}", e.to_string())
	})?;

	let set_tx = raw_exts
		.iter()
		.find(|x| (x.metadata.pallet_id, x.metadata.variant_id) == Set::HEADER_INDEX);
	let Some(set_tx) = set_tx else {
		return Err(std::format!("Failed to fetch and find Timestamp::Set extrinsic"));
	};
	let set_tx = match BlockExtrinsic::<Set>::try_from(set_tx) {
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
	let failed_tx = match BlockExtrinsic::<FailedSendMessageTxs>::try_from(failed_tx) {
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

pub async fn get_block_height(
	block_height: Option<u32>,
	db: &Database,
	node: &avail_rust::Client,
) -> Result<u32, String> {
	if let Some(block_height) = block_height {
		return Ok(block_height);
	}

	if let Some(block_height) = db.find_highest_block_height().await? {
		return Ok(block_height);
	}

	node.finalized().block_height().await.map_err(|e| e.to_string())
}
