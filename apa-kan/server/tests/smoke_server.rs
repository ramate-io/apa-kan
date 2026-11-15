use std::sync::Arc;

use apa_kan_account::crypto::{generate_keypair, Seed};
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
use apa_kan_store::SystemClock;

#[tokio::test]
async fn smoke_health_and_account_round_trip() {
	let (_addr, handle) = spawn_test_server(Arc::new(SystemClock), Arc::new(NoopMailer))
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(_addr);

	let health: serde_json::Value = client
		.get(format!("{root}/health"))
		.send()
		.await
		.expect("health")
		.json()
		.await
		.expect("json");
	assert_eq!(health["status"], "ok");

	let (_sk, vk) = generate_keypair(&Seed::from_bytes([11u8; 32]));
	let create: serde_json::Value = client
		.post(format!("{root}/v1/accounts"))
		.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&vk.0) }))
		.send()
		.await
		.expect("create")
		.json()
		.await
		.expect("json");
	let account_id = create["account_id"].as_str().expect("id");

	let profile: serde_json::Value = client
		.get(format!("{root}/v1/accounts/{account_id}"))
		.send()
		.await
		.expect("get")
		.json()
		.await
		.expect("json");
	assert_eq!(profile["tiles"].as_array().map(|a| a.len()), Some(15));
	assert_eq!(profile["elo_milli"], 1_500_000);

	handle.abort();
}