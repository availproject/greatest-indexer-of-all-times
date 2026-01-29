mod common;
mod configuration;
mod db;
mod indexer;
mod stats;

use crate::{configuration::Observability, indexer::Indexer};
use internal_utils::{TracingBuilder, TracingGuards, TracingOtelParams};
use tokio::runtime::Runtime;
use tracing::{error as terror, info};

const SERVICE_NAME: &'static str = env!("CARGO_CRATE_NAME");
const SERVICE_VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
	// Load configuration
	// There is no point in retrying. We will get the same error back each time.
	let config = configuration::Configuration::new().expect("Configuration file should not be malformed");
	setup_observability(&config.observability).expect("Observability should not fail");

	let obs = &config.observability;
	let service_name = obs.service_name.clone().unwrap_or_else(|| SERVICE_NAME.into());
	let service_version = obs.service_version.clone().unwrap_or_else(|| SERVICE_VERSION.into());
	info!(
		traces_endpoint = ?obs.traces_endpoint,
		metrics_endpoint = ?obs.metrics_endpoint,
		logs_endpoint = ?obs.logs_endpoint,
		service_name = service_name,
		service_version = service_version,
		avail_url = config.avail_url,
		main_table_name = config.table_name,
		send_message_table_name = config.send_message_table_name,
		execute_table_name = config.execute_table_name,
		block_height = ?config.block_height,
		max_task_count = config.max_task_count,
		log_interval_ms = config.log_interval_ms,
	);

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
		let t1 = tokio::spawn(async {
			let indexer = Indexer::new(config).await?;
			indexer.run().await
		});

		match t1.await {
			Err(err) => terror!(error = err.to_string(), "Indexer returned an error. Indexer shutting down"),
			_ => (),
		}
	});
}

pub fn setup_observability(config: &Observability) -> Result<TracingGuards, Box<dyn std::error::Error + Send + Sync>> {
	// Setting up tracing + otel
	let service_name = config.service_name.clone().unwrap_or_else(|| SERVICE_NAME.into());
	let service_version = config.service_version.clone().unwrap_or_else(|| SERVICE_VERSION.into());

	let env_filter = tracing_subscriber::EnvFilter::from_default_env()
		.add_directive(std::format!("{}=info", service_name).parse()?)
		.add_directive(tracing::Level::WARN.into());

	let otel = TracingOtelParams {
		endpoint_traces: config.traces_endpoint.clone(),
		endpoint_metrics: config.metrics_endpoint.clone(),
		endpoint_logs: config.logs_endpoint.clone(),
		service_name,
		service_version,
	};
	let path = config.log_to_file_path.clone();
	let mut builder = TracingBuilder::new()
		.with_json(Some(config.json_format.unwrap_or(true)))
		.with_env_filter(Some(env_filter))
		.with_file(path)
		.with_otel(otel);
	if let Some(value) = &config.metric_export_interval {
		builder = builder.with_otel_metric_export_interval(value);
	}

	builder.try_init()
}
