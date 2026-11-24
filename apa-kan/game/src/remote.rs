//! Client side of `/game/<id>` for server-hosted games.
//!
//! The server (`apa-kan-server`) is a ledger host: every change to a game is
//! a signed `apa-kan-tx` transaction posted to `POST /v1/tx`, and every read
//! is a view (`GET /v1/games/<id>` returns a [`HostedGameView`]). The client
//! holds an ML-DSA account key ([`identity`]), signs create / join / move
//! transactions against the stream head from the last view, replays the
//! view's record with core to get a real [`Game`], and polls while the other
//! seat moves. A rejected transaction (stale seq, illegal) drops the
//! optimistic move and reloads the authoritative view.
//!
//! HTTP goes through `ehttp` (browser `fetch` on web, a background thread on
//! desktop). Android and iOS have no HTTP client wired yet, so there
//! `/game/<id>` keeps the offline seeded stub.

use std::collections::VecDeque;
use std::sync::Mutex;

use apa_kan_tx::{game_stream, SignedTx, TxPayload, GENESIS_PREV};
use apa_kan_view::{GamePhase, HostedGameView, LedgerHead, Seat};

use crate::game::{Action, Game};
use crate::host::HostConfig;

/// Seconds between view polls while waiting on the other seat.
pub const POLL_SECS: f32 = 1.5;

/// Whether this build can reach a server.
pub const HTTP_AVAILABLE: bool =
	cfg!(any(target_arch = "wasm32", not(any(target_os = "android", target_os = "ios"))));

/// This device's account key.
///
/// A 32-byte ML-DSA seed, generated on first use and kept in `localStorage`
/// (`apakan.key.v1`) on web, or in `APAKAN_KEY_FILE` on desktop (memory only
/// when unset). The account is the one this key is genesis for, so it works
/// unregistered and becomes a full account if registered with
/// `POST /v1/accounts`; linking more devices uses the account crate's link flow.
pub mod identity {
	use std::sync::OnceLock;

	use apa_kan_account::Seed;
	use apa_kan_tx::TxSigner;

	const STORAGE_KEY: &str = "apakan.key.v1";

	fn parse(hex_seed: &str) -> Option<Seed> {
		let bytes: [u8; 32] = hex::decode(hex_seed.trim()).ok()?.try_into().ok()?;
		Some(Seed::from_bytes(bytes))
	}

	fn load_or_create() -> Seed {
		#[cfg(target_arch = "wasm32")]
		{
			let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
			if let Some(seed) = storage
				.as_ref()
				.and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
				.and_then(|v| parse(&v))
			{
				return seed;
			}
			let seed = Seed::random();
			if let Some(s) = storage {
				let _ = s.set_item(STORAGE_KEY, &hex::encode(seed.0));
			}
			seed
		}
		#[cfg(not(target_arch = "wasm32"))]
		{
			let _ = STORAGE_KEY;
			let path = std::env::var_os("APAKAN_KEY_FILE").map(std::path::PathBuf::from);
			if let Some(seed) = path
				.as_ref()
				.and_then(|p| std::fs::read_to_string(p).ok())
				.and_then(|v| parse(&v))
			{
				return seed;
			}
			let seed = Seed::random();
			if let Some(p) = path {
				let _ = std::fs::write(p, hex::encode(seed.0));
			}
			seed
		}
	}

	/// The signer for this device (loaded once).
	pub fn signer() -> &'static TxSigner {
		static SIGNER: OnceLock<TxSigner> = OnceLock::new();
		SIGNER.get_or_init(|| TxSigner::from_seed(&load_or_create()))
	}

	/// Hex account id of [`signer`].
	pub fn account_hex() -> String {
		signer().account().to_hex()
	}
}

/// One hosted game this client is attached to.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteSession {
	pub id: String,
	/// Hex account this client signs as.
	pub account: String,
	/// The seat this account holds, from the last view (`None` = spectating
	/// or not joined yet).
	pub seat: Option<Seat>,
	pub phase: GamePhase,
	/// Stream head from the last view or accepted transaction.
	pub ledger: Option<LedgerHead>,
	/// Engine record length of the position the client holds.
	pub game_seq: Option<u64>,
	pub loaded: bool,
	/// A transaction or view request is in flight.
	pub inflight: bool,
	/// A join was already sent for this game.
	pub join_sent: bool,
	pub poll_wait: f32,
	pub labels: [String; 2],
	/// Invite link to show the creator while the seat is open.
	pub invite: Option<String>,
}

impl RemoteSession {
	pub fn new(id: impl Into<String>, account: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			account: account.into(),
			seat: None,
			phase: GamePhase::Open,
			ledger: None,
			game_seq: None,
			loaded: false,
			inflight: false,
			join_sent: false,
			poll_wait: 0.0,
			labels: ["P1".into(), "P2".into()],
			invite: None,
		}
	}

	pub fn player(&self) -> Option<crate::game::Player> {
		self.seat.and_then(apa_kan_view::player_of)
	}

	/// May the local user act on `game` right now?
	pub fn can_act(&self, game: &Game) -> Result<(), &'static str> {
		if !self.loaded {
			return Err("Loading game from the server…");
		}
		if self.inflight {
			return Err("Sending move…");
		}
		match self.phase {
			GamePhase::Open => return Err("Waiting for the other seat to join…"),
			GamePhase::Cancelled => return Err("This game was cancelled."),
			GamePhase::Finished => return Err("Game over."),
			GamePhase::Playing => {}
		}
		let Some(me) = self.player() else {
			return Err("Spectating: both seats are taken.");
		};
		if game.current_player() != me {
			return Err("Waiting for the other seat…");
		}
		Ok(())
	}

	/// Sign `payload` against the current stream head.
	pub fn sign(&self, payload: TxPayload) -> Option<SignedTx> {
		let head = self.ledger.as_ref()?;
		identity::signer()
			.sign(&head.chain, &head.stream, head.seq, &head.head, payload)
			.ok()
	}

	/// Signed move for an action just chosen locally.
	pub fn transaction(&self, action: Action) -> Option<SignedTx> {
		self.seat?;
		self.sign(TxPayload::GameMove { action: action.into() })
	}

	/// Should this client take the open seat? (Opening an invite link joins.)
	pub fn wants_join(&self) -> bool {
		self.loaded
			&& !self.inflight
			&& !self.join_sent
			&& self.phase == GamePhase::Open
			&& self.seat.is_none()
	}

	/// Should we poll for the other seat?
	pub fn wants_poll(&self, game: &Game) -> bool {
		if !self.loaded || self.inflight {
			return false;
		}
		match self.phase {
			GamePhase::Open => self.seat.is_some(),
			GamePhase::Playing => {
				!game.status().is_over() && self.player() != Some(game.current_player())
			}
			GamePhase::Finished | GamePhase::Cancelled => false,
		}
	}

	/// Adopt an authoritative view. Returns the replayed game when the
	/// position changed (new engine seq or first load with a position).
	pub fn adopt(&mut self, view: &HostedGameView) -> Result<Option<Game>, String> {
		if view.id != self.id {
			return Err(format!("view is for game {}, not {}", view.id, self.id));
		}
		self.seat = view.seat_of_account(&self.account);
		self.phase = view.phase;
		self.labels = [0, 1].map(|i| {
			view.seats[i]
				.as_ref()
				.map(|c| c.label.clone())
				.unwrap_or_else(|| format!("P{}", i + 1))
		});
		self.ledger = Some(view.ledger.clone());
		self.loaded = true;
		let Some(g) = view.game.as_ref() else {
			return Ok(None);
		};
		if self.game_seq == Some(g.seq) {
			return Ok(None);
		}
		let game = g.replay()?;
		self.game_seq = Some(g.seq);
		Ok(Some(game))
	}
}

/// Shareable link to a game: opening it joins the open seat or spectates.
pub fn game_link(host: &HostConfig, id: &str) -> String {
	host.share_url(&crate::route::Route::Game { id: id.to_string() })
}

/// Fresh random game id (24 lowercase hex).
pub fn new_game_id() -> String {
	use rand::RngCore;
	let mut b = [0u8; 12];
	rand::thread_rng().fill_bytes(&mut b);
	hex::encode(b)
}

/// Result of one HTTP call, drained by the Bevy client each frame.
#[derive(Clone, Debug)]
pub enum RemoteEvent {
	View { id: String, view: Box<HostedGameView> },
	Created { view: Box<HostedGameView> },
	TxAccepted { id: String, view: Box<HostedGameView> },
	TxRejected { id: String, message: String },
	Failed { id: String, message: String },
}

#[derive(serde::Deserialize)]
struct TxResponseWire {
	view: HostedGameView,
}

#[derive(serde::Deserialize)]
struct LedgerInfoWire {
	chain: String,
}

fn events() -> &'static Mutex<VecDeque<RemoteEvent>> {
	static Q: Mutex<VecDeque<RemoteEvent>> = Mutex::new(VecDeque::new());
	&Q
}

fn push(ev: RemoteEvent) {
	if let Ok(mut q) = events().lock() {
		q.push_back(ev);
	}
}

pub fn drain_events() -> Vec<RemoteEvent> {
	events().lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
}

/// Typed error body from the server, flattened to one line.
fn error_message(status: u16, body: &str) -> String {
	match serde_json::from_str::<apa_kan_tx::TxError>(body) {
		Ok(e) => e.to_string(),
		Err(_) => format!("HTTP {status}"),
	}
}

/// Signed `game.create` taking seat 1 of a fresh game on `chain`.
pub fn create_tx(chain: &str, id: &str) -> Option<SignedTx> {
	identity::signer()
		.sign(
			chain,
			&game_stream(id),
			0,
			GENESIS_PREV,
			TxPayload::GameCreate { seat: 1, label: String::new() },
		)
		.ok()
}

#[cfg(any(target_arch = "wasm32", not(any(target_os = "android", target_os = "ios"))))]
mod http {
	use super::*;

	fn json_request(url: String, body: Option<Vec<u8>>) -> ehttp::Request {
		match body {
			Some(b) => {
				let mut req = ehttp::Request::post(url, b);
				req.headers = ehttp::Headers::new(&[
					("Content-Type", "application/json"),
					("Accept", "application/json"),
				]);
				req
			}
			None => {
				let mut req = ehttp::Request::get(url);
				req.headers = ehttp::Headers::new(&[("Accept", "application/json")]);
				req
			}
		}
	}

	pub fn get_view(host: &HostConfig, id: &str) -> bool {
		let Some(url) = host.api(&format!("/v1/games/{id}")) else {
			return false;
		};
		let id = id.to_string();
		ehttp::fetch(json_request(url, None), move |res| match res {
			Ok(r) if r.ok => match serde_json::from_slice::<HostedGameView>(&r.bytes) {
				Ok(view) => push(RemoteEvent::View { id, view: Box::new(view) }),
				Err(e) => push(RemoteEvent::Failed { id, message: format!("bad view: {e}") }),
			},
			Ok(r) => push(RemoteEvent::Failed {
				id,
				message: error_message(r.status, r.text().unwrap_or_default()),
			}),
			Err(e) => push(RemoteEvent::Failed { id, message: e }),
		});
		true
	}

	/// `POST /v1/tx`. `created` reports success as [`RemoteEvent::Created`].
	pub fn post_tx(host: &HostConfig, tx: &SignedTx, created: bool) -> bool {
		let Some(url) = host.api("/v1/tx") else {
			return false;
		};
		let Ok(body) = serde_json::to_vec(tx) else {
			return false;
		};
		let id = apa_kan_tx::game_id_of(&tx.body.stream).unwrap_or_default().to_string();
		ehttp::fetch(json_request(url, Some(body)), move |res| match res {
			Ok(r) if r.ok => match serde_json::from_slice::<TxResponseWire>(&r.bytes) {
				Ok(t) if created => push(RemoteEvent::Created { view: Box::new(t.view) }),
				Ok(t) => push(RemoteEvent::TxAccepted { id, view: Box::new(t.view) }),
				Err(e) => push(RemoteEvent::TxRejected { id, message: format!("bad reply: {e}") }),
			},
			Ok(r) => {
				let message = error_message(r.status, r.text().unwrap_or_default());
				if created {
					push(RemoteEvent::Failed { id: String::new(), message });
				} else {
					push(RemoteEvent::TxRejected { id, message });
				}
			}
			Err(e) => push(RemoteEvent::Failed {
				id: if created { String::new() } else { id },
				message: e,
			}),
		});
		true
	}

	/// Read the chain id (`GET /v1/ledger`), then sign and post `game.create`.
	pub fn create_game(host: &HostConfig) -> bool {
		let Some(url) = host.api("/v1/ledger") else {
			return false;
		};
		let host = host.clone();
		let fail = |message: String| push(RemoteEvent::Failed { id: String::new(), message });
		ehttp::fetch(json_request(url, None), move |res| match res {
			Ok(r) if r.ok => match serde_json::from_slice::<LedgerInfoWire>(&r.bytes) {
				Ok(info) => match create_tx(&info.chain, &new_game_id()) {
					Some(tx) => {
						if !post_tx(&host, &tx, true) {
							fail("no API configured".into());
						}
					}
					None => fail("could not sign the create transaction".into()),
				},
				Err(e) => fail(format!("bad ledger info: {e}")),
			},
			Ok(r) => fail(error_message(r.status, r.text().unwrap_or_default())),
			Err(e) => fail(e),
		});
		true
	}
}

#[cfg(not(any(target_arch = "wasm32", not(any(target_os = "android", target_os = "ios")))))]
mod http {
	use super::*;

	pub fn get_view(_host: &HostConfig, _id: &str) -> bool {
		false
	}

	pub fn post_tx(_host: &HostConfig, _tx: &SignedTx, _created: bool) -> bool {
		false
	}

	pub fn create_game(_host: &HostConfig) -> bool {
		false
	}
}

/// Start a `GET /v1/games/<id>`. `false` when no API is configured.
pub fn request_view(host: &HostConfig, id: &str) -> bool {
	http::get_view(host, id)
}

/// Start a `POST /v1/tx`. `false` when offline.
pub fn submit(host: &HostConfig, tx: &SignedTx) -> bool {
	http::post_tx(host, tx, false)
}

/// Create a hosted game (`GET /v1/ledger`, then a signed `game.create`).
/// `false` when offline.
pub fn request_create(host: &HostConfig) -> bool {
	http::create_game(host)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;
	use crate::host::HostOverrides;
	use apa_kan_account::Seed;
	use apa_kan_tx::{admit, GameState, KeyStatus, TxSigner};

	/// Drive the real state machine with this device's key plus an opponent.
	fn hosted(moves: usize) -> (GameState, TxSigner) {
		let other = TxSigner::from_seed(&Seed::from_bytes([77; 32]));
		let tx = create_tx("c", "g13").unwrap();
		let id = admit(&tx, "c", KeyStatus::UnknownAccount).unwrap();
		let (mut s, _) = GameState::apply(None, &tx, &id).unwrap();
		let join = other
			.sign("c", "game/g13", s.next_seq, &s.head, TxPayload::GameJoin { label: "opp".into() })
			.unwrap();
		let id = admit(&join, "c", KeyStatus::UnknownAccount).unwrap();
		s = GameState::apply(Some(&s), &join, &id).unwrap().0;
		for _ in 0..moves {
			let g = s.game.as_ref().unwrap();
			let action = g.legal_actions()[0];
			let me_to_move = s.view().seat_of_account(&identity::account_hex())
				== Some(apa_kan_view::seat_of(g.current_player()));
			let payload = TxPayload::GameMove { action: action.into() };
			let tx = if me_to_move {
				identity::signer().sign("c", "game/g13", s.next_seq, &s.head, payload)
			} else {
				other.sign("c", "game/g13", s.next_seq, &s.head, payload)
			}
			.unwrap();
			let id = admit(&tx, "c", KeyStatus::UnknownAccount).unwrap();
			s = GameState::apply(Some(&s), &tx, &id).unwrap().0;
		}
		(s, other)
	}

	#[test]
	fn adopt_derives_seat_and_gates_turns() {
		let (state, _) = hosted(0);
		let view = state.view();
		let mut s = RemoteSession::new("g13", identity::account_hex());
		let got = s.adopt(&view).unwrap().unwrap();
		assert_eq!(s.seat, Some(1));
		assert_eq!(s.phase, GamePhase::Playing);
		assert_eq!(s.labels[1], "opp");
		assert_eq!(s.can_act(&got), Ok(()));
		assert!(!s.wants_poll(&got));
		assert!(!s.wants_join());
		// Same view again is a no-op.
		assert!(s.adopt(&view).unwrap().is_none());
		let tx = s.transaction(got.legal_actions()[0]).unwrap();
		assert_eq!(tx.body.seq, view.ledger.seq);
		assert_eq!(tx.body.prev, view.ledger.head);
		assert!(admit(&tx, "c", KeyStatus::UnknownAccount).is_ok());
		assert!(GameState::apply(Some(&state), &tx, &tx.id().unwrap()).is_ok());

		// Another account spectates.
		let mut spec = RemoteSession::new("g13", "ff".repeat(32));
		let got = spec.adopt(&view).unwrap().unwrap();
		assert!(spec.can_act(&got).is_err());
		assert!(spec.transaction(Action::Pass).is_none());
		let mut other = RemoteSession::new("zzz", "aa");
		assert!(other.adopt(&view).is_err());
	}

	#[test]
	fn open_game_invites_a_join_and_newer_views_replace_the_game() {
		let tx = create_tx("c", "g13").unwrap();
		let id = admit(&tx, "c", KeyStatus::UnknownAccount).unwrap();
		let (open, _) = GameState::apply(None, &tx, &id).unwrap();
		let mut guest = RemoteSession::new("g13", "ee".repeat(32));
		assert!(guest.adopt(&open.view()).unwrap().is_none());
		assert!(guest.wants_join());
		let mut creator = RemoteSession::new("g13", identity::account_hex());
		creator.adopt(&open.view()).unwrap();
		assert!(!creator.wants_join());
		assert!(creator.wants_poll(&Game::new(crate::game::RuleSet::default(), 1)));

		let (s0, _) = hosted(0);
		let (s2, _) = hosted(2);
		let mut s = RemoteSession::new("g13", identity::account_hex());
		s.adopt(&s0.view()).unwrap();
		let next = s.adopt(&s2.view()).unwrap().unwrap();
		assert_eq!(next.turn_count(), s2.game.as_ref().unwrap().turn_count());
	}

	#[test]
	fn links() {
		let host = HostConfig::from_overrides(HostOverrides {
			public_url: Some("https://play.example".into()),
			..Default::default()
		});
		let link = game_link(&host, "g1");
		assert_eq!(link, "https://play.example/#/game/g1");
		let parsed = crate::route::DeepLink::parse(&link);
		assert_eq!(parsed.route, crate::route::Route::Game { id: "g1".into() });
		let id = new_game_id();
		assert_eq!(id.len(), 24);
		assert!(apa_kan_tx::game_id_of(&game_stream(&id)).is_some());
	}

	#[test]
	fn offline_host_sends_nothing() {
		let host = HostConfig::default();
		assert!(!request_view(&host, "g"));
		assert!(!request_create(&host));
		assert!(drain_events().is_empty());
	}

	#[test]
	fn error_bodies_flatten() {
		let body = serde_json::to_string(&apa_kan_tx::TxError::GameOver).unwrap();
		assert_eq!(error_message(409, &body), "game is over");
		assert_eq!(error_message(502, "<html>"), "HTTP 502");
	}
}
