use avail_rust::{AccountId, H256};
use sqlx::Row;
use sqlx::types::chrono::DateTime;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub struct Database {
	conn: Pool<Postgres>,
	avail_table_name: String,
	eth_table_name: String,
}

impl Database {
	pub async fn new(url: &str, avail_table_name: String, eth_table_name: String) -> Result<Self, String> {
		let conn = PgPoolOptions::new()
			.max_connections(5)
			.connect(&url)
			.await
			.map_err(|x| x.to_string())?;
		let s = Self { conn, avail_table_name, eth_table_name };

		Ok(s)
	}

	pub async fn create_table(&self) -> Result<(), String> {
		let q = std::format!(
			"
				CREATE TABLE  IF NOT EXISTS {} (
					message_id BIGINT PRIMARY KEY,
					status TEXT,
					source_transaction_hash TEXT,
					source_block_number BIGINT,
					source_block_hash TEXT,
					source_transaction_index BIGINT,
					source_timestamp TIMESTAMPTZ,
					source_token_address TEXT,
					destination_transaction_hash TEXT,
					destination_block_number BIGINT,
					destination_block_hash TEXT,
					destination_transaction_index BIGINT,
					destination_timestamp TIMESTAMPTZ,
					destination_token_address TEXT,
					depositor_address TEXT,
					receiver_address TEXT,
					amount TEXT
				);
			",
			self.avail_table_name
		);

		sqlx::query(&q).execute(&self.conn).await.map_err(|e| e.to_string());
		Ok(())
	}

	pub async fn find_highest_source_block_number(&self) -> Result<Option<u32>, String> {
		let q = std::format!("SELECT MAX(source_block_number) FROM {}", self.avail_table_name);
		let row = sqlx::query(&q)
			.fetch_optional(&self.conn)
			.await
			.map_err(|e| e.to_string())?;

		let Some(row) = row else {
			return Ok(None);
		};

		let block_number = row
			.try_get::<i64, _>("max")
			.map_err(|e| std::format!("Failed to convert source_block_number. Error: {}", e.to_string()))?;

		Ok(Some(block_number as u32))
	}

	pub async fn store_send_message(&self, value: &SendMessageDb) -> Result<(), String> {
		let q = std::format!(
					"
					INSERT INTO {} (message_id, status, source_transaction_hash, source_block_number, source_block_hash, source_transaction_index, source_timestamp, source_token_address, depositor_address, receiver_address, amount)
					VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
					self.avail_table_name
				);
		let res = sqlx::query(&q)
			.bind(value.message_id as i64)
			.bind(value.status.as_str())
			.bind(std::format!("{:?}", value.source_transaction_hash))
			.bind(value.source_block_number as i64)
			.bind(std::format!("{:?}", value.source_block_hash))
			.bind(value.source_transaction_index as i64)
			.bind(DateTime::from_timestamp(value.source_timestamp as i64, 0))
			.bind(std::format!("{:?}", value.source_token_address))
			.bind(std::format!("{}", value.depositor_address))
			.bind(std::format!("{:?}", value.receiver_address))
			.bind(std::format!("{}", value.amount))
			.execute(&self.conn)
			.await
			.map_err(|e| e.to_string());
		res.unwrap();

		Ok(())
	}
}

pub struct SendMessageDb {
	/// 4 High bytes are block height (u32)
	/// 4 Low bytes are transaction index (u32)
	/// Example:
	///	message_id: u64 = (block_height as u64) << 32 | transaction_index as u64
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	message_id: u64,
	/// PG i64
	/// It is actually an ENUM
	/// - IN_PROGRESS
	/// - CLAIM_PENDING
	/// - BRIDGED
	status: String,
	/// Transaction Hash.
	/// To make it DB friendly we hex encode it.
	///
	/// In the DB this is stored as TEXT
	source_transaction_hash: H256,
	/// Block Height at which the Tx was executed
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	source_block_number: u32,
	/// Block Hash at which the Tx was executed.
	/// To make it DB friendly we hex encode it
	///
	/// In the DB this is stored as TEXT
	source_block_hash: H256,
	/// Transaction index.
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	source_transaction_index: u32,
	/// TODO
	///
	///
	///
	source_timestamp: u64,
	/// This is Vector::SendMessage::FungibleToken::AssetId
	/// AssetId is in H256 so to make it db friendly we hex encode it
	///
	/// In the DB this is stored as TEXT
	source_token_address: H256,
	/// Not our problem :)
	///
	/// In the DB we store this as TEXT
	destination_transaction_hash: Option<H256>,
	/// Not our problem :)
	///
	/// In the DB we store this as i64
	destination_block_number: Option<u32>,
	/// Not our problem :)
	///
	/// In the DB we store this as TEXT
	destination_block_hash: Option<H256>,
	/// Not our problem :)
	///
	/// In the DB we store this as i64
	destination_transaction_index: Option<u32>,
	/// Not our problem :)
	///
	/// In the DB we store this as Timestamp
	destination_timestamp: Option<u64>,
	/// Not our problem :)
	///
	/// In the DB we store this as TEXT
	destination_token_address: Option<H256>,
	/// Transaction signer address
	/// To make it DB friendly we SS58 encode it.
	///
	/// In the DB this is stored as TEXT
	depositor_address: AccountId,
	/// This is Vector::SendMessage::FungibleToken::To
	/// To is in H256 so to make it db friendly we hex encode it
	///
	/// In the DB this is stored as TEXT
	receiver_address: H256,
	/// This is Vector::SendMessage::FungibleToken::Amount
	/// Amount is in u128 so to make it db friendly we stringify it
	///
	/// In the DB this is stored as TEXT
	amount: u128,
}

impl SendMessageDb {
	pub fn new(
		block_height: u32,
		tx_index: u32,
		block_hash: H256,
		tx_hash: H256,
		timestamp: u64,
		token_address: H256,
		depositor_address: AccountId,
		receiver_address: H256,
		amount: u128,
	) -> Self {
		Self {
			message_id: (block_height as u64) << 32 | tx_index as u64,
			status: String::from("IN_PROGRESS"),
			source_transaction_hash: tx_hash,
			source_block_number: block_height,
			source_block_hash: block_hash,
			source_transaction_index: tx_index,
			source_timestamp: timestamp,
			source_token_address: token_address,
			destination_transaction_hash: None,
			destination_block_number: None,
			destination_block_hash: None,
			destination_transaction_index: None,
			destination_timestamp: None,
			destination_token_address: None,
			depositor_address,
			receiver_address,
			amount,
		}
	}
}
