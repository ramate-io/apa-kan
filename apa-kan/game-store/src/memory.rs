//! In-process backend. Same CAS semantics as the SQL stores; nothing
//! survives a restart.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::*;

#[derive(Default)]
pub struct MemoryGameStore {
	games: Mutex<HashMap<String, LoadedGame>>,
}

#[async_trait]
impl GameStore for MemoryGameStore {
	async fn create(&self, game: &StoredGame) -> Result<()> {
		validate_new(game)?;
		let mut games = self.games.lock().await;
		if games.contains_key(&game.game_id) {
			return Err(GameStoreError::Exists);
		}
		games.insert(game.game_id.clone(), LoadedGame { game: game.clone(), log: Vec::new() });
		Ok(())
	}

	async fn append(&self, tx: &LoggedTx) -> Result<()> {
		validate_tx(tx)?;
		let mut games = self.games.lock().await;
		let g = games.get_mut(&tx.game_id).ok_or(GameStoreError::NotFound)?;
		let current = g.seq();
		if tx.seq != current {
			return Err(GameStoreError::Conflict { current, got: tx.seq });
		}
		g.log.push(tx.clone());
		Ok(())
	}

	async fn load(&self, game_id: &str) -> Result<LoadedGame> {
		self.games.lock().await.get(game_id).cloned().ok_or(GameStoreError::NotFound)
	}

	async fn head_seq(&self, game_id: &str) -> Result<u64> {
		self.games
			.lock()
			.await
			.get(game_id)
			.map(LoadedGame::seq)
			.ok_or(GameStoreError::NotFound)
	}

	async fn count(&self) -> Result<u64> {
		Ok(self.games.lock().await.len() as u64)
	}

	fn backend(&self) -> &'static str {
		"memory"
	}
}
