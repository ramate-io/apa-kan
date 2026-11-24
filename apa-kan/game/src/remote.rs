//! Client side of `/game/<id>` for server-hosted games.
//!
//! The server (`apa-kan-server`) holds the authoritative [`GameRecord`]. The
//! client fetches a [`GameView`], replays its record with core to get a real
//! [`Game`], and submits each local action as an `apa-kan-tx`
//! [`Transaction`]. While it is not this seat's turn the client polls the view
//! and replays any new actions. A rejected transaction (stale seq, illegal)
//! drops the optimistic move and reloads the authoritative view.
//!
//! HTTP goes through `ehttp` (browser `fetch` on web, a background thread on
//! desktop). Android and iOS have no HTTP client wired yet, so there
//! `/game/<id>` keeps the offline seeded stub.

use std::collections::VecDeque;
use std::sync::Mutex;

use apa_kan_tx::{Transaction, TxAuth};
use apa_kan_view::{player_of, ActionWire, GameView, Seat};

use crate::game::{Action, Game, Player};
use crate::host::HostConfig;

/// Seconds between view polls while waiting on the other seat.
pub const POLL_SECS: f32 = 1.5;

/// Whether this build can reach a server.
pub const HTTP_AVAILABLE: bool =
	cfg!(any(target_arch = "wasm32", not(any(target_os = "android", target_os = "ios"))));

/// One hosted game this client is attached to.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteSession {
	pub id: String,
	/// The seat this client plays, or `None` to spectate.
	pub seat: Option<Seat>,
	pub token: Option<String>,
	/// Server record seq the local game matches (after the last load/accept).
	pub seq: u64,
	pub loaded: bool,
	/// A transaction or view request is in flight.
	pub inflight: bool,
	pub poll_wait: f32,
	pub labels: [String; 2],
	/// Seat-2 invite link to show the host after creating a game.
	pub invite: Option<String>,
}

impl RemoteSession {
	pub fn new(id: impl Into<String>, seat: Option<Seat>, token: Option<String>) -> Self {
		let seat = seat.filter(|s| player_of(*s).is_some());
		let token = token.filter(|t| !t.is_empty());
		Self {
			id: id.into(),
			seat: if token.is_some() { seat } else { None },
			token,
			seq: 0,
			loaded: false,
			inflight: false,
			poll_wait: 0.0,
			labels: ["P1".into(), "P2".into()],
			invite: None,
		}
	}

	pub fn player(&self) -> Option<Player> {
		self.seat.and_then(player_of)
	}

	/// May the local user act on `game` right now?
	pub fn can_act(&self, game: &Game) -> Result<(), &'static str> {
		if !self.loaded {
			return Err("Loading game from the server…");
		}
		if self.inflight {
			return Err("Sending move…");
		}
		let Some(me) = self.player() else {
			return Err("Spectating: open your seat link to play.");
		};
		if game.current_player() != me {
			return Err("Waiting for the other seat…");
		}
		Ok(())
	}

	/// Build the transaction for an action just chosen locally.
	pub fn transaction(&self, action: Action) -> Option<Transaction> {
		Some(Transaction {
			game_id: self.id.clone(),
			seq: self.seq,
			seat: self.seat?,
			action: ActionWire::from(action),
			auth: TxAuth::SeatToken { token: self.token.clone()? },
		})
	}

	/// Should we poll for the other seat's move?
	pub fn wants_poll(&self, game: &Game) -> bool {
		self.loaded
			&& !self.inflight
			&& !game.status().is_over()
			&& self.player() != Some(game.current_player())
	}

	/// Adopt an authoritative view. Returns the replayed game when it differs
	/// from what the client has (new seq or first load).
	pub fn adopt(&mut self, view: &GameView) -> Result<Option<Game>, String> {
		if view.id != self.id {
			return Err(format!("view is for game {}, not {}", view.id, self.id));
		}
		let fresh = !self.loaded || view.seq != self.seq;
		self.labels = [view.seats[0].label.clone(), view.seats[1].label.clone()];
		if !fresh {
			return Ok(None);
		}
		let game = view.replay()?;
		self.seq = view.seq;
		self.loaded = true;
		Ok(Some(game))
	}
}

/// `?seat=N&token=…` from a deep link's query pairs.
pub fn seat_from_query(
	query: &std::collections::BTreeMap<String, String>,
) -> (Option<Seat>, Option<String>) {
	let seat = query.get("seat").and_then(|s| s.parse::<Seat>().ok());
	let token = query.get("token").cloned();
	(seat, token)
}

/// Shareable link that claims `seat` of `id`.
pub fn seat_link(host: &HostConfig, id: &str, seat: Seat, token: &str) -> String {
	let route = crate::route::Route::Game { id: id.to_string() };
	format!("{}?seat={seat}&token={token}", host.share_url(&route))
}

/// Result of one HTTP call, drained by the Bevy client each frame.
#[derive(Clone, Debug)]
pub enum RemoteEvent {
	View { id: String, view: Box<GameView> },
	Created { created: Box<CreatedGameWire> },
	TxAccepted { id: String, view: Box<GameView> },
	TxRejected { id: String, message: String },
	Failed { id: String, message: String },
}

/// `POST /v1/games` response, mirrored so the client need not depend on the
/// server crate.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreatedGameWire {
	pub game_id: String,
	pub seats: [SeatInviteWire; 2],
	pub view: GameView,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SeatInviteWire {
	pub seat: Seat,
	pub token: String,
}

#[derive(serde::Deserialize)]
struct TxResponseWire {
	view: GameView,
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
			Ok(r) if r.ok => match serde_json::from_slice::<GameView>(&r.bytes) {
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

	pub fn post_tx(host: &HostConfig, tx: &Transaction) -> bool {
		let Some(url) = host.api(&format!("/v1/games/{}/transactions", tx.game_id)) else {
			return false;
		};
		let Ok(body) = serde_json::to_vec(tx) else {
			return false;
		};
		let id = tx.game_id.clone();
		ehttp::fetch(json_request(url, Some(body)), move |res| match res {
			Ok(r) if r.ok => match serde_json::from_slice::<TxResponseWire>(&r.bytes) {
				Ok(t) => push(RemoteEvent::TxAccepted { id, view: Box::new(t.view) }),
				Err(e) => push(RemoteEvent::TxRejected { id, message: format!("bad reply: {e}") }),
			},
			Ok(r) => push(RemoteEvent::TxRejected {
				id,
				message: error_message(r.status, r.text().unwrap_or_default()),
			}),
			Err(e) => push(RemoteEvent::Failed { id, message: e }),
		});
		true
	}

	pub fn create_game(host: &HostConfig) -> bool {
		let Some(url) = host.api("/v1/games") else {
			return false;
		};
		ehttp::fetch(json_request(url, Some(b"{}".to_vec())), move |res| match res {
			Ok(r) if r.ok => match serde_json::from_slice::<CreatedGameWire>(&r.bytes) {
				Ok(created) => push(RemoteEvent::Created { created: Box::new(created) }),
				Err(e) => push(RemoteEvent::Failed {
					id: String::new(),
					message: format!("bad create reply: {e}"),
				}),
			},
			Ok(r) => push(RemoteEvent::Failed {
				id: String::new(),
				message: error_message(r.status, r.text().unwrap_or_default()),
			}),
			Err(e) => push(RemoteEvent::Failed { id: String::new(), message: e }),
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

	pub fn post_tx(_host: &HostConfig, _tx: &Transaction) -> bool {
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

/// Start a `POST /v1/games/<id>/transactions`. `false` when offline.
pub fn submit(host: &HostConfig, tx: &Transaction) -> bool {
	http::post_tx(host, tx)
}

/// Start a `POST /v1/games`. `false` when offline.
pub fn request_create(host: &HostConfig) -> bool {
	http::create_game(host)
}

/// Remember a seat token for `id` so a reload keeps the seat (web only).
pub fn remember_seat(id: &str, seat: Seat, token: &str) {
	#[cfg(target_arch = "wasm32")]
	{
		if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
			let _ = s.set_item(&format!("apakan.seat.{id}"), &format!("{seat}:{token}"));
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = (id, seat, token);
	}
}

/// Seat token saved by [`remember_seat`].
pub fn recall_seat(id: &str) -> Option<(Seat, String)> {
	#[cfg(target_arch = "wasm32")]
	{
		let s = web_sys::window()?.local_storage().ok()??;
		let v = s.get_item(&format!("apakan.seat.{id}")).ok()??;
		let (seat, token) = v.split_once(':')?;
		Some((seat.parse().ok()?, token.to_string()))
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = id;
		None
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;
	use crate::game::RuleSet;
	use crate::host::HostOverrides;
	use apa_kan_view::GameRecord;

	fn hosted(n: usize) -> (GameRecord, Game) {
		let mut game = Game::new(RuleSet::default(), 13);
		let mut rec = GameRecord::begin("g13", &game).unwrap();
		for _ in 0..n {
			let a = game.legal_actions()[0];
			game.apply(a).unwrap();
			rec.actions.push(a.into());
		}
		(rec, game)
	}

	#[test]
	fn seat_requires_a_token_and_valid_number() {
		assert_eq!(RemoteSession::new("g", Some(1), None).seat, None);
		assert_eq!(RemoteSession::new("g", Some(3), Some("t".into())).seat, None);
		assert_eq!(RemoteSession::new("g", Some(2), Some("t".into())).seat, Some(2));
		assert_eq!(RemoteSession::new("g", Some(2), Some(String::new())).seat, None);
	}

	#[test]
	fn adopt_replays_and_gates_turns() {
		let (rec, game) = hosted(0);
		let view = GameView::new(&rec, &game, ["a", "b"]);
		let mut s = RemoteSession::new("g13", Some(1), Some("t".into()));
		assert!(s.can_act(&game).is_err());
		let got = s.adopt(&view).unwrap().unwrap();
		assert_eq!(got.legal_actions(), game.legal_actions());
		assert!(s.loaded);
		assert_eq!(s.labels[0], "a");
		assert_eq!(s.can_act(&got), Ok(()));
		assert!(!s.wants_poll(&got));
		// Same seq again is a no-op.
		assert!(s.adopt(&view).unwrap().is_none());
		let tx = s.transaction(got.legal_actions()[0]).unwrap();
		assert_eq!(tx.seq, 0);
		assert_eq!(tx.seat, 1);
		// Spectators never act and always poll.
		let mut spec = RemoteSession::new("g13", None, None);
		spec.adopt(&view).unwrap();
		assert!(spec.can_act(&got).is_err());
		assert!(spec.wants_poll(&got));
		assert!(spec.transaction(Action::Pass).is_none());
		// Wrong game id is refused.
		let mut other = RemoteSession::new("zzz", None, None);
		assert!(other.adopt(&view).is_err());
	}

	#[test]
	fn newer_view_replaces_local_game() {
		let (rec0, g0) = hosted(0);
		let mut s = RemoteSession::new("g13", Some(2), Some("t".into()));
		s.adopt(&GameView::new(&rec0, &g0, ["a", "b"])).unwrap();
		assert!(s.wants_poll(&g0));
		let (rec2, g2) = hosted(2);
		let next = s.adopt(&GameView::new(&rec2, &g2, ["a", "b"])).unwrap().unwrap();
		assert_eq!(s.seq, 2);
		assert_eq!(next.turn_count(), g2.turn_count());
	}

	#[test]
	fn links_and_query() {
		let host = HostConfig::from_overrides(HostOverrides {
			public_url: Some("https://play.example".into()),
			..Default::default()
		});
		let link = seat_link(&host, "g1", 2, "abc");
		assert_eq!(link, "https://play.example/#/game/g1?seat=2&token=abc");
		let parsed = crate::route::DeepLink::parse(&link);
		assert_eq!(parsed.route, crate::route::Route::Game { id: "g1".into() });
		assert_eq!(seat_from_query(&parsed.query), (Some(2), Some("abc".into())));
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
