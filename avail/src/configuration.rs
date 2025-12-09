use std::env;
use tracing::info;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct ConfigurationFile {
	pub db_url: Option<String>,
	pub avail_url: Option<String>,
	pub send_message_block_height: Option<u32>,
	pub execute_block_height: Option<u32>,
	pub avail_table_name: Option<String>,
	pub eth_table_name: Option<String>,
	pub run_send_message: Option<bool>,
	pub run_execute: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TaskConfig {
	pub db_url: String,
	pub avail_url: String,
	pub table_name: String,
	pub block_height: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct Configuration {
	pub send_message: Option<TaskConfig>,
	pub execute: Option<TaskConfig>,
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

		let mut s = Configuration::default();

		let run_send_message: bool = if let Ok(value) = env::var("RUN_SEND_MESSAGE") {
			info!("RUN_SEND_MESSAGE from ENV");
			value
				.parse::<bool>()
				.map_err(|e| std::format!("Failed to parse RUN_SEND_MESSAGE as bool. {}", e))?
		} else if let Some(value) = config_file.run_send_message {
			info!("RUN_SEND_MESSAGE from FILE");
			value
		} else {
			info!("Failed to read RUN_SEND_MESSAGE either from ENV or config file. Defaulting to true");
			true
		};

		let run_execute: bool = if let Ok(value) = env::var("RUN_EXECUTE") {
			info!("RUN_EXECUTE from ENV");
			value
				.parse::<bool>()
				.map_err(|e| std::format!("Failed to parse RUN_EXECUTE as bool. {}", e))?
		} else if let Some(value) = config_file.run_execute {
			info!("RUN_EXECUTE from FILE");
			value
		} else {
			info!("Failed to read RUN_EXECUTE either from ENV or config file. Defaulting to true");
			true
		};

		if !run_send_message && !run_execute {
			return Err(String::from("Both RUN_EXECUTE and RUN_SEND_MESSAGE are set to false. There is nothing to do"));
		}

		if run_send_message {
			let block_height: Option<u32> = if let Ok(value) = env::var("SEND_MESSAGE_BLOCK_HEIGHT") {
				info!("SEND_MESSAGE_BLOCK_HEIGHT from ENV");
				Some(
					value
						.parse::<u32>()
						.map_err(|e| std::format!("Failed to parse SEND_MESSAGE_BLOCK_HEIGHT as u32. {}", e))?,
				)
			} else if let Some(value) = config_file.send_message_block_height {
				info!("SEND_MESSAGE_BLOCK_HEIGHT from FILE");
				Some(value)
			} else {
				info!(
					"Failed to read SEND_MESSAGE_BLOCK_HEIGHT either from ENV or config file. Defaulting to latest block height from db"
				);
				None
			};

			let table_name = if let Ok(value) = env::var("AVAIL_TABLE_NAME") {
				info!("AVAIL_TABLE_NAME from ENV");
				value
			} else if let Some(value) = config_file.avail_table_name {
				info!("AVAIL_TABLE_NAME from FILE");
				value
			} else {
				info!(
					"Failed to read AVAIL_TABLE_NAME either from ENV or config file. Defaulting to avail_send_message"
				);
				String::from("avail_send_message")
			};

			s.send_message = Some(TaskConfig {
				db_url: db_url.clone(),
				avail_url: avail_url.clone(),
				table_name,
				block_height,
			})
		}

		if run_execute {
			let block_height: Option<u32> = if let Ok(value) = env::var("EXECUTE_BLOCK_HEIGHT") {
				info!("EXECUTE_BLOCK_HEIGHT from ENV");
				Some(
					value
						.parse::<u32>()
						.map_err(|e| std::format!("Failed to parse EXECUTE_BLOCK_HEIGHT as u32. {}", e))?,
				)
			} else if let Some(value) = config_file.execute_block_height {
				info!("EXECUTE_BLOCK_HEIGHT from FILE");
				Some(value)
			} else {
				info!(
					"Failed to read EXECUTE_BLOCK_HEIGHT either from ENV or config file. Defaulting to latest block height from db"
				);
				None
			};

			let table_name = if let Ok(value) = env::var("ETH_TABLE_NAME") {
				info!("ETH_TABLE_NAME from ENV");
				value
			} else if let Some(value) = config_file.eth_table_name {
				info!("ETH_TABLE_NAME from FILE");
				value
			} else {
				info!("Failed to read ETH_TABLE_NAME either from ENV or config file. Defaulting to avail_execute");
				String::from("avail_execute")
			};

			s.execute = Some(TaskConfig {
				db_url: db_url.clone(),
				avail_url: avail_url.clone(),
				table_name,
				block_height,
			})
		}

		Ok(s)
	}
}
