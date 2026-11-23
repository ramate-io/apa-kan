use std::sync::Arc;

use apa_kan_store::sqlite::SqliteStore;
use apa_kan_store::Clock;

use crate::games::GameHost;
use crate::mailer::Mailer;

#[derive(Clone)]
pub struct AppState {
	pub store: Arc<SqliteStore>,
	pub clock: Arc<dyn Clock>,
	pub mailer: Arc<dyn Mailer>,
	pub games: Arc<GameHost>,
}

impl AppState {
	pub fn new(store: SqliteStore, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Self {
		Self { store: Arc::new(store), clock, mailer, games: Arc::new(GameHost::default()) }
	}

	/// Swap in a game host (e.g. one backed by a durable store).
	pub fn with_games(mut self, games: GameHost) -> Self {
		self.games = Arc::new(games);
		self
	}

	pub fn service(&self) -> crate::service::AccountService<SqliteStore> {
		crate::service::AccountService::new(
			self.store.clone(),
			self.clock.clone(),
			self.mailer.clone(),
		)
	}
}
