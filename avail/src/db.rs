use sqlx::Row;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub struct Database {
	conn: Pool<Postgres>,
}

impl Database {
	const TABLE_NAME: &str = "availsendmessage";

	pub async fn new(url: &str) -> Result<Self, String> {
		let conn = PgPoolOptions::new()
			.max_connections(5)
			.connect(&url)
			.await
			.map_err(|x| x.to_string())?;
		let s = Self { conn };

		Ok(s)
	}

	pub async fn find_highest_source_block_number(&self) -> Result<Option<u32>, String> {
		let q = std::format!("SELECT MAX(source_block_number) FROM {}", Self::TABLE_NAME);
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
}
