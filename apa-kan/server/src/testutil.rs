use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use apa_kan_store::sqlite::SqliteStore;

use crate::api;
use crate::games::GameHost;
use crate::mailer::Mailer;
use crate::state::AppState;
use apa_kan_store::Clock;

pub async fn spawn_test_server(
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
	spawn_test_server_with_games(clock, mailer, GameHost::default()).await
}

/// Like [`spawn_test_server`], with a caller-chosen game host (e.g. one over a
/// durable store, to exercise restarts).
pub async fn spawn_test_server_with_games(
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
	games: GameHost,
) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
	let store = SqliteStore::connect_in_memory()
		.await
		.map_err(|e| std::io::Error::other(e.to_string()))?;
	let state = AppState::new(store, clock, mailer).with_games(games);
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
