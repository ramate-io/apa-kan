#![allow(clippy::expect_used)]

//! HTTP round trip for hosted games: create, view, transact, conflict.

use std::sync::Arc;

use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
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
