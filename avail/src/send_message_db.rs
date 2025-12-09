use avail_rust::{AccountId, H256};
use sqlx::Row;
use sqlx::types::chrono::DateTime;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub struct Database {
	conn: Pool<Postgres>,
	table_name: String,
}

impl Database {
	pub async fn new(url: &str, table_name: String) -> Result<Self, String> {
		let conn = PgPoolOptions::new()
			.max_connections(5)
			.connect(&url)
			.await
			.map_err(|x| x.to_string())?;
		let s = Self { conn, table_name };

		Ok(s)
	}

	pub async fn create_table(&self) -> Result<(), String> {
		let q = std::format!(
			"
				CREATE TABLE IF NOT EXISTS {} (
					id BIGINT PRIMARY KEY,
					block_hash TEXT,
					block_height BIGINT,
					tx_hash TEXT,
					tx_index BIGINT,
					block_timestamp TIMESTAMPTZ,
					token_address TEXT,
					depositor_address TEXT,
					receiver_address TEXT,
					amount TEXT
				);
			",
			self.table_name
		);

		sqlx::query(&q).execute(&self.conn).await.map_err(|e| e.to_string())?;
		Ok(())
	}

	pub async fn find_highest_source_block_number(&self) -> Result<Option<u32>, String> {
		let q = std::format!("SELECT MAX(block_height) FROM {}", self.table_name);
		let row = sqlx::query(&q)
			.fetch_optional(&self.conn)
			.await
			.map_err(|e| e.to_string())?;

		let Some(row) = row else {
			return Ok(None);
		};

		let block_number = row
			.try_get::<Option<i64>, _>("max")
			.map_err(|e| std::format!("Failed to convert block_height. Error: {}", e.to_string()))?
			.map(|x| x as u32);

		Ok(block_number)
	}

	pub async fn row_exists(&self, message_id: u64) -> Result<bool, String> {
		let q = std::format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id={})", self.table_name, message_id as i64);
		let row = sqlx::query(&q).fetch_one(&self.conn).await.map_err(|e| e.to_string())?;

		let exists = row
			.try_get::<bool, _>("exists")
			.map_err(|e| std::format!("Failed to convert exists. Error: {}", e.to_string()))?;

		Ok(exists)
	}

	pub async fn store_send_message(&self, value: &SendMessageDb) -> Result<(), String> {
		let q = std::format!(
				"
					INSERT INTO {} (id, block_hash, block_height, tx_hash, tx_index, block_timestamp, token_address, depositor_address, receiver_address, amount)
					VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
					self.table_name
				);
		let _ = sqlx::query(&q)
			.bind(value.id as i64)
			.bind(std::format!("{:?}", value.block_hash))
			.bind(value.block_height as i64)
			.bind(std::format!("{:?}", value.tx_hash))
			.bind(value.tx_index as i64)
			.bind(DateTime::from_timestamp(value.block_timestamp as i64, 0))
			.bind(std::format!("{:?}", value.token_address))
			.bind(std::format!("{}", value.depositor_address))
			.bind(std::format!("{:?}", value.receiver_address))
			.bind(std::format!("{}", value.amount))
			.execute(&self.conn)
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}
}

pub struct SendMessageDb {
	/// 4 High bytes are block height (u32)
	/// 4 Low bytes are transaction index (u32)
	/// Example:
	///	id: u64 = (block_height as u64) << 32 | transaction_index as u64
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	pub id: u64,
	/// Block Hash.
	/// To make it DB friendly we hex encode it.
	///
	/// In the DB this is stored as TEXT
	block_hash: H256,
	/// Block Height at which the Tx was executed
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	block_height: u32,
	/// Transaction Hash at which the Tx was executed.
	/// To make it DB friendly we hex encode it
	///
	/// In the DB this is stored as TEXT
	tx_hash: H256,
	/// Transaction index.
	///
	/// In the DB this is stored as "i64" ¯\_(ツ)_/¯
	tx_index: u32,
	/// TODO
	///
	///
	///
	block_timestamp: u64,
	/// This is Vector::SendMessage::FungibleToken::AssetId
	/// AssetId is in H256 so to make it db friendly we hex encode it
	///
	/// In the DB this is stored as TEXT
	token_address: H256,
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
			id: (block_height as u64) << 32 | tx_index as u64,
			block_hash,
			block_height,
			tx_hash,
			tx_index,
			block_timestamp: timestamp,
			token_address,
			depositor_address,
			receiver_address,
			amount,
		}
	}
}
