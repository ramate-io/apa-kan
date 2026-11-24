use std::sync::Arc;

use apa_kan_account::Seed;
use apa_kan_ledger::LedgerStore;
use apa_kan_store::sqlite::SqliteStore;
use apa_kan_store::Clock;
use apa_kan_tx::TxSigner;

use crate::accounts::{AccountHost, AccountKeys};
use crate::games::{GameHost, DEFAULT_CHAIN_ID, DEFAULT_MAX_GAMES};
use crate::mailer::Mailer;
use crate::service::AccountService;

#[derive(Clone)]
pub struct AppState {
	pub store: Arc<SqliteStore>,
	pub clock: Arc<dyn Clock>,
	pub mailer: Arc<dyn Mailer>,
	pub games: Arc<GameHost>,
	pub accounts: Arc<AccountHost>,
}

impl AppState {
	/// In-memory ledger on the default chain with a throwaway operator key
	/// (tests and local experiments).
	pub fn new(store: SqliteStore, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Self {
		Self::with_ledger(
			store,
			clock,
			mailer,
			Arc::new(apa_kan_ledger::memory::MemoryLedger::default()),
			DEFAULT_CHAIN_ID,
			Some(TxSigner::from_seed(&Seed::random())),
			DEFAULT_MAX_GAMES,
		)
	}

	/// Games and accounts on one ledger and chain. `operator` signs recovery
	/// challenges and administers roles; `None` turns email recovery off.
	pub fn with_ledger(
		store: SqliteStore,
		clock: Arc<dyn Clock>,
		mailer: Arc<dyn Mailer>,
		ledger: Arc<dyn LedgerStore>,
		chain: &str,
		operator: Option<TxSigner>,
		max_games: usize,
	) -> Self {
		let store = Arc::new(store);
		let projection = Arc::new(AccountService::new(store.clone(), clock.clone()));
		let accounts = Arc::new(
			AccountHost::new(ledger.clone(), chain, operator, clock.clone(), mailer.clone())
				.with_projection(projection),
		);
		let games =
			GameHost::new(ledger, Arc::new(AccountKeys(accounts.clone())), chain, max_games);
		Self { store, clock, mailer, games: Arc::new(games), accounts }
	}

	pub fn service(&self) -> AccountService<SqliteStore> {
		AccountService::new(self.store.clone(), self.clock.clone())
	}
}
