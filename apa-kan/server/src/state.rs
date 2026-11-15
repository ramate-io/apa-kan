use std::sync::Arc;

use apa_kan_store::sqlite::SqliteStore;
use apa_kan_store::Clock;

use crate::mailer::Mailer;

#[derive(Clone)]
pub struct AppState {
	pub store: Arc<SqliteStore>,
	pub clock: Arc<dyn Clock>,
	pub mailer: Arc<dyn Mailer>,
}

impl AppState {
	pub fn new(store: SqliteStore, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Self {
		Self {
			store: Arc::new(store),
			clock,
			mailer,
		}
	}

	pub fn service(&self) -> crate::service::AccountService<SqliteStore> {
		crate::service::AccountService::new(
			self.store.clone(),
			self.clock.clone(),
			self.mailer.clone(),
		)
	}
}