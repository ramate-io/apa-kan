use std::net::SocketAddr;
use std::sync::Arc;

use apa_kan_server::admin;
use apa_kan_server::api;
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::state::AppState;
use apa_kan_store::sqlite::SqliteStore;
use apa_kan_store::{Store, SystemClock};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let database_url = std::env::var("APA_KAN_DATABASE_URL").unwrap_or_else(|_| {
		format!("sqlite:{}", std::env::temp_dir().join("apa-kan-server.db").display())
	});
	let store: Arc<dyn Store> =
		if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
			#[cfg(feature = "postgres")]
			{
				Arc::new(apa_kan_store::postgres::PostgresStore::connect(&database_url).await?)
			}
			#[cfg(not(feature = "postgres"))]
			{
				return Err("postgres feature disabled".into());
			}
		} else {
			let path = database_url.strip_prefix("sqlite:").unwrap_or(&database_url);
			Arc::new(SqliteStore::connect_file(path).await?)
		};
	let state = AppState::new(store, Arc::new(SystemClock), Arc::new(NoopMailer));
	admin::seed_if_needed(&state).await?;

	let app = api::router(state);

	let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	let listener = TcpListener::bind(addr).await?;
	tracing::info!(%addr, "apa-kan-server listening");
	axum::serve(listener, app).await?;
	Ok(())
}
