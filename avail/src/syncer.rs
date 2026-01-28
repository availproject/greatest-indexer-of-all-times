use std::time::{Duration, Instant};

use avail_rust::{Client, block::extrinsic_options::Options};
use tokio::task::JoinHandle;
use tracing::{error as terror, info};

use crate::{
	db::{self, Database, DbEntry},
	indexer::{add_to_db, convert_extrinsics_to_table_entries, fetch_block_timestamp_and_failed_txs},
};

const SLEEP_DURATION: Duration = Duration::from_secs(30);
const DISPLAY_MESSAGE_INTERVAL_SECS: u64 = 60;

pub struct SyncStats {
	pub total_indexed: u32,
	pub previously_indexed: u32,
	pub first_time: bool,
	pub checkpoint: Instant,
}

impl SyncStats {
	pub fn new() -> Self {
		Self {
			total_indexed: 0,
			previously_indexed: 0,
			first_time: true,
			checkpoint: Instant::now(),
		}
	}

	pub fn maybe_display_stats(&mut self, last_indexed_height: u32, final_height: u32) {
		if !((self.checkpoint.elapsed().as_secs() > DISPLAY_MESSAGE_INTERVAL_SECS) || self.first_time) {
			return;
		}

		let bps = self.bps();
		self.first_time = false;
		self.checkpoint = Instant::now();
		self.previously_indexed = self.total_indexed;

		let blocks_left_to_index = final_height.saturating_add(1).saturating_sub(last_indexed_height);
		info!(
			last_indexed_height,
			final_height,
			total_indexed = self.total_indexed,
			blocks_left_to_index,
			bps,
			"Syncing..."
		);
	}

	pub fn bps(&self) -> f32 {
		let elapsed = self.checkpoint.elapsed();
		let diff = self.total_indexed.saturating_sub(self.previously_indexed);
		let elapsed = elapsed.as_millis() as f32;
		if elapsed > 0f32 {
			((diff * 1000) as f32) / elapsed
		} else {
			0f32
		}
	}
}

#[derive(Clone)]
pub struct TaskParams {
	pub node: Client,
	pub filter: Options,
	pub block_height: u32,
}

impl TaskParams {
	pub fn new(node: Client, filter: Options) -> Self {
		Self { node, filter, block_height: 0 }
	}
}

struct TaskResult {
	db_entries: Vec<DbEntry>,
	execute_entries: Vec<db::execute_table::TableEntry>,
	send_message_entries: Vec<db::send_message_table::TableEntry>,
	block_height: u32,
}

#[derive(Debug)]
struct ProcessedHeight {
	pub height: Option<u32>,
	pub error: Option<String>,
}

impl ProcessedHeight {
	pub fn new(height: Option<u32>, error: Option<String>) -> Self {
		Self { height, error }
	}
}

pub struct Syncer {
	pub next_height_to_sync: u32,
	pub finalized_height: u32,
	pub task_count: usize,
	pub stats: SyncStats,
}

impl Syncer {
	pub fn new(next_height_to_sync: u32, finalized_height: u32, task_count: u32) -> Self {
		Self {
			next_height_to_sync,
			finalized_height,
			task_count: task_count.max(1) as usize,
			stats: SyncStats::new(),
		}
	}

	pub async fn run(mut self, filter: Options, avail_url: &str, db: &Database) -> Result<u32, String> {
		// Create Main Node
		let node = Client::new(avail_url)
			.await
			.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;

		// Handles
		let mut handles = Vec::with_capacity(self.task_count);

		// Create Task Params
		let n = Instant::now();
		let mut task_params = create_task_params(avail_url, self.task_count, filter.clone()).await?;
		//println!("Creating Task Params Time: {:?}", n.elapsed());

		self.stats.checkpoint = Instant::now();
		loop {
			if self.is_done(&node).await {
				info!(last_synced_height = self.next_height_to_sync.saturating_sub(1), "Syncing done");
				return Ok(self.next_height_to_sync);
			}

			let processed_height = Self::process_n_blocks(
				self.next_height_to_sync,
				self.finalized_height,
				self.task_count as u32,
				db,
				&mut task_params,
				&mut handles,
			)
			.await;

			if let Some(processed_height) = processed_height.height {
				self.stats.total_indexed += processed_height
					.saturating_add(1)
					.saturating_sub(self.next_height_to_sync);
				self.next_height_to_sync = processed_height + 1;
			}

			self.stats
				.maybe_display_stats(self.next_height_to_sync.saturating_sub(1), self.finalized_height);

			if let Some(err) = processed_height.error {
				terror!(
					error = err,
					sleep_duration_secs = SLEEP_DURATION.as_secs(),
					"Failed to sync some of of the blocks. Sleeping and then retrying."
				);
				tokio::time::sleep(SLEEP_DURATION).await;
			}
		}
	}

	async fn process_n_blocks(
		start_block_height: u32,
		end_block_height: u32,
		task_count: u32,
		db: &Database,
		task_params: &mut Vec<TaskParams>,
		handles: &mut Vec<JoinHandle<Result<TaskResult, String>>>,
	) -> ProcessedHeight {
		// Update bock height of every task param
		update_task_params(start_block_height, end_block_height, task_count, task_params);

		// Create tasks
		let n = Instant::now();
		spawn_tasks(handles, &task_params);
		let spawn_time = n.elapsed();

		// Process Results
		let n = Instant::now();
		let processed_height = process_results(db, handles).await;
		let process_time = n.elapsed();

		//info!(?spawn_time, ?process_time);

		processed_height
	}

	async fn is_done(&mut self, node: &Client) -> bool {
		const THRESHOLD: u32 = 10;

		if self.finalized_height.saturating_sub(self.next_height_to_sync) <= THRESHOLD {
			self.finalized_height = node
				.finalized()
				.block_height()
				.await
				.unwrap_or_else(|_| self.finalized_height);

			if self.finalized_height.saturating_sub(self.next_height_to_sync) <= THRESHOLD {
				return true;
			}
		}

		return false;
	}
}

fn update_task_params(start_height: u32, end_height: u32, task_count: u32, task_params: &mut Vec<TaskParams>) {
	let expected_length = end_height
		.saturating_add(1)
		.saturating_sub(start_height)
		.min(task_count) as usize;
	for (i, param) in task_params.iter_mut().enumerate() {
		param.block_height = start_height + i as u32;
	}
	task_params.truncate(expected_length);
}

async fn create_task_params(avail_url: &str, task_count: usize, filter: Options) -> Result<Vec<TaskParams>, String> {
	let mut task_params: Vec<TaskParams> = Vec::with_capacity(task_count);
	for _ in 0..task_count {
		let node = Client::new(avail_url)
			.await
			.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;
		task_params.push(TaskParams::new(node, filter.clone()));
	}

	Ok(task_params)
}

fn spawn_tasks(handles: &mut Vec<JoinHandle<Result<TaskResult, String>>>, params: &[TaskParams]) {
	handles.clear();
	for param in params.iter() {
		let params = param.clone();
		let handle = tokio::spawn(async move { task(params).await });
		handles.push(handle);
	}
}

async fn process_results(db: &Database, handles: &mut [JoinHandle<Result<TaskResult, String>>]) -> ProcessedHeight {
	let mut processed_height = None;
	for handle in handles {
		let result = match handle.await {
			Ok(x) => x,
			Err(err) => {
				return ProcessedHeight::new(processed_height, Some(err.to_string()));
			},
		};
		let result = match result {
			Ok(x) => x,
			Err(err) => {
				return ProcessedHeight::new(processed_height, Some(err));
			},
		};
		if let Err(error) = add_to_db(db, result.db_entries, result.execute_entries, result.send_message_entries).await
		{
			return ProcessedHeight::new(processed_height, Some(error));
		}
		processed_height = Some(result.block_height);
	}

	ProcessedHeight::new(processed_height, None)
}

async fn task(params: TaskParams) -> Result<TaskResult, String> {
	let TaskParams { node, filter, block_height } = params;
	let block = avail_rust::block::encoded::BlockEncodedExtrinsicsQuery::new(node.clone(), block_height.into());
	let list = block.all(filter).await.map_err(|e| e.to_string())?;

	if list.is_empty() {
		return Ok(TaskResult {
			db_entries: Vec::new(),
			execute_entries: Vec::new(),
			send_message_entries: Vec::new(),
			block_height,
		});
	}

	let block_hash = node
		.chain()
		.block_hash(Some(block_height))
		.await
		.map_err(|e| e.to_string())?
		.ok_or(std::format!("Failed to fetch block hash for block height: {}", block_height))?;

	let (timestamp, failed_txs) = fetch_block_timestamp_and_failed_txs(node.clone(), block_hash).await?;
	let table_entries =
		convert_extrinsics_to_table_entries(&node, list, block_height, block_hash, timestamp, failed_txs).await?;

	Ok(TaskResult {
		db_entries: table_entries.0,
		execute_entries: table_entries.1,
		send_message_entries: table_entries.2,
		block_height,
	})
}
