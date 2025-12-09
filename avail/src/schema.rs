// use crate::Error;
// use avail_rust::{AccountId, BlockRef, H256, avail_rust_core::types::TxRef};
// use sqlx::{FromRow, PgConnection, Row, postgres::PgRow};
// use tabled::Tabled;

// pub trait BasicTableOperations<T> {
// 	const TABLE_NAME: &str;

// 	async fn table_exists(conn: &mut PgConnection) -> Result<bool, Error> {
// 		let q =
// 			std::format!("SELECT * FROM pg_catalog.pg_tables WHERE tablename = '{}';", Self::TABLE_NAME.to_lowercase());
// 		let result = sqlx::query(&q).fetch_all(conn).await?;
// 		Ok(!result.is_empty())
// 	}

// 	async fn table_drop(conn: &mut PgConnection) -> Result<(), Error> {
// 		let q = std::format!("DROP TABLE IF EXISTS {}", Self::TABLE_NAME.to_lowercase());
// 		sqlx::query(&q).execute(conn).await?;
// 		Ok(())
// 	}

// 	async fn table_list_entries(conn: &mut PgConnection) -> Result<(), Error>
// 	where
// 		T: for<'r> FromRow<'r, PgRow> + Tabled + Send + Unpin,
// 	{
// 		if !Self::table_exists(conn).await? {
// 			panic!("Table cars does not exist, dummy");
// 		}

// 		let q = std::format!("SELECT * FROM {};", Self::TABLE_NAME.to_lowercase());
// 		let result = sqlx::query_as::<_, T>(&q).fetch_all(conn).await?;
// 		Ok(())
// 	}
// }

// #[derive(Debug, Clone, Copy)]
// pub enum Status {
// 	InProgress,
// 	ClaimPending,
// 	Bridged,
// }

// impl Status {
// 	pub fn db_decode(&self) -> &str {
// 		match self {
// 			Status::InProgress => "IN_PROGRESS",
// 			Status::ClaimPending => "CLAIM_PENDING",
// 			Status::Bridged => "BRIDGED",
// 		}
// 	}
// }

// #[derive(Debug, Clone)]

// pub struct SendMessageEntry {
// 	// 4 High bytes are block height (u32)
// 	// 4 Low bytes are transaction index (u32)
// 	// Example:
// 	//	message_id: u64 = (block_height as u64) << 32 | transaction_index as u64
// 	//
// 	// In the DB this is stored as "i64" ¯\_(ツ)_/¯
// 	message_id: u64, // PG i64
// 	// It is actually an ENUM
// 	// - IN_PROGRESS
// 	// - CLAIM_PENDING
// 	// - BRIDGED
// 	//
// 	//  In the DB this is stored as PostgreSQL enum
// 	status: Status,
// 	// Transaction Hash.
// 	// To make it DB friendly we hex encode it.
// 	//
// 	// In the DB this is stored as VARCHAR(100)
// 	source_transaction_hash: H256,
// 	// Block Height at which the Tx was executed
// 	//
// 	// In the DB this is stored as "i64" ¯\_(ツ)_/¯
// 	source_block_number: u32,
// 	// Block Hash at which the Tx was executed.
// 	// To make it DB friendly we hex encode it
// 	//
// 	// In the DB this is stored as VARCHAR(100)
// 	source_block_hash: H256,
// 	// Transaction index.
// 	//
// 	// In the DB this is stored as "i64" ¯\_(ツ)_/¯
// 	source_transaction_index: u32,
// 	// This is Vector::SendMessage::FungibleToken::AssetId
// 	// AssetId is in H256 so to make it db friendly we hex encode it
// 	//
// 	// In the DB this is stored as VARCHAR(100)
// 	source_token_address: H256,
// 	// Not our problem :)
// 	//
// 	// In the DB we store this as NULL
// 	destination_transaction_hash: Option<H256>,
// 	// Not our problem :)
// 	//
// 	// In the DB we store this as NULL
// 	destination_block_number: Option<u32>,
// 	// Not our problem :)
// 	//
// 	// In the DB we store this as NULL
// 	destination_block_hash: Option<H256>,
// 	// Not our problem :)
// 	//
// 	// In the DB we store this as NULL
// 	destination_transaction_index: Option<u32>,
// 	// Not our problem :)
// 	//
// 	// In the DB we store this as NULL
// 	destination_token_address: Option<H256>,
// 	// Transaction signer address
// 	// To make it DB friendly we SS58 encode it.
// 	//
// 	// In the DB this is stored as VARCHAR(100)
// 	depositor_address: String,
// 	// This is Vector::SendMessage::FungibleToken::To
// 	// To is in H256 so to make it db friendly we hex encode it
// 	//
// 	// In the DB this is stored as VARCHAR(100)
// 	receiver_address: String,
// 	// This is Vector::SendMessage::FungibleToken::Amount
// 	// Amount is in u128 so to make it db friendly we stringify it
// 	//
// 	// In the DB this is stored as VARCHAR(300)
// 	amount: u128,
// }

// impl SendMessageEntry {
// 	pub fn new(block_ref: BlockRef, tx_ref: TxRef, asset_id: H256, amount: u128, to: H256, from: AccountId) -> Self {
// 		Self {
// 			message_id: (block_ref.height as u64) << 32 | tx_ref.index as u64,
// 			source_transaction_hash: std::format!("{:?}", tx_ref.hash),
// 			source_block_number: block_ref.height,
// 			source_block_hash: std::format!("{:?}", block_ref.hash),
// 			source_transaction_index: tx_ref.index,
// 			token_id: std::format!("{:?}", asset_id),
// 			depositor_address: from.to_string(),
// 			receiver_address: std::format!("{:?}", to),
// 			amount: std::format!("{}", amount),
// 		}
// 	}

// 	pub async fn table_create(conn: &mut PgConnection) -> Result<(), Error> {
// 		if Self::table_exists(conn).await? {
// 			panic!("Table already exist, dummy");
// 		}

// 		let q = std::format!(
// 			"
// 				CREATE TABLE {} (
// 					message_id BIGINT,
// 					source_transaction_hash VARCHAR(100),
// 					source_block_number BIGINT,
// 					source_block_hash VARCHAR(100),
// 					source_transaction_index BIGINT,
// 					token_id VARCHAR(100),
// 					depositor_address VARCHAR(100),
// 					receiver_address VARCHAR(100),
// 					amount VARCHAR(100)
// 				);
// 			",
// 			Self::TABLE_NAME
// 		);

// 		sqlx::query(&q).execute(conn).await?;
// 		Ok(())
// 	}

// 	pub async fn table_insert_entry(&self, conn: &mut PgConnection) -> Result<(), Error> {
// 		if !Self::table_exists(conn).await? {
// 			panic!("Table cars does not exist, dummy");
// 		}

// 		let q = std::format!(
// 			"
// 			INSERT INTO {} (message_id, source_transaction_hash, source_block_number, source_block_hash, source_transaction_index, token_id, depositor_address, receiver_address, amount)
// 			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
// 			Self::TABLE_NAME
// 		);
// 		sqlx::query(&q)
// 			.bind(self.message_id as i64)
// 			.bind(self.source_transaction_hash.as_str())
// 			.bind(self.source_block_number as i64)
// 			.bind(self.source_block_hash.as_str())
// 			.bind(self.source_transaction_index as i64)
// 			.bind(self.token_id.as_str())
// 			.bind(self.depositor_address.as_str())
// 			.bind(self.receiver_address.as_str())
// 			.bind(self.amount.as_str())
// 			.execute(conn)
// 			.await?;

// 		Ok(())
// 	}
// }

// impl BasicTableOperations<Self> for SendMessageEntry {
// 	const TABLE_NAME: &str = "AvailSendMessage";
// }

// impl<'r> FromRow<'r, PgRow> for SendMessageEntry {
// 	fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
// 		Ok(Self {
// 			message_id: row.try_get::<i64, _>("message_id")? as u64,
// 			source_transaction_hash: row.try_get("source_transaction_hash")?,
// 			source_block_number: row.try_get::<i64, _>("source_block_number")? as u32,
// 			source_block_hash: row.try_get("source_block_hash")?,
// 			source_transaction_index: row.try_get::<i64, _>("source_transaction_index")? as u32,
// 			token_id: row.try_get("token_id")?,
// 			depositor_address: row.try_get("depositor_address")?,
// 			receiver_address: row.try_get("receiver_address")?,
// 			amount: row.try_get("amount")?,
// 		})
// 	}
// }

// //
// // For Testing
// //

// pub async fn list_table_names(conn: &mut PgConnection) {
// 	let q = "SELECT * FROM pg_catalog.pg_tables;";
// 	let result = sqlx::query(q).fetch_all(conn).await.unwrap();
// 	for row in result {
// 		let name: &str = row.try_get("tablename").unwrap();
// 		println!("Name: {}", name);
// 	}
// }
