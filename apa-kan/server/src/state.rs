use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use apa_kan_account::auth::AuthChallengePayload;
use apa_kan_store::{Clock, Store};

use crate::mailer::Mailer;
use crate::session::SessionStore;

#[derive(Clone)]
pub struct AppState {
	pub store: Arc<dyn Store>,
	pub clock: Arc<dyn Clock>,
	pub mailer: Arc<dyn Mailer>,
	pub sessions: Arc<SessionStore>,
	pub challenges: Arc<Mutex<HashMap<String, AuthChallengePayload>>>,
	pub bootstrap_admin: Option<String>,
	pub allow_first_admin: bool,
}

impl AppState {
	pub fn new(store: Arc<dyn Store>, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Self {
		Self {
			store,
			clock,
			mailer,
			sessions: Arc::new(SessionStore::new()),
			challenges: Arc::new(Mutex::new(HashMap::new())),
			bootstrap_admin: std::env::var("APA_KAN_ADMIN_ACCOUNT").ok(),
			allow_first_admin: std::env::var("APA_KAN_ALLOW_FIRST_ADMIN")
				.map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
				.unwrap_or(true),
		}
	}

	pub fn service(&self) -> crate::service::AccountService {
		crate::service::AccountService::new(
			self.store.clone(),
			self.clock.clone(),
			self.mailer.clone(),
		)
	}
}
