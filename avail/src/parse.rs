use avail_rust::{
	BlockRawExtrinsic, ExtrinsicCall, H256, HasHeader, MultiAddress, RawExtrinsic,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		vector::tx::{Execute, SendMessage},
	},
	codec::Decode,
};

pub struct Target {
	pub address: MultiAddress,
	pub ext_hash: H256,
	pub ext_index: u32,
	pub call: SendMsgOrExecute,
}

impl Target {
	pub fn new(address: MultiAddress, ext_hash: H256, ext_index: u32, call: SendMsgOrExecute) -> Self {
		Self { address, ext_hash, ext_index, call }
	}
}

#[derive(Debug)]
pub enum SendMsgOrExecute {
	Send(SendMessage),
	Execute(Execute),
}

impl From<SendMessage> for SendMsgOrExecute {
	fn from(value: SendMessage) -> Self {
		Self::Send(value)
	}
}

impl From<Execute> for SendMsgOrExecute {
	fn from(value: Execute) -> Self {
		Self::Execute(value)
	}
}

pub fn parse_transactions(list: &Vec<BlockRawExtrinsic>) -> Result<Vec<Target>, String> {
	let mut targets: Vec<Target> = Vec::with_capacity(list.len());
	for tx in list {
		let Some(raw_ext) = &tx.data else {
			return Err("Failed to fetch transaction with data. This is not good.".into());
		};
		let metadata = tx.metadata.clone();

		let raw_ext = RawExtrinsic::try_from(raw_ext.as_str())?;
		let Some(signature) = raw_ext.signature else {
			return Err("Extrinsic did not had signature. This is not good".into());
		};

		let call = ExtrinsicCall::try_from(&raw_ext.call)?;
		let Some(call) = parse_extrinsic_call(&call)? else {
			continue;
		};
		let target = Target::new(signature.address, metadata.ext_hash, metadata.ext_index, call);

		targets.push(target);
	}

	Ok(targets)
}

fn parse_extrinsic_call(call: &ExtrinsicCall) -> Result<Option<SendMsgOrExecute>, String> {
	let header = (call.pallet_id, call.variant_id);

	if header == SendMessage::HEADER_INDEX {
		return Ok(Some(parse_send_message_call(&call.data)?.into()));
	}

	if header == Execute::HEADER_INDEX {
		return Ok(Some(parse_execute_call(&call.data)?.into()));
	}

	if header == AsMulti::HEADER_INDEX {
		return parse_multisig_call(&call.data);
	}

	if header == Proxy::HEADER_INDEX {
		return parse_proxy_call(&call.data);
	}

	Ok(None)
}

fn parse_send_message_call(mut call_data: &[u8]) -> Result<SendMessage, String> {
	SendMessage::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_execute_call(mut call_data: &[u8]) -> Result<Execute, String> {
	Execute::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_multisig_call(mut call_data: &[u8]) -> Result<Option<SendMsgOrExecute>, String> {
	let multi = match AsMulti::decode(&mut call_data) {
		Ok(x) => x,
		Err(err) => {
			tracing::warn!(
				"Failed to convert raw extrinsic to multisig. That is OK as this multisig is probably not the one that we need. Err: {}",
				err
			);
			return Ok(None);
		},
	};

	parse_extrinsic_call(&multi.call)
}

fn parse_proxy_call(mut call_data: &[u8]) -> Result<Option<SendMsgOrExecute>, String> {
	let proxy = Proxy::decode(&mut call_data)
		.map_err(|e| std::format!("Failed to convert raw ext to Proxy::Proxy. Err: {}", e))?;

	parse_extrinsic_call(&proxy.call)
}
