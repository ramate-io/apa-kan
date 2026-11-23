use std::sync::Arc;

use apa_kan_account::auth::{AuthChallengePayload, AuthChallengeResponse};
use apa_kan_account::crypto::{generate_keypair, sign_bytes, Seed};
use apa_kan_server::mailer::NoopMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
use apa_kan_store::FakeClock;

const NOW: i64 = 1_700_000_000;

async fn sign_in(client: &reqwest::Client, root: &str, seed: [u8; 32]) -> (String, String) {
	let (sk, vk) = generate_keypair(&Seed::from_bytes(seed));
	let create: serde_json::Value = client
		.post(format!("{root}/v1/accounts"))
		.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&vk.0) }))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	let account_id = create["account_id"].as_str().unwrap().to_string();
	let challenge: serde_json::Value = client
		.post(format!("{root}/v1/auth/challenge"))
		.json(&serde_json::json!({
			"account_id": account_id,
			"vk_hex": hex::encode(&vk.0),
		}))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	let payload: AuthChallengePayload =
		serde_json::from_value(challenge["payload"].clone()).unwrap();
	let id = payload.message_id().unwrap();
	let response = AuthChallengeResponse { payload, signature: sign_bytes(&sk, &id) };
	let session: serde_json::Value = client
		.post(format!("{root}/v1/auth/session"))
		.json(&response)
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(session["is_admin"], true);
	(account_id, session["token"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn e2e_admin_catalog_publish_and_tile_filters() {
	let clock = Arc::new(FakeClock::new(NOW));
	let (addr, handle) = spawn_test_server(clock, Arc::new(NoopMailer)).await.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);
	let (_account, token) = sign_in(&client, &root, [42u8; 32]).await;

	let auth =
		|req: reqwest::RequestBuilder| req.header("Authorization", format!("Bearer {token}"));

	let vocab: serde_json::Value = auth(client.get(format!("{root}/v1/admin/vocabulary")))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert!(vocab.as_array().unwrap().iter().any(|v| v["key"] == "color.teal"));

	let chars: serde_json::Value = auth(client.get(format!("{root}/v1/admin/characters")))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert!(chars.as_array().unwrap().len() >= 26);

	let created = auth(client.post(format!("{root}/v1/admin/characters")))
		.json(&serde_json::json!({ "character_id": "newface", "display_name": "New Face" }))
		.send()
		.await
		.unwrap();
	assert_eq!(created.status(), 200);

	let embed = auth(client.put(format!("{root}/v1/admin/characters/newface/embedding")))
		.json(&serde_json::json!({
			"row": { "id": "newface", "color": { "maroon": -1.5 }, "type": { "zag": 1.2 } }
		}))
		.send()
		.await
		.unwrap();
	assert_eq!(embed.status(), 200);

	let zero = auth(client.put(format!("{root}/v1/admin/characters/newface/embedding")))
		.json(&serde_json::json!({ "row": { "id": "newface" } }))
		.send()
		.await
		.unwrap();
	assert_eq!(zero.status(), 400);

	let pass_before: serde_json::Value = auth(client.get(format!("{root}/v1/admin/pass")))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(pass_before["published_version"], 1);
	assert_eq!(pass_before["draft_version"], 2);

	let published =
		auth(client.post(format!("{root}/v1/admin/pass/publish"))).send().await.unwrap();
	assert_eq!(published.status(), 200);
	let pass_after: serde_json::Value = published.json().await.unwrap();
	assert_eq!(pass_after["published_version"], 2);

	let preview: serde_json::Value = auth(client.post(format!("{root}/v1/admin/preview")))
		.json(&serde_json::json!({
			"row": { "id": "preview", "color": { "teal": 2.0 } },
			"k": 5
		}))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert!(!preview.as_array().unwrap().is_empty());

	let tiles: serde_json::Value = auth(client.get(format!("{root}/v1/admin/tiles")))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	let tiles = tiles.as_array().unwrap();
	assert!(!tiles.is_empty());
	let character = tiles[0]["character_id"].as_str().unwrap();
	let filtered: serde_json::Value =
		auth(client.get(format!("{root}/v1/admin/tiles?character={character}")))
			.send()
			.await
			.unwrap()
			.json()
			.await
			.unwrap();
	assert!(filtered.as_array().unwrap().iter().all(|t| t["character_id"] == character));

	handle.abort();
}
