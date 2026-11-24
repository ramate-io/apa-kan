#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::Duration;

use apa_kan_account::Seed;
use apa_kan_tx::{account_stream, TxPayload, TxSigner, GENESIS_PREV};

fn server_binary() -> PathBuf {
	std::env::var("CARGO_BIN_EXE_apa_kan_server")
		.or_else(|_| std::env::var("CARGO_BIN_EXE_apa-kan-server"))
		.map(PathBuf::from)
		.unwrap_or_else(|_| {
			PathBuf::from(env!("CARGO_MANIFEST_DIR"))
				.join("../..")
				.join("target")
				.join("debug")
				.join("apa-kan-server")
		})
}

#[tokio::test]
async fn smoke_real_server_binary_health_and_account() {
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
	let port = listener.local_addr().expect("addr").port();
	drop(listener);

	let db_path = std::env::temp_dir().join(format!("apa-kan-smoke-{}.db", std::process::id()));
	let _ = std::fs::remove_file(&db_path);

	let mut child = std::process::Command::new(server_binary())
		.env("PORT", port.to_string())
		.env("APA_KAN_DATABASE_URL", format!("sqlite:{}", db_path.display()))
		.env("APAKAN_OPERATOR_SEED", "07".repeat(32))
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.spawn()
		.expect("spawn apa-kan-server");

	let client = reqwest::Client::new();
	let root = format!("http://127.0.0.1:{port}");

	for _ in 0..40 {
		if client
			.get(format!("{root}/health"))
			.send()
			.await
			.is_ok_and(|r| r.status().is_success())
		{
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}

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
	let key = TxSigner::from_seed(&Seed::from_bytes([99u8; 32]));
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
		.expect("profile")
		.json()
		.await
		.expect("json");
	assert_eq!(profile["tiles"].as_array().map(|a| a.len()), Some(15));

	let _ = child.kill();
	let _ = child.wait();
	let _ = std::fs::remove_file(db_path);
}
