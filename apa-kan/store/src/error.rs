use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
	#[error("not found")]
	NotFound,
	#[error("conflict: {0}")]
	Conflict(String),
	#[error("bad nonce")]
	BadNonce,
	#[error("expired")]
	Expired,
	#[error("invalid input: {0}")]
	InvalidInput(String),
	#[error("storage: {0}")]
	Storage(String),
	#[error("account error: {0}")]
	Account(#[from] apa_kan_account::link::LinkError),
}
