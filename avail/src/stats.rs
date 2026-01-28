use std::time::Instant;
use tracing::info;

const DISPLAY_MESSAGE_INTERVAL_SECS: u64 = 5;

pub struct IndexerStats {
	pub total_indexed: u32,
	pub previously_indexed: u32,
	pub checkpoint: Instant,
}

impl IndexerStats {
	pub fn new() -> Self {
		Self {
			total_indexed: 0,
			previously_indexed: 0,
			checkpoint: Instant::now(),
		}
	}

	pub fn maybe_display_stats(&mut self, last_indexed_block: u32, finalized_block: u32, remaining_block_count: u32) {
		if !(self.checkpoint.elapsed().as_secs() > DISPLAY_MESSAGE_INTERVAL_SECS) {
			return;
		}

		let bps = self.bps();
		let block_indexed_since_last_log_count = self.total_indexed - self.previously_indexed;
		let block_indexed_count = self.total_indexed;
		self.checkpoint = Instant::now();
		self.previously_indexed = self.total_indexed;

		info!(
			last_indexed_block,
			remaining_block_count,
			block_indexed_count,
			finalized_block,
			block_indexed_since_last_log_count,
			bps,
			"📊 Indexing Stats"
		);
	}

	pub fn bps(&self) -> f32 {
		let elapsed = self.checkpoint.elapsed();
		let diff = self.total_indexed.saturating_sub(self.previously_indexed);
		let elapsed = elapsed.as_millis() as f32;
		if elapsed > 0f32 {
			((diff * 1000) as f32) / elapsed
		} else {
			0f32
		}
	}
}
