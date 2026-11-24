#![allow(clippy::expect_used)]

use std::sync::Arc;

use apa_kan_account::Seed;
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
use apa_kan_store::SystemClock;
use apa_kan_tx::{account_stream, TxPayload, TxSigner, GENESIS_PREV};

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

	// Registration is a signed `account.register` on `POST /v1/tx`.
	let info: serde_json::Value = client
		.get(format!("{root}/v1/ledger"))
		.send()
		.await
		.expect("ledger")
		.json()
		.await
		.expect("json");
	let chain = info["chain"].as_str().expect("chain");
	let key = TxSigner::from_seed(&Seed::from_bytes([11u8; 32]));
	let account = key.account();
	let register = key
		.sign(
			chain,
			&account_stream(&account),
			0,
			GENESIS_PREV,
			TxPayload::AccountRegister { label: "smoke".into() },
		)
		.expect("sign");
	let res = client
		.post(format!("{root}/v1/tx"))
		.json(&register)
		.send()
		.await
		.expect("register");
	assert!(res.status().is_success(), "{:?}", res.text().await);
	let account_id = account.to_hex();

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
