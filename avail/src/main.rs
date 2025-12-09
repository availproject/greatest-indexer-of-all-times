mod configuration;
mod db;
mod error;
mod schema;
mod sync;

use std::time::Duration;

use avail_rust::{
	BlockExtrinsic, BlockRawExtrinsic, BlockWithRawExt, Client, ExtrinsicCall, ExtrinsicSignature, H256, HasHeader,
	RawExtrinsic,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		timestamp::tx::Set,
		vector::tx::{Execute, FailedSendMessageTxs, SendMessage},
	},
	block_api::BlockExtOptionsExpanded,
	codec::Decode,
	subscription::RawExtrinsicSub,
};
use tokio::runtime::Runtime;
use tracing::error as terror;
use tracing_subscriber::util::SubscriberInitExt;

use crate::configuration::Configuration;

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
		let err = match db::Database::new(&config.db_url).await {
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
		let targets = parse_transactions(&list)?;
	}

	Ok(())
}

pub struct Target {
	signature: ExtrinsicSignature,
	send: Vec<SendMessage>,
	execute: Vec<Execute>,
}

impl Target {
	pub fn new(signature: ExtrinsicSignature) -> Self {
		Self { signature, send: Vec::new(), execute: Vec::new() }
	}
}

#[derive(Debug)]
pub enum SendMsgOrExecute {
	Send(SendMessage),
	Execute(Execute),
}

fn parse_transactions(list: &Vec<BlockRawExtrinsic>) -> Result<Vec<Target>, String> {
	let mut targets: Vec<Target> = Vec::with_capacity(list.len());
	for tx in list {
		let Some(raw_ext) = &tx.data else {
			return Err("Failed to fetch transaction with data. This is not good.".into());
		};

		let raw_ext = RawExtrinsic::try_from(raw_ext.as_str())?;
		let Some(signature) = raw_ext.signature else {
			return Err("Extrinsic did not had signature. This is not good".into());
		};

		let call = ExtrinsicCall::try_from(&raw_ext.call)?;
		let calls = parse_extrinsic_call(&call)?;

		let mut target = Target::new(signature);
		for call in calls {
			match call {
				SendMsgOrExecute::Send(x) => target.send.push(x),
				SendMsgOrExecute::Execute(x) => target.execute.push(x),
			}
		}

		targets.push(target);
	}

	Ok(targets)
}

fn parse_extrinsic_call(call: &ExtrinsicCall) -> Result<Vec<SendMsgOrExecute>, String> {
	let header = (call.pallet_id, call.variant_id);

	if header == SendMessage::HEADER_INDEX {
		return Ok(vec![SendMsgOrExecute::Send(parse_send_message_call(&call.data)?)]);
	}

	if header == Execute::HEADER_INDEX {
		return Ok(vec![SendMsgOrExecute::Execute(parse_execute_call(&call.data)?)]);
	}

	if header == AsMulti::HEADER_INDEX {
		return parse_multisig_call(&call.data);
	}

	if header == Proxy::HEADER_INDEX {
		return parse_proxy_call(&call.data);
	}

	Ok(Vec::new())
}

fn parse_send_message_call(mut call_data: &[u8]) -> Result<SendMessage, String> {
	SendMessage::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_execute_call(mut call_data: &[u8]) -> Result<Execute, String> {
	Execute::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_multisig_call(mut call_data: &[u8]) -> Result<Vec<SendMsgOrExecute>, String> {
	let multi = match AsMulti::decode(&mut call_data) {
		Ok(x) => x,
		Err(err) => {
			tracing::warn!(
				"Failed to convert raw extrinsic to multisig. That is OK as this multisig is probably not the one that we need. Err: {}",
				err
			);
			return Ok(Vec::new());
		},
	};

	parse_extrinsic_call(&multi.call)
}

fn parse_proxy_call(mut call_data: &[u8]) -> Result<Vec<SendMsgOrExecute>, String> {
	let proxy = Proxy::decode(&mut call_data)
		.map_err(|e| std::format!("Failed to convert raw ext to Proxy::Proxy. Err: {}", e))?;

	parse_extrinsic_call(&proxy.call)
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

	Ok((set_tx.call.now, failed_tx.call.failed_txs))
}

// async fn handle_send_message_ext(
// 	list: Vec<BlockRawExtrinsic>,
// 	failed_list: Vec<u32>,
// 	block_ref: BlockRef,
// 	conn: &mut PgConnection,
// ) -> Result<(), Error> {
// 	// TODO don't include TXs that failed
// 	assert_eq!(failed_list.len(), 0);

// 	// For testing reason let's just print them for now.
// 	let list: Result<Vec<BlockSignedExtrinsic<SendMessage>>, _> = list
// 		.into_iter()
// 		.map(BlockSignedExtrinsic::<SendMessage>::try_from)
// 		.collect();
// 	let Ok(list) = list else {
// 		// TODO proper error handling
// 		panic!("Failed to convert one Send Message from Raw to Ext");
// 	};
// 	for ext in list {
// 		println!(
// 			"✉️  Send Message: Message: {:?}, To: {:?}, Domain: {}",
// 			ext.call.message, ext.call.to, ext.call.domain
// 		);

// 		let (asset_id, amount) = match ext.call.message {
// 			FungibleToken { asset_id, amount } => (asset_id, amount),
// 			_ => continue,
// 		};

// 		let tx_ref: TxRef = (ext.metadata.ext_hash, ext.metadata.ext_index).into();
// 		let from = match ext.signature.address {
// 			avail_rust::MultiAddress::Id(x) => x,
// 			_ => panic!("Ohh, account is not of type ID. TODO"),
// 		};

// 		let entry = SendMessageEntry::new(block_ref, tx_ref, asset_id, amount, ext.call.to, from);
// 		entry.table_insert_entry(conn).await?;
// 		println!("✉️  Send Message: Added to table <3",);
// 		SendMessageEntry::table_list_entries(conn).await?;
// 	}
// 	Ok(())
// }

// fn handle_execute_ext(list: Vec<BlockRawExtrinsic>) {
// 	// For testing reason let's just print them for now.
// 	let list: Result<Vec<BlockSignedExtrinsic<Execute>>, _> = list
// 		.into_iter()
// 		.map(BlockSignedExtrinsic::<Execute>::try_from)
// 		.collect();
// 	let Ok(list) = list else {
// 		// TODO proper error handling
// 		panic!("Failed to convert one Execute from Raw to Ext");
// 	};
// 	for ext in list {
// 		println!("☠️  Execute: From: {:?}, To: {:?}", ext.call.addr_message.from, ext.call.addr_message.to,)
// 	}
// }
