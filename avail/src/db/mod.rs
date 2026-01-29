pub mod execute_table;
pub mod main_table;
pub mod send_message_table;

use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub struct Database {
	pub conn: Pool<Postgres>,
	pub main_table_name: String,
	pub send_message_table_name: String,
	pub execute_table_name: String,
}

impl Database {
	pub async fn new(
		url: &str,
		main_table_name: String,
		send_message_table_name: String,
		execute_table_name: String,
	) -> Result<Self, String> {
		let conn = PgPoolOptions::new()
			.max_connections(5)
			.connect(&url)
			.await
			.map_err(|x| x.to_string())?;

		let db = Self {
			conn,
			main_table_name,
			send_message_table_name,
			execute_table_name,
		};

		main_table::MainTable::create_table(&db).await?;
		execute_table::ExecuteTable::create_table(&db).await?;
		send_message_table::SendMessageTable::create_table(&db).await?;

		Ok(db)
	}

	pub async fn insert(&self, data: DataForDatabase) -> Result<(), String> {
		for entry in data.main_entries {
			main_table::MainTable::insert(entry, self).await?;
		}

		for entry in data.execute_entries {
			execute_table::ExecuteTable::insert(entry, self).await?;
		}

		for entry in data.send_message_entries {
			send_message_table::SendMessageTable::insert(entry, self).await?;
		}

		Ok(())
	}

	pub async fn find_highest_block_height(&self) -> Result<Option<u32>, String> {
		main_table::MainTable::find_highest_block_height(self).await
	}
}

#[derive(Default)]
pub struct DataForDatabase {
	pub main_entries: Vec<main_table::TableEntry>,
	pub execute_entries: Vec<execute_table::TableEntry>,
	pub send_message_entries: Vec<send_message_table::TableEntry>,
}
