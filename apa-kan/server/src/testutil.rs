use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use apa_kan_store::sqlite::SqliteStore;

use crate::api;
use crate::mailer::Mailer;
use crate::state::AppState;
use apa_kan_ledger::LedgerStore;
use apa_kan_store::Clock;
use apa_kan_tx::TxSigner;

pub async fn spawn_test_server(
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
	let store = SqliteStore::connect_in_memory()
		.await
		.map_err(|e| std::io::Error::other(e.to_string()))?;
	serve(AppState::new(store, clock, mailer)).await
}

/// Like [`spawn_test_server`], over a caller-chosen ledger and chain (e.g. a
/// durable one, to exercise restarts) with a fixed operator key.
pub async fn spawn_test_server_with_ledger(
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
	ledger: Arc<dyn LedgerStore>,
	chain: &str,
	operator: TxSigner,
) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
	let store = SqliteStore::connect_in_memory()
		.await
		.map_err(|e| std::io::Error::other(e.to_string()))?;
	serve(AppState::with_ledger(
		store,
		clock,
		mailer,
		ledger,
		chain,
		Some(operator),
		crate::games::DEFAULT_MAX_GAMES,
	))
	.await
}

async fn serve(state: AppState) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
	let app = api::router(state);

	let listener = TcpListener::bind("127.0.0.1:0").await?;
	let addr = listener.local_addr()?;
	let handle = tokio::spawn(async move {
		axum::serve(listener, app).await.ok();
	});
	Ok((addr, handle))
}

pub fn base_url(addr: SocketAddr) -> String {
	format!("http://{addr}")
}
