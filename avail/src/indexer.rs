use crate::{configuration::Configuration, db::Database, syncer::Syncer};
use avail_rust::{
	Client, HasHeader,
	avail::vector::tx::{Execute, SendMessage},
	block::extrinsic_options::Options,
};

pub struct Indexer {
	pub node: Client,
	pub db: Database,
	pub config: Configuration,
	pub start_height: u32,
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
		let start_height = define_starting_height(config.block_height, &db, &node).await?;

		Ok(Self { node, db, config, start_height })
	}

	pub async fn run(self) -> Result<(), String> {
		let finalized_height = self.node.finalized().block_height().await.map_err(|e| e.to_string())?;

		// Here we define what extrinsics we will follow
		let tracked_calls: Vec<(u8, u8)> = vec![SendMessage::HEADER_INDEX, Execute::HEADER_INDEX];
		let filter = Options::default().filter(tracked_calls);

		let syncer = Syncer::new(self.start_height, finalized_height, self.config.task_count);
		_ = syncer.run(filter.clone(), &self.config.avail_url, &self.db).await?;
		Ok(())
	}
}

pub async fn define_starting_height(
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
