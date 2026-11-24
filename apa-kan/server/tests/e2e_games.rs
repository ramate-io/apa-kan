#![allow(clippy::expect_used)]

//! HTTP round trip for the ledger: every mutation is a signed transaction on
//! `POST /v1/tx`; reads are views (`GET /v1/ledger`, `/v1/games/{id}`,
//! `/v1/games/{id}/txs`).

use std::sync::Arc;

use apa_kan_account::{AccountId, Seed};
use apa_kan_server::games::GameHost;
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server, spawn_test_server_with_games};
use apa_kan_store::SystemClock;
use apa_kan_tx::{game_stream, seed_commitment, SignedTx, TxPayload, TxSigner, GENESIS_PREV};
use apa_kan_view::{GamePhase, HostedGameView};

fn signer(n: u8) -> TxSigner {
	TxSigner::from_seed(&Seed::from_bytes([n; 32]))
}

async fn chain(client: &reqwest::Client, root: &str) -> String {
	let info: serde_json::Value = client
		.get(format!("{root}/v1/ledger"))
		.send()
		.await
		.expect("ledger")
		.json()
		.await
		.expect("json");
	assert_eq!(info["domain"], "apa-kan/tx/v1");
	info["chain"].as_str().expect("chain").to_string()
}

async fn post(client: &reqwest::Client, root: &str, tx: &SignedTx) -> reqwest::Response {
	client.post(format!("{root}/v1/tx")).json(tx).send().await.expect("tx")
}

async fn accept(client: &reqwest::Client, root: &str, tx: &SignedTx) -> HostedGameView {
	let res = post(client, root, tx).await;
	assert_eq!(res.status(), 200, "{:?}", res.text().await);
	let v: serde_json::Value = res.json().await.expect("json");
	serde_json::from_value(v["view"].clone()).expect("view")
}

fn next(who: &TxSigner, v: &HostedGameView, payload: TxPayload) -> SignedTx {
	who.sign(&v.ledger.chain, &v.ledger.stream, v.ledger.seq, &v.ledger.head, payload)
		.expect("sign")
}

fn to_move<'a>(v: &HostedGameView, a: &'a TxSigner, b: &'a TxSigner) -> &'a TxSigner {
	let seat = v.game.as_ref().expect("game").to_move;
	if v.seat_of_account(&a.account().to_hex()) == Some(seat) {
		a
	} else {
		b
	}
}

fn play_first(v: &HostedGameView, a: &TxSigner, b: &TxSigner) -> SignedTx {
	let action = v.game.as_ref().expect("game").legal[0];
	next(to_move(v, a, b), v, TxPayload::GameMove { action })
}

async fn open_game(
	client: &reqwest::Client,
	root: &str,
	id: &str,
	a: &TxSigner,
	b: &TxSigner,
) -> HostedGameView {
	let chain = chain(client, root).await;
	let create = a
		.sign(
			&chain,
			&game_stream(id),
			0,
			GENESIS_PREV,
			TxPayload::GameCreate {
				seat: 1,
				label: "ana".into(),
				seed_commit: seed_commitment(&game_stream(id), &SALT),
				preview_pk: None,
			},
		)
		.expect("sign");
	let v = accept(client, root, &create).await;
	assert_eq!(v.phase, GamePhase::Open);
	assert!(v.game.is_none());
	let v = accept(
		client,
		root,
		&next(b, &v, TxPayload::GameJoin { label: "bo".into(), preview_pk: None }),
	)
	.await;
	assert_eq!(v.phase, GamePhase::Revealing);
	accept(client, root, &next(a, &v, TxPayload::GameReveal { salt: hex::encode(SALT) })).await
}

const SALT: [u8; 32] = [3; 32];

#[tokio::test]
async fn e2e_signed_create_join_move_resign() {
	let (addr, handle) = spawn_test_server(Arc::new(SystemClock), Arc::new(NoopMailer))
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);
	let (a, b) = (signer(31), signer(32));

	let mut v = open_game(&client, &root, "e2e1", &a, &b).await;
	assert_eq!(v.phase, GamePhase::Playing);
	for _ in 0..4 {
		v = accept(&client, &root, &play_first(&v, &a, &b)).await;
	}
	let got: HostedGameView = client
		.get(format!("{root}/v1/games/e2e1"))
		.send()
		.await
		.expect("get")
		.json()
		.await
		.expect("view");
	assert_eq!(got, v);
	assert_eq!(
		got.game.as_ref().expect("game").replay().expect("replay").turn_count(),
		got.game.as_ref().expect("game").turn
	);

	// Replaying an accepted transaction: 409 stale_seq.
	let stale = {
		let log: Vec<serde_json::Value> = client
			.get(format!("{root}/v1/games/e2e1/txs?from=3"))
			.send()
			.await
			.expect("txs")
			.json()
			.await
			.expect("json");
		assert_eq!(log.len(), 4);
		serde_json::from_value::<SignedTx>(log[0]["tx"].clone()).expect("tx")
	};
	let res = post(&client, &root, &stale).await;
	assert_eq!(res.status(), 409);
	let err: serde_json::Value = res.json().await.expect("json");
	assert_eq!(err["error"], "stale_seq");
	assert_eq!(err["current"], 7);

	// Tampered body: 401 bad_signature.
	let mut forged = play_first(&v, &a, &b);
	forged.body.payload = TxPayload::GameResign;
	let res = post(&client, &root, &forged).await;
	assert_eq!(res.status(), 401);

	// Resign ends it.
	let v = accept(&client, &root, &next(&a, &v, TxPayload::GameResign)).await;
	assert_eq!(v.phase, GamePhase::Finished);

	let missing = client.get(format!("{root}/v1/games/nope")).send().await.expect("missing");
	assert_eq!(missing.status(), 404);
	// Old mutation routes are gone.
	let old = client
		.post(format!("{root}/v1/games"))
		.json(&serde_json::json!({}))
		.send()
		.await
		.expect("old");
	assert!(old.status() == 404 || old.status() == 405);

	let pre = client
		.request(reqwest::Method::OPTIONS, format!("{root}/v1/tx"))
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

/// Registered accounts: their genesis key acts; a key never linked to the
/// account does not, even with a valid signature.
#[tokio::test]
async fn e2e_registered_account_key_indirection() {
	let (addr, handle) = spawn_test_server(Arc::new(SystemClock), Arc::new(NoopMailer))
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);
	let genesis = signer(41);
	let res = client
		.post(format!("{root}/v1/accounts"))
		.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&genesis.verifying_key().0) }))
		.send()
		.await
		.expect("account");
	assert!(res.status().is_success());
	let chain = chain(&client, &root).await;
	let create = |who: &TxSigner, id: &str| {
		who.sign(
			&chain,
			&game_stream(id),
			0,
			GENESIS_PREV,
			TxPayload::GameCreate {
				seat: 1,
				label: "x".into(),
				seed_commit: seed_commitment(&game_stream(id), &SALT),
				preview_pk: None,
			},
		)
		.expect("sign")
	};
	accept(&client, &root, &create(&genesis, "reg1")).await;
	let account: AccountId = genesis.account();
	let unlinked = TxSigner::for_account(&Seed::from_bytes([42; 32]), account);
	let res = post(&client, &root, &create(&unlinked, "reg2")).await;
	assert_eq!(res.status(), 401);
	let err: serde_json::Value = res.json().await.expect("json");
	assert_eq!(err["error"], "key_not_authorized");
	handle.abort();
}

/// Acknowledged transactions are on disk: a fresh server over the same SQLite
/// ledger rebuilds the game by replay and keeps going.
#[tokio::test]
async fn e2e_game_survives_server_restart() {
	let dir = std::env::temp_dir().join(format!("apakan-e2e-restart-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).expect("dir");
	let url = format!("sqlite:{}", dir.join("ledger.db").display());
	let client = reqwest::Client::new();
	let (a, b) = (signer(51), signer(52));
	let host = |ledger| {
		GameHost::new(ledger, Arc::new(apa_kan_server::games::GenesisOnly), "restart", 100)
	};

	let first = apa_kan_ledger::connect(&url).await.expect("ledger");
	let (addr, handle) =
		spawn_test_server_with_games(Arc::new(SystemClock), Arc::new(NoopMailer), host(first))
			.await
			.expect("spawn");
	let root = base_url(addr);
	let mut v = open_game(&client, &root, "durable", &a, &b).await;
	for _ in 0..3 {
		v = accept(&client, &root, &play_first(&v, &a, &b)).await;
	}
	handle.abort();
	let _ = handle.await;

	let second = apa_kan_ledger::connect(&url).await.expect("reopen");
	let (addr, handle) =
		spawn_test_server_with_games(Arc::new(SystemClock), Arc::new(NoopMailer), host(second))
			.await
			.expect("respawn");
	let root = base_url(addr);
	let after: HostedGameView = client
		.get(format!("{root}/v1/games/durable"))
		.send()
		.await
		.expect("get")
		.json()
		.await
		.expect("view");
	assert_eq!(after, v);
	accept(&client, &root, &play_first(&after, &a, &b)).await;
	handle.abort();
	let _ = std::fs::remove_dir_all(dir);
}
