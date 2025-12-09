use std::env;
use tracing::info;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct ConfigurationFile {
	pub db_url: Option<String>,
	pub avail_url: Option<String>,
	pub table_name: Option<String>,
	pub block_height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Configuration {
	pub db_url: String,
	pub avail_url: String,
	pub table_name: String,
	pub block_height: Option<u32>,
}

impl Configuration {
	pub fn new() -> Result<Self, String> {
		// First check if there is a file that we need to read
		let config_file = if let Ok(file_path) = env::var("CONFIG") {
			info!("ENV CONFIG was set to: {}", file_path);

			// Read file
			let config_file = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
			let config_file: ConfigurationFile = serde_json::from_str(&config_file).map_err(|e| e.to_string())?;
			config_file
		} else {
			info!("ENV CONFIG was not set. Not reading any config file");
			Default::default()
		};

		let db_url = if let Ok(value) = env::var("DB_URL") {
			info!("DB_URL from ENV");
			value
		} else if let Some(value) = config_file.db_url {
			info!("DB_URL from FILE");
			value
		} else {
			return Err("Failed to read DB_URL either from ENV or config file.".into());
		};

		let avail_url = if let Ok(value) = env::var("AVAIL_URL") {
			info!(avail_url = value, "AVAIL_URL from ENV");
			value
		} else if let Some(value) = config_file.avail_url {
			info!(avail_url = value, "AVAIL_URL from FILE");
			value
		} else {
			info!("Failed to read AVAIL_URL either from ENV or config file. Defaulting to Turing");
			avail_rust::prelude::MAINNET_ENDPOINT.to_owned()
		};

		let block_height: Option<u32> = if let Ok(value) = env::var("BLOCK_HEIGHT") {
			info!(block_height = value, "BLOCK_HEIGHT from ENV");
			Some(
				value
					.parse::<u32>()
					.map_err(|e| std::format!("Failed to parse BLOCK_HEIGHT as u32. {}", e))?,
			)
		} else if let Some(value) = config_file.block_height {
			info!(block_height = value, "BLOCK_HEIGHT from FILE");
			Some(value)
		} else {
			info!(
				"Failed to read BLOCK_HEIGHT either from ENV or config file. Defaulting to latest block height from db"
			);
			None
		};

		let table_name = if let Ok(value) = env::var("TABLE_NAME") {
			info!("TABLE_NAME from ENV");
			value
		} else if let Some(value) = config_file.table_name {
			info!("TABLE_NAME from FILE");
			value
		} else {
			info!("Failed to read TABLE_NAME either from ENV or config file. Defaulting to avail_table");
			String::from("avail_table")
		};

		Ok(Configuration { db_url, avail_url, table_name, block_height })
	}
}
