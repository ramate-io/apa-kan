#![allow(clippy::expect_used)]

//! HTTP round trip for hosted games: create, view, transact, conflict.

use std::sync::Arc;

use apa_kan_server::games::GameHost;
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server, spawn_test_server_with_games};
use apa_kan_store::SystemClock;
use apa_kan_view::GameView;

#[tokio::test]
async fn e2e_create_view_and_transact() {
	let (addr, handle) = spawn_test_server(Arc::new(SystemClock), Arc::new(NoopMailer))
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);

	let created: serde_json::Value = client
		.post(format!("{root}/v1/games"))
		.json(&serde_json::json!({ "seed": 21, "labels": ["ana", "bo"] }))
		.send()
		.await
		.expect("create")
		.json()
		.await
		.expect("json");
	let id = created["game_id"].as_str().expect("id").to_string();
	let tokens = [
		created["seats"][0]["token"].as_str().expect("t1").to_string(),
		created["seats"][1]["token"].as_str().expect("t2").to_string(),
	];

	let get = client.get(format!("{root}/v1/games/{id}")).send().await.expect("get");
	assert_eq!(get.status(), 200);
	let mut view: GameView = get.json().await.expect("view");
	assert_eq!(view.seq, 0);
	assert_eq!(view.seats[1].label, "bo");

	for _ in 0..4 {
		let seat = view.to_move;
		let body = serde_json::json!({
			"game_id": id,
			"seq": view.seq,
			"seat": seat,
			"action": view.legal[0],
			"auth": { "type": "seat_token", "token": tokens[usize::from(seat - 1)] },
		});
		let res = client
			.post(format!("{root}/v1/games/{id}/transactions"))
			.json(&body)
			.send()
			.await
			.expect("tx");
		assert_eq!(res.status(), 200);
		let v: serde_json::Value = res.json().await.expect("json");
		view = serde_json::from_value(v["view"].clone()).expect("view");
	}
	assert_eq!(view.seq, 4);
	let replayed = view.replay().expect("replay");
	assert_eq!(replayed.turn_count(), view.turn);

	// Re-sending an old seq is a 409 with a typed error body.
	let stale = serde_json::json!({
		"game_id": id,
		"seq": 0,
		"seat": view.to_move,
		"action": view.legal[0],
		"auth": { "type": "seat_token", "token": tokens[usize::from(view.to_move - 1)] },
	});
	let res = client
		.post(format!("{root}/v1/games/{id}/transactions"))
		.json(&stale)
		.send()
		.await
		.expect("stale");
	assert_eq!(res.status(), 409);
	let err: serde_json::Value = res.json().await.expect("json");
	assert_eq!(err["error"], "stale_seq");
	assert_eq!(err["current"], 4);

	let missing = client.get(format!("{root}/v1/games/nope")).send().await.expect("missing");
	assert_eq!(missing.status(), 404);

	// CORS preflight for the web client's own domain.
	let pre = client
		.request(reqwest::Method::OPTIONS, format!("{root}/v1/games/{id}/transactions"))
		.header("Origin", "https://play.example")
		.header("Access-Control-Request-Method", "POST")
		.header("Access-Control-Request-Headers", "content-type")
		.send()
		.await
		.expect("preflight");
	assert!(pre.status().is_success());
	assert!(pre.headers().get("access-control-allow-origin").is_some());

	handle.abort();
}

async fn post_move(
	client: &reqwest::Client,
	root: &str,
	view: &GameView,
	tokens: &[String; 2],
) -> reqwest::Response {
	let seat = view.to_move;
	let body = serde_json::json!({
		"game_id": view.id,
		"seq": view.seq,
		"seat": seat,
		"action": view.legal[0],
		"auth": { "type": "seat_token", "token": tokens[usize::from(seat - 1)] },
	});
	client
		.post(format!("{root}/v1/games/{}/transactions", view.id))
		.json(&body)
		.send()
		.await
		.expect("tx")
}

/// Acknowledged moves are on disk: a fresh server over the same SQLite file
/// picks the game up at the same move number and keeps going.
#[tokio::test]
async fn e2e_game_survives_server_restart() {
	let dir = std::env::temp_dir().join(format!("apakan-e2e-restart-{}", std::process::id()));
	std::fs::create_dir_all(&dir).expect("dir");
	let url = format!("sqlite:{}", dir.join("games.db").display());
	let client = reqwest::Client::new();

	let first = apa_kan_game_store::connect(&url).await.expect("store");
	let (addr, handle) = spawn_test_server_with_games(
		Arc::new(SystemClock),
		Arc::new(NoopMailer),
		GameHost::new(first, 100),
	)
	.await
	.expect("spawn");
	let root = base_url(addr);
	let created: serde_json::Value = client
		.post(format!("{root}/v1/games"))
		.json(&serde_json::json!({ "seed": 5 }))
		.send()
		.await
		.expect("create")
		.json()
		.await
		.expect("json");
	let tokens = [
		created["seats"][0]["token"].as_str().expect("t1").to_string(),
		created["seats"][1]["token"].as_str().expect("t2").to_string(),
	];
	let mut view: GameView = serde_json::from_value(created["view"].clone()).expect("view");
	for _ in 0..3 {
		let res = post_move(&client, &root, &view, &tokens).await;
		assert_eq!(res.status(), 200);
		let v: serde_json::Value = res.json().await.expect("json");
		view = serde_json::from_value(v["view"].clone()).expect("view");
	}
	handle.abort();
	let _ = handle.await;

	let second = apa_kan_game_store::connect(&url).await.expect("reopen");
	let (addr, handle) = spawn_test_server_with_games(
		Arc::new(SystemClock),
		Arc::new(NoopMailer),
		GameHost::new(second, 100),
	)
	.await
	.expect("respawn");
	let root = base_url(addr);
	let after: GameView = client
		.get(format!("{root}/v1/games/{}", view.id))
		.send()
		.await
		.expect("get")
		.json()
		.await
		.expect("view");
	assert_eq!(serde_json::to_value(&after).expect("a"), serde_json::to_value(&view).expect("b"));
	let res = post_move(&client, &root, &after, &tokens).await;
	assert_eq!(res.status(), 200);
	handle.abort();
	let _ = std::fs::remove_dir_all(dir);
}
