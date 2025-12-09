use std::env;
use tracing::info;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct ConfigurationFile {
	pub db_url: Option<String>,
	pub avail_url: Option<String>,
	pub block_height: Option<String>,
	pub avail_table_name: Option<String>,
	pub eth_table_name: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Configuration {
	pub db_url: String,
	pub avail_url: String,
	pub block_height: Option<u32>,
	pub avail_table_name: String,
	pub eth_table_name: String,
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
			info!("AVAIL_URL from ENV");
			value
		} else if let Some(value) = config_file.avail_url {
			info!("AVAIL_URL from FILE");
			value
		} else {
			info!("Failed to read AVAIL_URL either from ENV or config file. Defaulting to Turing");
			avail_rust::prelude::MAINNET_ENDPOINT.to_owned()
		};

		let block_height: Option<u32> = if let Ok(value) = env::var("BLOCK_HEIGHT") {
			info!("BLOCK_HEIGHT from ENV");
			Some(
				value
					.parse::<u32>()
					.map_err(|e| std::format!("Failed to parse block height as u32. {}", e))?,
			)
		} else if let Some(value) = config_file.block_height {
			info!("BLOCK_HEIGHT from FILE");
			Some(
				value
					.parse::<u32>()
					.map_err(|e| std::format!("Failed to parse block height as u32. {}", e))?,
			)
		} else {
			info!(
				"Failed to read BLOCK_HEIGHT either from ENV or config file. Defaulting to latest block height from db + 1"
			);
			None
		};

		let avail_table_name = if let Ok(value) = env::var("AVAIL_TABLE_NAME") {
			info!("AVAIL_TABLE_NAME from ENV");
			value
		} else if let Some(value) = config_file.avail_table_name {
			info!("AVAIL_TABLE_NAME from FILE");
			value
		} else {
			info!("Failed to read AVAIL_TABLE_NAME either from ENV or config file. Defaulting to avail_send_message");
			String::from("avail_send_message")
		};

		let eth_table_name = if let Ok(value) = env::var("ETH_TABLE_NAME") {
			info!("ETH_TABLE_NAME from ENV");
			value
		} else if let Some(value) = config_file.eth_table_name {
			info!("ETH_TABLE_NAME from FILE");
			value
		} else {
			info!("Failed to read ETH_TABLE_NAME either from ENV or config file. Defaulting to eth_send_message");
			String::from("eth_send_message")
		};

		Ok(Self {
			db_url,
			avail_url,
			block_height,
			avail_table_name,
			eth_table_name,
		})
	}
}
