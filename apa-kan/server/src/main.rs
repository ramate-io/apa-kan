use std::net::SocketAddr;
use std::sync::Arc;

use apa_kan_server::api;
use apa_kan_server::games::{chain_id_from_env, ledger_url_from_env, GameHost, DEFAULT_MAX_GAMES};
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::state::AppState;
use apa_kan_store::sqlite::SqliteStore;
use apa_kan_store::SystemClock;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let database_url = std::env::var("APA_KAN_DATABASE_URL").unwrap_or_else(|_| {
		format!("sqlite:{}", std::env::temp_dir().join("apa-kan-server.db").display())
	});
	let path = database_url.strip_prefix("sqlite:").unwrap_or(&database_url);
	let store = SqliteStore::connect_file(path).await?;
	let ledger = apa_kan_ledger::connect(&ledger_url_from_env()).await?;
	let chain = chain_id_from_env();
	tracing::info!(backend = ledger.backend(), %chain, "ledger ready");
	let state = AppState::new(store, Arc::new(SystemClock), Arc::new(NoopMailer));
	let keys = state.account_keys();
	let state = state.with_games(GameHost::new(ledger, keys, chain, DEFAULT_MAX_GAMES));

	let app = api::router(state);

	let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	let listener = TcpListener::bind(addr).await?;
	tracing::info!(%addr, "apa-kan-server listening");
	axum::serve(listener, app).await?;
	Ok(())
}
