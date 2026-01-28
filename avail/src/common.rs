use crate::db::{self, DataForDatabase};
use avail_rust::{
	ExtrinsicDecodable, H256, HasHeader,
	avail::{
		timestamp::tx::Set,
		vector::{
			tx::{Execute, FailedSendMessageTxs, SendMessage},
			types::{AddressedMessage, Message},
		},
	},
	block,
	block::{BlockEncodedExtrinsicsQuery, BlockEvents, BlockEventsQuery, BlockExtrinsic, extrinsic_options::Options},
	ext::const_hex,
};
use tracing::{info, warn};

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
pub struct SerializedSendMessage {
	pub message: SerializedMessage,
	pub to: H256,
	pub domain: u32,
}

impl SerializedSendMessage {
	pub fn to_json(&self) -> Result<String, String> {
		serde_json::to_string(&self)
			.map_err(|err| std::format!("Failed to serialize Send Message. Error: {}", err.to_string()))
	}
}

impl From<SendMessage> for SerializedSendMessage {
	fn from(value: SendMessage) -> Self {
		Self {
			message: value.message.into(),
			to: value.to,
			domain: value.domain,
		}
	}
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SerializedExecute {
	pub slot: u64,
	pub addr_message: SerializedAddressedMessage,
	pub account_proof: Vec<String>,
	pub storage_proof: Vec<String>,
}

impl SerializedExecute {
	pub fn to_json(&self) -> Result<String, String> {
		serde_json::to_string(&self)
			.map_err(|err| std::format!("Failed to serialize Execute. Error: {}", err.to_string()))
	}
}

impl From<Execute> for SerializedExecute {
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

pub async fn convert_extrinsics_to_table_entries(
	node: &avail_rust::Client,
	list: Vec<block::BlockEncodedExtrinsic>,
	block_height: u32,
	block_hash: H256,
	block_timestamp: u64,
	failed_txs: Vec<u32>,
) -> Result<DataForDatabase, String> {
	let mut db_data = DataForDatabase::default();

	let mut events_query = BlockEventsQuery::new(node.clone(), block_hash);
	events_query.set_retry_on_error(Some(false));

	for ext in list {
		let mut main_entry =
			db::main_table::TableEntry::from_block_ext(block_height, block_hash, block_timestamp, &ext);

		let events = events_query
			.extrinsic(ext.ext_index())
			.await
			.unwrap_or_else(|_| BlockEvents::new(Vec::new()));
		if !events.is_empty() {
			main_entry.ext_success = Some(events.is_extrinsic_success_present())
		}

		let extrinsic_index = main_entry.ext_index;
		let id = main_entry.id;
		if let Ok(send_message) = SendMessage::from_call(&ext.call) {
			if failed_txs.contains(&ext.metadata.ext_index) {
				warn!(
					block_height,
					extrinsic_index = main_entry.ext_index,
					skipped = true,
					"✉️  Send Message found but skipped as it ext index is in failed txs list",
				);
				continue;
			}

			info!(block_height, extrinsic_index, "✉️  Send Message",);
			let serialized_call = SerializedSendMessage::from(send_message);
			let extra_entry = db::send_message_table::TableEntry::from_call(id, &serialized_call);

			main_entry.ext_call = serialized_call.to_json()?;
			db_data.main_entries.push(main_entry);
			db_data.send_message_entries.push(extra_entry);

			continue;
		}

		if let Ok(execute) = Execute::from_call(&ext.call) {
			info!(block_height, extrinsic_index, "☠️  Execute",);
			let serialized_call = SerializedExecute::from(execute);
			let extra_entry = db::execute_table::TableEntry::from_call(id, &serialized_call);

			main_entry.ext_call = serialized_call.to_json()?;
			db_data.main_entries.push(main_entry);
			db_data.execute_entries.push(extra_entry);

			continue;
		}
	}

	Ok(db_data)
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
