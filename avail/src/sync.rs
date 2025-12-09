use std::{
	sync::{Arc, Mutex},
	time::Duration,
};

use avail_rust::{
	BlockRawExtrinsic, BlockRef, BlockWithRawExt, Client, HasHeader, MAINNET_ENDPOINT,
	avail::vector::tx::{Execute, SendMessage},
	block::BlockExtOptionsExpanded,
	subscription::Sub,
};
use tokio::sync::{
	mpsc,
	mpsc::{Receiver, Sender},
};
use tracing::info;

type ChannelMessage = (Vec<BlockRawExtrinsic>, BlockRef);

pub struct SyncManager {
	sync_from: u32,
}

impl SyncManager {
	pub fn new(sync_from: u32) -> Self {
		Self { sync_from }
	}

	pub async fn run(client: Client) {
		let start = std::time::Instant::now();

		let (tx, mut rx) = mpsc::channel::<ChannelMessage>(1000);
		let tracked_calls: Vec<(u8, u8)> = vec![SendMessage::HEADER_INDEX, Execute::HEADER_INDEX];

		let sync_target = Arc::new(Mutex::new((0u32, 1_900_000u32)));

		info!("Building client...");

		let mut handles = Vec::with_capacity(1000);
		for _ in 0..500 {
			tokio::time::sleep(Duration::from_millis(10)).await;
			handles.push(tokio::spawn(async move { Client::new("https://100x-rpc.avail.so/rpc").await.unwrap() }));
		}
		let mut clients: Vec<Client> = Vec::with_capacity(1000);
		for h in handles {
			clients.push(h.await.unwrap());
		}
		info!("...Done");

		info!("Spawning futures...");
		for c in clients.into_iter().rev() {
			let calls = tracked_calls.clone();
			let tx_copy = tx.clone();
			let st = sync_target.clone();
			tokio::spawn(async move { fetch_blocks_task(c, calls, tx_copy, st) }.await);
		}
		info!("...Done");

		// let calls = tracked_calls.clone();
		// let tx_copy = tx.clone();
		// tokio::spawn(async move { fetch_blocks_task(c2, 1909600, 1909692, calls, tx_copy) }.await);

		/* 		let c = client.clone();
		let calls = tracked_calls.clone();
		let tx_copy = tx.clone();
		tokio::spawn(async move { fetch_blocks_task(c, 1909200, 1909268, calls, tx_copy) }.await);

		let c = client.clone();
		let calls = tracked_calls.clone();
		let tx_copy = tx.clone();
		tokio::spawn(async move { fetch_blocks_task(c, 1909200, 1909268, calls, tx_copy) }.await);

		let c = client.clone();
		let calls = tracked_calls.clone();
		let tx_copy = tx.clone();
		tokio::spawn(async move { fetch_blocks_task(c, 1909200, 1909268, calls, tx_copy) }.await); */

		// We don't need it anymore
		drop(tx);

		let mut threshold = 0;
		let mut count = 0;
		loop {
			let res = rx.recv().await;
			let Some(res) = res else {
				info!("finished in {:?}, count: {}", start.elapsed(), count);
				return;
			};

			if res.1.height > threshold {
				info!("Threshold: {}", res.1.height);
				threshold = res.1.height + 25_000;
			}

			count += res.0.len();
		}
	}
}

pub async fn fetch_blocks_task(
	client: Client,
	tracked_calls: Vec<(u8, u8)>,
	tx: Sender<ChannelMessage>,
	sync_target: Arc<Mutex<(u32, u32)>>,
) {
	let opts = BlockExtOptionsExpanded { filter: Some(tracked_calls.into()), ..Default::default() };

	loop {
		let (from, to) = {
			let mut lock = sync_target.lock().unwrap();
			if lock.0 > lock.1 {
				return;
			}

			let start = lock.0;
			let mut end = start + 100;
			lock.0 = end;
			if lock.0 > lock.1 {
				end = lock.1;
			}
			(start, end)
		};

		let mut sub = Sub::new();
		sub.set_block_height(from);

		loop {
			let info = sub.next(&client).await.unwrap();
			let block = BlockWithRawExt::new(client.clone(), info.hash);
			let txs = block.all(opts.clone()).await.unwrap();

			if txs.len() > 0 {
				tx.send((txs, info)).await.unwrap();
			}

			if info.height >= to {
				break;
			}
		}
	}
}

pub async fn process_blocks_task() {}
