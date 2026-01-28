use std::time::Instant;
use tracing::info;

const DISPLAY_MESSAGE_INTERVAL_SECS: u64 = 60;

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

	pub fn maybe_display_stats(&mut self, last_indexed_height: u32, final_height: u32) {
		if !(self.checkpoint.elapsed().as_secs() > DISPLAY_MESSAGE_INTERVAL_SECS) {
			return;
		}

		let bps = self.bps();
		self.checkpoint = Instant::now();
		self.previously_indexed = self.total_indexed;

		let blocks_left_to_index = final_height.saturating_add(1).saturating_sub(last_indexed_height);
		info!(
			last_indexed_height,
			final_height,
			total_indexed = self.total_indexed,
			blocks_left_to_index,
			bps,
			"Indexing..."
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
