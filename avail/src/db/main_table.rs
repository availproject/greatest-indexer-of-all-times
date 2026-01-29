use crate::db::Database;
use avail_rust::{H256, block::BlockEncodedExtrinsic};
use sqlx::{
	Row,
	types::chrono::{DateTime, Utc},
};

pub struct MainTable;
impl MainTable {
	pub async fn create_table(db: &Database) -> Result<(), String> {
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
			db.main_table_name
		);

		sqlx::query(&q).execute(&db.conn).await.map_err(|e| e.to_string())?;
		Ok(())
	}

	pub async fn find_highest_block_height(db: &Database) -> Result<Option<u32>, String> {
		let q = std::format!("SELECT MAX(block_height) FROM {}", db.main_table_name);
		let row = sqlx::query(&q)
			.fetch_optional(&db.conn)
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

	pub async fn insert(value: TableEntry, db: &Database) -> Result<(), String> {
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
			db.main_table_name
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
			.execute(&db.conn)
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}
}

pub struct TableEntry {
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

impl TableEntry {
	pub fn from_block_ext(
		block_height: u32,
		block_hash: H256,
		block_timestamp: u64,
		ext: &BlockEncodedExtrinsic,
	) -> Self {
		Self {
			id: (block_height as u64) << 32 | ext.metadata.ext_index as u64,
			block_height,
			block_hash,
			block_timestamp,
			ext_index: ext.metadata.ext_index,
			ext_hash: ext.metadata.ext_hash,
			signature_address: ext.ss58_address(),
			pallet_id: ext.metadata.pallet_id,
			variant_id: ext.metadata.variant_id,
			ext_success: None,
			ext_call: String::new(),
		}
	}
}
