mod configuration;
mod db;
mod indexer;

use std::time::Duration;

use tokio::runtime::Runtime;
use tracing::{error as terror, info};
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
	setup_tracing();

	// Load configuration
	// There is no point in retrying. We will get the same error back each time.
	let config = match configuration::Configuration::new() {
		Ok(x) => x,
		Err(err) => {
			terror!("Failed to load configuration. Existing program. Reason: {}", err);
			return;
		},
	};

	// Create runtime
	// There is no point in retrying. We will get the same error back each time.
	let runtime = match Runtime::new() {
		Ok(r) => r,
		Err(err) => {
			terror!("Failed to create runtime. Existing program. Reason: {}", err);
			return;
		},
	};

	runtime.block_on(async move {
		let t1 = tokio::spawn(async { indexer::run_indexer(config).await });
		let t2 = tokio::spawn(async { i_am_alive().await });

		_ = t1.await;
		_ = t2.await;
	});
}

fn setup_tracing() {
	let builder = tracing_subscriber::fmt::SubscriberBuilder::default();
	_ = builder.json().finish().try_init();
}

async fn i_am_alive() {
	loop {
		tokio::time::sleep(Duration::from_hours(1)).await;
		info!("❤️  Indexer is still alive",);
	}
}
