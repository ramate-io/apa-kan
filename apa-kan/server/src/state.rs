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
		let store = Arc::new(store);
		let games =
			GameHost::default().with_keys(Arc::new(crate::games::AccountStoreKeys(store.clone())));
		Self { store, clock, mailer, games: Arc::new(games) }
	}

	/// Swap in a game host (e.g. one over a durable ledger). Pair it with
	/// [`AppState::account_keys`] so admission sees registered accounts.
	pub fn with_games(mut self, games: GameHost) -> Self {
		self.games = Arc::new(games);
		self
	}

	/// Key directory over this state's account store.
	pub fn account_keys(&self) -> Arc<dyn crate::games::KeyDirectory> {
		Arc::new(crate::games::AccountStoreKeys(self.store.clone()))
	}

	pub fn service(&self) -> crate::service::AccountService<SqliteStore> {
		crate::service::AccountService::new(
			self.store.clone(),
			self.clock.clone(),
			self.mailer.clone(),
		)
	}
}
