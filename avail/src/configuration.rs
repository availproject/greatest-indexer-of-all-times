use std::{env, num::ParseIntError};

#[derive(Debug, Default, serde::Deserialize, Clone)]
pub struct ConfigurationFile {
	pub db_url: Option<String>,
	pub avail_url: Option<String>,
	pub table_name: Option<String>,
	pub send_message_table_name: Option<String>,
	pub execute_table_name: Option<String>,
	pub block_height: Option<u32>,
	pub max_task_count: Option<u32>,
	pub observability: Option<Observability>,
	pub log_interval_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Configuration {
	pub db_url: String,
	pub avail_url: String,
	pub table_name: String,
	pub send_message_table_name: String,
	pub execute_table_name: String,
	pub block_height: Option<u32>,
	pub max_task_count: u32,
	pub observability: Observability,
	pub log_interval_ms: u32,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct Observability {
	pub traces_endpoint: Option<String>,  // If None then no traces will be send
	pub metrics_endpoint: Option<String>, // If None then no metrics will be send
	pub logs_endpoint: Option<String>,    // If None then no logs will be send
	pub json_format: Option<bool>,        // The default is true
	pub log_to_file_path: Option<String>, // If None then logs won't be stored to a file
	pub metric_export_interval: Option<String>,
	pub service_name: Option<String>,    // Default is env!("CARGO_CRATE_NAME");
	pub service_version: Option<String>, // Default is env!("CARGO_PKG_VERSION");
}

impl Configuration {
	pub fn new() -> Result<Self, String> {
		// First check if there is a file that we need to read
		let config_file = if let Ok(file_path) = env::var("CONFIG") {
			println!("ENV CONFIG was set to: {}", file_path);

			// Read file
			let config_file = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
			let config_file: ConfigurationFile = serde_json::from_str(&config_file).map_err(|e| e.to_string())?;
			config_file
		} else {
			println!("ENV CONFIG was not set. Not reading any config file");
			Default::default()
		};

		let db_url = if let Ok(value) = env::var("DB_URL") {
			println!("DB_URL: ENV");
			value
		} else if let Some(value) = config_file.db_url {
			println!("DB_URL: FILE");
			value
		} else {
			return Err("Failed to read DB_URL either from ENV or config file.".into());
		};

		let avail_url = if let Ok(value) = env::var("AVAIL_URL") {
			println!("AVAIL_URL: ENV");
			value
		} else if let Some(value) = config_file.avail_url {
			println!("AVAIL_URL: FILE");
			value
		} else {
			println!("AVAIL_URL: DEFAULT");
			avail_rust::prelude::MAINNET_ENDPOINT.to_owned()
		};
		println!("AVAIL_URL: {}", avail_url);

		let block_height: Option<u32> = if let Ok(value) = env::var("BLOCK_HEIGHT") {
			println!("BLOCK_HEIGHT: ENV");
			Some(
				value
					.parse::<u32>()
					.map_err(|e| std::format!("Failed to parse BLOCK_HEIGHT as u32. {}", e))?,
			)
		} else if let Some(value) = config_file.block_height {
			println!("BLOCK_HEIGHT: FILE");
			Some(value)
		} else {
			println!(
				"Failed to read BLOCK_HEIGHT either from ENV or config file. Defaulting to latest block height from db"
			);
			None
		};
		println!("BLOCK_HEIGHT: {:?}", block_height);

		let table_name = if let Ok(value) = env::var("TABLE_NAME") {
			println!("MAIN_TABLE_NAME: ENV");
			value
		} else if let Some(value) = config_file.table_name {
			println!("MAIN_TABLE_NAME: FILE");
			value
		} else {
			println!("MAIN_TABLE_NAME: DEFAULT");
			String::from("avail_table")
		};
		println!("MAIN_TABLE_NAME: {:?}", table_name);

		let send_message_table_name = if let Ok(value) = env::var("SEND_MESSAGE_TABLE_NAME") {
			println!("SEND_MESSAGE_TABLE_NAME: ENV");
			value
		} else if let Some(value) = config_file.send_message_table_name {
			println!("SEND_MESSAGE_TABLE_NAME: FILE");
			value
		} else {
			println!("SEND_MESSAGE_TABLE_NAME: DEFAULT");
			String::from("avail_send_message_table")
		};
		println!("SEND_MESSAGE_TABLE_NAME: {:?}", send_message_table_name);

		let execute_table_name = if let Ok(value) = env::var("EXECUTE_TABLE_NAME") {
			println!("EXECUTE_TABLE_NAME: ENV");
			value
		} else if let Some(value) = config_file.execute_table_name {
			println!("EXECUTE_TABLE_NAME: FILE");
			value
		} else {
			println!("EXECUTE_TABLE_NAME: DEFAULT");
			String::from("avail_execute_table")
		};
		println!("EXECUTE_TABLE_NAME: {:?}", execute_table_name);

		let max_task_count: u32 = if let Ok(value) = env::var("MAX_TASK_COUNT") {
			println!("MAX_TASK_COUNT: ENV");
			value.parse().map_err(|e: ParseIntError| e.to_string())?
		} else if let Some(value) = config_file.max_task_count {
			println!("MAX_TASK_COUNT: FILE");
			value
		} else {
			println!("MAX_TASK_COUNT: DEFAULT");
			25
		};
		println!("MAX_TASK_COUNT: {:?}", max_task_count);

		let mut observability = config_file.observability.unwrap_or_default();
		if let Ok(endpoint) = env::var("TRACES_ENDPOINT") {
			observability.traces_endpoint = Some(endpoint);
		}
		if let Ok(endpoint) = env::var("METRICS_ENDPOINT") {
			observability.metrics_endpoint = Some(endpoint);
		}
		if let Ok(endpoint) = env::var("LOGS_ENDPOINT") {
			observability.logs_endpoint = Some(endpoint);
		}
		if let Ok(name) = env::var("SERVICE_NAME") {
			observability.service_name = Some(name);
		}
		if let Ok(version) = env::var("SERVICE_VERSION") {
			observability.service_version = Some(version);
		}
		if let Ok(path) = env::var("LOG_TO_FILE_PATH") {
			observability.log_to_file_path = Some(path);
		}

		let log_interval_ms: u32 = if let Ok(value) = env::var("LOG_INTERVAL_MS") {
			println!("LOG_INTERVAL_MS: ENV");
			value.parse().map_err(|e: ParseIntError| e.to_string())?
		} else if let Some(value) = config_file.log_interval_ms {
			println!("LOG_INTERVAL_MS: FILE");
			value
		} else {
			println!("LOG_INTERVAL_MS: DEFAULT");
			60_000
		};
		println!("LOG_INTERVAL_MS: {:?}", log_interval_ms);

		Ok(Configuration {
			db_url,
			avail_url,
			table_name,
			block_height,
			send_message_table_name,
			execute_table_name,
			max_task_count,
			observability,
			log_interval_ms,
		})
	}
}
