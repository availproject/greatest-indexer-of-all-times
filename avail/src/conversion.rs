use avail_rust::{BlockRawExtrinsic, BlockSignedExtrinsic, avail::vector::tx::SendMessage};

pub fn from_raw_to_send_message(ext: BlockRawExtrinsic) -> Result<BlockSignedExtrinsic<SendMessage>, String> {
	let send_message = BlockSignedExtrinsic::<SendMessage>::try_from(ext)?;
	Ok(send_message)
}
