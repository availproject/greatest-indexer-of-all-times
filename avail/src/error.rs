use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
	SDK(avail_rust::Error),
	SQL(sqlx::Error),
}

impl From<avail_rust::Error> for Error {
	fn from(value: avail_rust::Error) -> Self {
		Self::SDK(value)
	}
}

impl From<avail_rust::avail_rust_core::RpcError> for Error {
	fn from(value: avail_rust::avail_rust_core::RpcError) -> Self {
		Self::SDK(value.into())
	}
}

impl From<sqlx::Error> for Error {
	fn from(value: sqlx::Error) -> Self {
		Self::SQL(value)
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::SDK(error) => error.fmt(f),
			Error::SQL(error) => error.fmt(f),
		}
	}
}
