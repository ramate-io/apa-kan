//! Write side of Apa Kan: one signed transaction envelope for every state
//! change, and the deterministic state machine that folds a ledger stream.
//!
//! - [`envelope`]: [`SignedTx`] = [`TxBody`] + ML-DSA signature. The body names
//!   the domain, chain, stream, sequence number, previous entry's hash, signing
//!   account and key, and a typed [`TxPayload`]. Signing mirrors
//!   `apa-kan-account`: canonical JSON (sorted keys), message id =
//!   SHA3-256(canonical ‖ signer_vk), ML-DSA-65 over the id. The id is also the
//!   entry hash the next transaction links to.
//! - [`game`]: [`GameState`] and [`GameState::apply`], the transition function
//!   for `game/<id>` streams (create, join, move, resign, cancel). State is the
//!   fold of the stream: [`game::replay`] rebuilds it from committed entries.
//!
//! Account authority uses the account crate's indirection: a transaction is
//! signed by one of the account's keys, the key travels in the body, and the
//! host checks it against the account's authorized keys ([`KeyStatus`]); an
//! account nobody has registered is self-certifying through its genesis key
//! (`AccountId = SHA3(domain ‖ genesis_vk)`).
//!
//! Nothing here does I/O: hosts (HTTP server today, peers over UDP later)
//! carry the bytes, look up key status, and persist entries via
//! `apa-kan-ledger`.

pub mod envelope;
pub mod game;

pub use envelope::{
	admit, verify_self_contained, KeyStatus, SignedTx, TxBody, TxPayload, TxSigner, GENESIS_PREV,
	TX_DOMAIN,
};
pub use game::{deal_salt, game_id_of, game_stream, seed_commitment, GameState};

use apa_kan_view::{GamePhase, HostedGameView, Seat};
use serde::{Deserialize, Serialize};

/// What an accepted transaction did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxReceipt {
	/// Hex id (= ledger entry hash) of the accepted transaction.
	pub tx_id: String,
	pub stream: String,
	/// `seq` of this transaction; the stream's next is `seq + 1`.
	pub seq: u64,
	pub kind: String,
	pub phase: GamePhase,
	/// Engine log line for a move.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub log_line: Option<String>,
}

/// Host reply to an accepted transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxResponse {
	pub receipt: TxReceipt,
	pub view: HostedGameView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TxError {
	#[error("stream not found")]
	NotFound,
	#[error("malformed transaction: {reason}")]
	BadEnvelope { reason: String },
	#[error("transaction is for chain {got}, this host is {expected}")]
	WrongChain { expected: String, got: String },
	#[error("unsupported stream {stream}")]
	UnsupportedStream { stream: String },
	#[error("signature does not verify")]
	BadSignature,
	#[error("signing key is not authorized for the account")]
	KeyNotAuthorized,
	#[error("stale sequence: stream is at {current}, transaction was built at {got}")]
	StaleSeq { current: u64, got: u64 },
	#[error("prev does not match the stream head {expected}")]
	PrevMismatch { expected: String },
	#[error("signer may not do that here")]
	Unauthorized,
	#[error("account already holds a seat")]
	AlreadySeated,
	#[error("game is {phase:?}")]
	WrongPhase { phase: GamePhase },
	#[error("game is over")]
	GameOver,
	#[error("not your turn: seat {to_move} is to move")]
	NotYourTurn { to_move: Seat },
	#[error("revealed salt does not match the creator's commitment")]
	BadReveal,
	#[error("illegal action: {reason}")]
	Illegal { reason: String },
	#[error("host error: {reason}")]
	Host { reason: String },
}
