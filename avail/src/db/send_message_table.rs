use avail_rust::H256;

use crate::{common::SerializedSendMessage, db::Database};

pub struct SendMessageTable;
impl SendMessageTable {
	pub async fn create_table(db: &Database) -> Result<(), String> {
		let q = std::format!(
			"
				CREATE TABLE IF NOT EXISTS {} (
					id BIGINT PRIMARY KEY REFERENCES {},
					\"type\" TEXT NOT NULL,
					amount TEXT,
					\"to\" TEXT NOT NULL
				);
			",
			&db.send_message_table_name,
			&db.main_table_name
		);

		sqlx::query(&q).execute(&db.conn).await.map_err(|e| e.to_string())?;
		Ok(())
	}

	pub async fn insert(value: TableEntry, db: &Database) -> Result<(), String> {
		let q = std::format!(
			"
				INSERT INTO {} (
					id,
					\"type\",
					amount,
					\"to\"
				)
				VALUES ($1, $2, $3, $4)
				ON CONFLICT (id) DO UPDATE SET
					\"type\" = EXCLUDED.\"type\",
					amount = EXCLUDED.amount,
					\"to\" = EXCLUDED.\"to\"
			",
			&db.send_message_table_name
		);
		let amount = value.amount.map(|x| std::format!("{}", x));
		let _ = sqlx::query(&q)
			.bind(value.id as i64)
			.bind(value.kind)
			.bind(amount)
			.bind(std::format!("{:?}", value.to))
			.execute(&db.conn)
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}
}

pub struct TableEntry {
	/// In the DB this is stored as "BIGINT PRIMARY KEY"
	pub id: u64,
	/// In the DB this is stored as "TEXT NOT NULL"
	pub kind: String,
	/// In the DB this is stored as "TEXT"
	pub amount: Option<u128>,
	/// In the DB this is stored as "TEXT NOT NULL"
	pub to: H256,
}

impl TableEntry {
	pub fn from_call(id: u64, call: &SerializedSendMessage) -> Self {
		Self {
			id,
			kind: call.message.kind().to_string(),
			amount: call.message.amount(),
			to: call.to,
		}
	}
}
