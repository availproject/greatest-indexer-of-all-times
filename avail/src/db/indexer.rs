use avail_rust::H256;
use sqlx::Row;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub struct Database {
	pub conn: Pool<Postgres>,
	pub table_name: String,
	pub send_message_table_name: String,
	pub execute_table_name: String,
}

impl Database {
	pub async fn new(
		url: &str,
		table_name: String,
		send_message_table_name: String,
		execute_table_name: String,
	) -> Result<Self, String> {
		let conn = PgPoolOptions::new()
			.max_connections(5)
			.connect(&url)
			.await
			.map_err(|x| x.to_string())?;
		let s = Self {
			conn,
			table_name,
			send_message_table_name,
			execute_table_name,
		};

		Ok(s)
	}

	pub async fn create_table(&self) -> Result<(), String> {
		let q = std::format!(
			"
				CREATE TABLE IF NOT EXISTS {} (
					id BIGINT PRIMARY KEY,
					block_height INTEGER NOT NULL,
					block_hash TEXT NOT NULL,
					block_timestamp TIMESTAMPTZ NOT NULL,
					ext_index INTEGER NOT NULL,
					ext_hash TEXT NOT NULL,
					signature_address TEXT,
					pallet_id SMALLINT NOT NULL,
					variant_id SMALLINT NOT NULL,
					ext_success BOOL,
					ext_call TEXT NOT NULL
				);
			",
			self.table_name
		);

		sqlx::query(&q).execute(&self.conn).await.map_err(|e| e.to_string())?;
		Ok(())
	}

	pub async fn find_highest_block_height(&self) -> Result<Option<u32>, String> {
		let q = std::format!("SELECT MAX(block_height) FROM {}", self.table_name);
		let row = sqlx::query(&q)
			.fetch_optional(&self.conn)
			.await
			.map_err(|e| e.to_string())?;

		let Some(row) = row else {
			return Ok(None);
		};

		let block_height = row
			.try_get::<Option<i64>, _>("max")
			.map_err(|e| std::format!("Failed to convert block_height. Error: {}", e.to_string()))?
			.map(|x| x as u32);

		Ok(block_height)
	}

	pub async fn row_exists(&self, id: u64) -> Result<bool, String> {
		let q = std::format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id={})", self.table_name, id as i64);
		let row = sqlx::query(&q).fetch_one(&self.conn).await.map_err(|e| e.to_string())?;

		let exists = row
			.try_get::<bool, _>("exists")
			.map_err(|e| std::format!("Failed to convert exists. Error: {}", e.to_string()))?;

		Ok(exists)
	}

	pub async fn insert(&self, value: DbEntry) -> Result<(), String> {
		let q = std::format!(
			"
				INSERT INTO {} (
					id,
					block_height,
					block_hash,
					block_timestamp,
					ext_index,
					ext_hash,
					signature_address,
					pallet_id,
					variant_id,
					ext_success,
					ext_call
				)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
				ON CONFLICT (id) DO UPDATE SET
					block_height = EXCLUDED.block_height,
					block_hash = EXCLUDED.block_hash,
					block_timestamp = EXCLUDED.block_timestamp,
					ext_index = EXCLUDED.ext_index,
					ext_hash = EXCLUDED.ext_hash,
					signature_address = EXCLUDED.signature_address,
					pallet_id = EXCLUDED.pallet_id,
					variant_id = EXCLUDED.variant_id,
					ext_success = EXCLUDED.ext_success,
					ext_call = EXCLUDED.ext_call
			",
			self.table_name
		);
		let block_timestamp = DateTime::<Utc>::from_timestamp(value.block_timestamp as i64, 0)
			.ok_or_else(|| "Failed to convert block_timestamp to chrono DateTime".to_string())?;
		let _ = sqlx::query(&q)
			.bind(value.id as i64)
			.bind(value.block_height as i32)
			.bind(std::format!("{:?}", value.block_hash))
			.bind(block_timestamp)
			.bind(value.ext_index as i32)
			.bind(std::format!("{:?}", value.ext_hash))
			.bind(value.signature_address)
			.bind(value.pallet_id as i16)
			.bind(value.variant_id as i16)
			.bind(value.ext_success)
			.bind(value.ext_call)
			.execute(&self.conn)
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}
}

pub struct DbEntry {
	/// In the DB this is stored as "BIGINT PRIMARY KEY"
	pub id: u64,
	/// In the DB this is stored as "INTEGER NOT NULL"
	pub block_height: u32,
	/// In the DB this is stored as "TEXT NOT NULL"
	pub block_hash: H256,
	/// In the DB this is stored as "TIMESTAMPTZ NOT NULL"
	pub block_timestamp: u64,
	/// In the DB this is stored as "INTEGER NOT NULL"
	pub ext_index: u32,
	/// In the DB this is stored as "TEXT NOT NULL"
	pub ext_hash: H256,
	// ss58 address
	/// In the DB this is stored as "nullable TEXT"
	pub signature_address: Option<String>,
	/// In the DB this is stored as "SMALLINT NOT NULL"
	pub pallet_id: u8,
	/// In the DB this is stored as "SMALLINT NOT NULL"
	pub variant_id: u8,
	/// In the DB this is stored as "nullable BOOL"
	pub ext_success: Option<bool>,
	// Call data JSON encoded
	/// In the DB this is stored as "TEXT NOT NULL"
	pub ext_call: String,
}
