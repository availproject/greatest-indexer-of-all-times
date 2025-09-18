#[derive(Debug, Clone)]
pub struct DatabaseEntry {
	// Concat Block Height (u32) and Transaction Index (u32)
	message_id: u64,
	// It is actually an ENUM
	// - IN_PROGRESS
	// - CLAIM_PENDING
	// - BRIDGED
	status: String,
	// Transaction Hash H256 -> String
	source_transaction_hash: String,
	// Block Height at which the Tx was executed
	source_block_number: i64,
	// Block Hash at which the Tx was executed
	source_block_hash: String,
	// Transaction index u32 -> i64
	source_transaction_index: i64,
	//
	// missing some
	//
	// this is asset_id
	token_id: String,
	//
	// missing some
	//
	depositor_address: String,
	receiver_address: String,
	amount: String,
	// This is an enum
	claim_type: String,
}
