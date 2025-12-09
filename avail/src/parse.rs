use avail_rust::{
	ExtrinsicCall, H256, HasHeader, MultiAddress,
	avail::{
		multisig::tx::AsMulti,
		proxy::tx::Proxy,
		vector::{
			tx::{Execute, SendMessage},
			types::Message,
		},
	},
	block::BlockEncodedExtrinsic,
	codec::Decode,
};

pub struct Target {
	pub address: MultiAddress,
	pub ext_hash: H256,
	pub ext_index: u32,
	pub call: SendMsgOrExecute,
	pub wrapped: Wrapped,
}

impl Target {
	pub fn new(
		address: MultiAddress,
		ext_hash: H256,
		ext_index: u32,
		call: SendMsgOrExecute,
		wrapped: Wrapped,
	) -> Self {
		Self { address, ext_hash, ext_index, call, wrapped }
	}

	pub fn is_send_message_and_fungible(&self) -> bool {
		match &self.call {
			SendMsgOrExecute::Send(x) => match x.message {
				Message::FungibleToken { asset_id: _, amount: _ } => true,
				_ => false,
			},
			_ => false,
		}
	}

	pub fn is_send_message(&self) -> bool {
		match &self.call {
			SendMsgOrExecute::Send(_) => true,
			_ => false,
		}
	}

	pub fn is_execute(&self) -> bool {
		match &self.call {
			SendMsgOrExecute::Execute(_) => true,
			_ => false,
		}
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

#[derive(Debug, Default, Clone, Copy)]
pub struct Wrapped {
	pub inside_multisig: bool,
	pub inside_proxy: bool,
}

pub fn parse_transactions(list: &Vec<BlockEncodedExtrinsic>) -> Result<Vec<Target>, String> {
	let mut targets: Vec<Target> = Vec::with_capacity(list.len());
	for ext in list {
		let metadata = ext.metadata.clone();

		let Some(signature) = &ext.signature else {
			return Err("Extrinsic did not had signature. This is not good".into());
		};

		let mut wrapped = Wrapped::default();
		let call = ExtrinsicCall::try_from(&ext.call)?;
		let Some(call) = parse_extrinsic_call(&call, &mut wrapped)? else {
			continue;
		};
		let target = Target::new(signature.address.clone(), metadata.ext_hash, metadata.ext_index, call, wrapped);

		targets.push(target);
	}

	Ok(targets)
}

fn parse_extrinsic_call(call: &ExtrinsicCall, wrapped: &mut Wrapped) -> Result<Option<SendMsgOrExecute>, String> {
	let header = (call.pallet_id, call.variant_id);

	if header == SendMessage::HEADER_INDEX {
		return Ok(Some(parse_send_message_call(&call.data)?.into()));
	}

	if header == Execute::HEADER_INDEX {
		return Ok(Some(parse_execute_call(&call.data)?.into()));
	}

	if header == AsMulti::HEADER_INDEX {
		return parse_multisig_call(&call.data, wrapped);
	}

	if header == Proxy::HEADER_INDEX {
		return parse_proxy_call(&call.data, wrapped);
	}

	Ok(None)
}

fn parse_send_message_call(mut call_data: &[u8]) -> Result<SendMessage, String> {
	SendMessage::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_execute_call(mut call_data: &[u8]) -> Result<Execute, String> {
	Execute::decode(&mut call_data).map_err(|e| e.to_string())
}

fn parse_multisig_call(mut call_data: &[u8], wrapped: &mut Wrapped) -> Result<Option<SendMsgOrExecute>, String> {
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

	wrapped.inside_multisig = true;

	parse_extrinsic_call(&multi.call, wrapped)
}

fn parse_proxy_call(mut call_data: &[u8], wrapped: &mut Wrapped) -> Result<Option<SendMsgOrExecute>, String> {
	let proxy = Proxy::decode(&mut call_data)
		.map_err(|e| std::format!("Failed to convert raw ext to Proxy::Proxy. Err: {}", e))?;

	wrapped.inside_proxy = true;

	parse_extrinsic_call(&proxy.call, wrapped)
}
