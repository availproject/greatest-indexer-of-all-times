use crate::{
	common::{convert_extrinsics_to_table_entries, fetch_block_timestamp_and_failed_txs},
	configuration::Configuration,
	db::{DataForDatabase, Database},
	stats::IndexerStats,
};
use avail_rust::{
	Client, HasHeader,
	avail::vector::tx::{Execute, SendMessage},
	block::extrinsic_options::Options,
};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tracing::{error as terror, info};

const SLEEP_DURATION: Duration = Duration::from_secs(30);

pub struct Indexer {
	node: Client,
	db: Database,
	config: Configuration,
	next_height_to_index: u32,
	finalized_height: u32,
	stats: IndexerStats,
	filter: Options,
}

impl Indexer {
	/// Creates DB and Node instance. Calculates start height.
	pub async fn new(config: Configuration) -> Result<Self, String> {
		let db = Database::new(
			&config.db_url,
			config.table_name.clone(),
			config.send_message_table_name.clone(),
			config.execute_table_name.clone(),
		)
		.await
		.map_err(|e| std::format!("Failed to establish a connection with db. Reason: {}", e))?;

		let node = avail_rust::Client::new(&config.avail_url)
			.await
			.map_err(|e| std::format!("Failed to establish a connection with avail node. Reason: {}", e.to_string()))?;
		let next_height_to_index = define_next_height_to_index(config.block_height, &db, &node).await?;
		let finalized_height = node.finalized().block_height().await.map_err(|e| e.to_string())?;

		// Here we define what extrinsics we will follow
		let tracked_calls: Vec<(u8, u8)> = vec![SendMessage::HEADER_INDEX, Execute::HEADER_INDEX];
		let filter = Options::default().filter(tracked_calls);

		Ok(Self {
			node,
			db,
			config,
			next_height_to_index,
			finalized_height,
			stats: IndexerStats::new(),
			filter,
		})
	}

	pub async fn run(mut self) -> Result<(), String> {
		let max_task_count = self.config.max_task_count;

		info!(
			start_height = self.next_height_to_index,
			finalized_height = self.finalized_height,
			max_task_count,
			"Indexer up and running."
		);

		// Calculate how many tasks do we need.
		let blocks_to_index_count = self.blocks_to_index_count();
		let task_count = blocks_to_index_count.min(max_task_count);
		if task_count < max_task_count {
			info!(
				blocks_to_index_count,
				task_count = task_count,
				"Not that many blocks to index. Reduced max task count"
			);
		} else {
			info!(
				blocks_to_index_count,
				task_count = max_task_count,
				"We have many blocks to index. Using max task count"
			);
		}

		// Handles
		let mut handles = Vec::with_capacity(task_count as usize);

		info!(count = task_count, "Creating HTTP Node connections...");
		// Create Task Params
		let mut task_params =
			create_task_params(&self.config.avail_url, task_count as usize, self.filter.clone()).await?;

		info!("Main loop started");
		self.stats.checkpoint = Instant::now();
		loop {
			self.sleep_if_ahead().await;

			if let Err(err) = self.update_task_count(&mut task_params).await {
				terror!(error = err, "Failed to update task count. Sleeping and then retrying.");
				tokio::time::sleep(SLEEP_DURATION).await;
				continue;
			}

			let processed_height = self.process_n_blocks(&mut task_params, &mut handles).await;

			if let Some(processed_height) = processed_height.height {
				self.stats.total_indexed += processed_height
					.saturating_add(1)
					.saturating_sub(self.next_height_to_index);
				self.next_height_to_index = processed_height + 1;
			}

			self.stats.maybe_display_stats(
				self.next_height_to_index.saturating_sub(1),
				self.finalized_height,
				self.blocks_to_index_count(),
			);

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

	async fn update_task_count(&self, task_params: &mut Vec<TaskParams>) -> Result<(), String> {
		let expected_count = self.blocks_to_index_count().min(self.config.max_task_count) as usize;
		let current_count = task_params.len();

		if expected_count == current_count {
			return Ok(());
		}

		if expected_count > current_count {
			let diff = expected_count.saturating_sub(current_count);
			for _ in 0..diff {
				let node = Client::new(&self.config.avail_url).await.map_err(|e| e.to_string())?;
				task_params.push(TaskParams::new(node, self.filter.clone()));
			}

			let new_task_count = task_params.len();
			info!(previous_task_count = current_count, new_task_count, "Task count has increased");
			return Ok(());
		}

		task_params.truncate(expected_count.max(1));
		let new_task_count = task_params.len();
		info!(previous_task_count = current_count, new_task_count, "Task count has decreased");

		Ok(())
	}

	async fn process_n_blocks(
		&self,
		task_params: &mut Vec<TaskParams>,
		handles: &mut Vec<JoinHandle<Result<TaskResult, String>>>,
	) -> ProcessedHeight {
		// Update bock height of every task param
		update_task_params(self.next_height_to_index, task_params);
		spawn_tasks(handles, &task_params);
		process_results(&self.db, handles).await
	}

	async fn sleep_if_ahead(&mut self) {
		loop {
			if self.finalized_height >= self.next_height_to_index {
				return;
			}

			self.finalized_height = self
				.node
				.finalized()
				.block_height()
				.await
				.unwrap_or_else(|_| self.finalized_height);

			if self.next_height_to_index > self.finalized_height {
				// Nothing to do besides sleeping.
				tokio::time::sleep(Duration::from_secs(60)).await;
				continue;
			}
		}
	}

	fn blocks_to_index_count(&self) -> u32 {
		self.finalized_height
			.saturating_add(1)
			.saturating_sub(self.next_height_to_index)
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
	pub db_data: DataForDatabase,
	pub block_height: u32,
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

fn update_task_params(start_height: u32, task_params: &mut Vec<TaskParams>) {
	for (i, param) in task_params.iter_mut().enumerate() {
		param.block_height = start_height + i as u32;
	}
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

		if let Err(error) = db.insert(result.db_data).await {
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
		return Ok(TaskResult { db_data: Default::default(), block_height });
	}

	let block_hash = node
		.chain()
		.block_hash(Some(block_height))
		.await
		.map_err(|e| e.to_string())?
		.ok_or(std::format!("Failed to fetch block hash for block height: {}", block_height))?;

	let (timestamp, failed_txs) = fetch_block_timestamp_and_failed_txs(node.clone(), block_hash).await?;
	let db_data =
		convert_extrinsics_to_table_entries(&node, list, block_height, block_hash, timestamp, failed_txs).await?;

	Ok(TaskResult { db_data, block_height })
}

pub async fn define_next_height_to_index(
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
