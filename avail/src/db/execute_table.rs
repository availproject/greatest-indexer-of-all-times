use avail_rust::H256;

use crate::db::Database;

pub struct ExecuteTable;
impl ExecuteTable {
	pub async fn create_table(db: &Database) -> Result<(), String> {
		let q = std::format!(
			"
				CREATE TABLE IF NOT EXISTS {} (
					id BIGINT PRIMARY KEY REFERENCES {},
					\"type\" TEXT NOT NULL,
					amount TEXT,
					\"to\" TEXT NOT NULL,
					slot BIGINT NOT NULL,
					message_id NUMERIC(78) NOT NULL
				);
			",
			&db.execute_table_name,
			&db.table_name
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
					\"to\",
					slot,
					message_id
				)
				VALUES ($1, $2, $3, $4, $5, $6)
				ON CONFLICT (id) DO UPDATE SET
					\"type\" = EXCLUDED.\"type\",
					amount = EXCLUDED.amount,
					\"to\" = EXCLUDED.\"to\",
					slot = EXCLUDED.slot,
					message_id = EXCLUDED.message_id
			",
			&db.execute_table_name
		);

		let amount = value.amount.map(|x| std::format!("{}", x));
		let _ = sqlx::query(&q)
			.bind(value.id as i64)
			.bind(value.kind)
			.bind(amount)
			.bind(std::format!("{:?}", value.to))
			.bind(value.slot as i64)
			.bind(value.message_id as i64)
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
	/// In the DB this is stored as "BIGINT NOT NULL"
	pub slot: u64,
	/// In the DB this is stored as "BIGINT NOT NULL"
	pub message_id: u64,
}
