use std::sync::Arc;

use apa_kan_account::canonical::to_canonical_json;
use apa_kan_account::crypto::{generate_keypair, message_id, sign_bytes, Seed, SignatureBytes};
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{
	LinkApproval, LinkApprovalPayload, LinkRequest, LinkRequestPayload,
};
use apa_kan_account::recovery::COOLDOWN_RECOVERY_SECS;
use apa_kan_server::mailer::FakeMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
use apa_kan_store::FakeClock;

const NOW: i64 = 1_700_000_000;

#[tokio::test]
async fn e2e_link_two_keys_and_reject_bad_signature() {
	let clock = Arc::new(FakeClock::new(NOW));
	let mailer = Arc::new(FakeMailer::default());
	let (addr, handle) = spawn_test_server(clock.clone(), mailer)
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);

	let (genesis_sk, genesis_vk) = generate_keypair(&Seed::from_bytes([20u8; 32]));
	let create: serde_json::Value = client
		.post(format!("{root}/v1/accounts"))
		.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&genesis_vk.0) }))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	let account_hex = create["account_id"].as_str().unwrap().to_string();
	let account_id = AccountId::from_hex(&account_hex).unwrap();

	let (phone_sk, phone_vk) = generate_keypair(&Seed::from_bytes([21u8; 32]));
	let payload = LinkRequestPayload::new(
		account_id,
		phone_vk.clone(),
		[4u8; 32],
		NOW,
		"phone",
	);
	let request_id = LinkRequest::compute_request_id(&payload).unwrap();
	let req = LinkRequest {
		request_id,
		payload: payload.clone(),
		new_key_sig: sign_bytes(&phone_sk, &request_id),
	};

	let link_resp: serde_json::Value = client
		.post(format!("{root}/v1/accounts/{account_hex}/link-requests"))
		.json(&serde_json::json!({ "request": req }))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(link_resp["request_id"].as_str().unwrap(), hex::encode(request_id));

	let approval_payload = LinkApprovalPayload::new(
		request_id,
		account_id,
		genesis_vk.clone(),
		0,
		NOW,
	);
	let canonical = to_canonical_json(&approval_payload).unwrap();
	let msg_id = message_id(canonical.as_bytes(), &genesis_vk);
	let good_approval = LinkApproval {
		payload: approval_payload.clone(),
		approver_sig: sign_bytes(&genesis_sk, &msg_id),
	};

	client
		.post(format!("{root}/v1/accounts/{account_hex}/link-requests/approve"))
		.json(&serde_json::json!({ "approval": good_approval }))
		.send()
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let keys: Vec<serde_json::Value> = client
		.get(format!("{root}/v1/accounts/{account_hex}/keys"))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(keys.len(), 2);
	let vks: Vec<_> = keys
		.iter()
		.map(|k| k["vk_hex"].as_str().unwrap())
		.collect();
	assert!(vks.contains(&hex::encode(&genesis_vk.0).as_str()));
	assert!(vks.contains(&hex::encode(&phone_vk.0).as_str()));

	let bad_approval = LinkApproval {
		payload: approval_payload,
		approver_sig: SignatureBytes(vec![0u8; 64]),
	};
	let bad = client
		.post(format!("{root}/v1/accounts/{account_hex}/link-requests/approve"))
		.json(&serde_json::json!({ "approval": bad_approval }))
		.send()
		.await
		.unwrap();
	assert_eq!(bad.status(), 401);

	handle.abort();
}

#[tokio::test]
async fn e2e_recovery_email_cooldown_with_fake_clock_and_mailer() {
	let clock = Arc::new(FakeClock::new(NOW));
	let mailer = Arc::new(FakeMailer::default());
	let (addr, handle) = spawn_test_server(clock.clone(), mailer.clone())
		.await
		.expect("spawn");
	let client = reqwest::Client::new();
	let root = base_url(addr);

	let (_sk, vk) = generate_keypair(&Seed::from_bytes([30u8; 32]));
	let create: serde_json::Value = client
		.post(format!("{root}/v1/accounts"))
		.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&vk.0) }))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	let account_hex = create["account_id"].as_str().unwrap();

	client
		.post(format!("{root}/v1/accounts/{account_hex}/email"))
		.json(&serde_json::json!({
			"email": "player@example.test",
			"recovery_opt_in": true,
			"approve_via_email": true,
		}))
		.send()
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let (recovery_sk, recovery_vk) = generate_keypair(&Seed::from_bytes([31u8; 32]));
	client
		.post(format!("{root}/v1/accounts/{account_hex}/recovery/start"))
		.json(&serde_json::json!({
			"pending_vk_hex": hex::encode(&recovery_vk.0),
			"email": "player@example.test",
		}))
		.send()
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let sent = mailer.sent.lock().unwrap();
	let token = sent.last().expect("mail").2.clone();
	drop(sent);

	client
		.post(format!("{root}/v1/accounts/{account_hex}/recovery/confirm"))
		.json(&serde_json::json!({ "token": token }))
		.send()
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let early = client
		.post(format!("{root}/v1/accounts/{account_hex}/recovery/finish"))
		.send()
		.await
		.unwrap();
	assert_eq!(early.status(), 410);

	clock.advance_secs(COOLDOWN_RECOVERY_SECS);
	client
		.post(format!("{root}/v1/accounts/{account_hex}/recovery/finish"))
		.send()
		.await
		.unwrap()
		.error_for_status()
		.unwrap();

	let keys: Vec<serde_json::Value> = client
		.get(format!("{root}/v1/accounts/{account_hex}/keys"))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert!(keys.iter().any(|k| k["vk_hex"].as_str() == Some(&hex::encode(&recovery_vk.0))));
	let _ = recovery_sk;

	handle.abort();
}