#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Accounts over HTTP: every change is a signed `account.*` transaction on
//! `POST /v1/tx`; reads are views (`GET /v1/accounts/{id}`, `/keys`).

use std::sync::Arc;

use apa_kan_account::recovery::COOLDOWN_RECOVERY_SECS;
use apa_kan_account::{RecoveryKind, Seed};
use apa_kan_server::mailer::FakeMailer;
use apa_kan_server::testutil::{base_url, spawn_test_server};
use apa_kan_store::FakeClock;
use apa_kan_tx::account::email_proof;
use apa_kan_tx::{
	account_stream, game_stream, seed_commitment, SignedTx, TxPayload, TxSigner, GENESIS_PREV,
};
use apa_kan_view::{AccountView, KeyState, RecoveryStage};

const NOW: i64 = 1_700_000_000;

struct Api {
	client: reqwest::Client,
	root: String,
	chain: String,
}

impl Api {
	async fn new(root: String) -> Self {
		let client = reqwest::Client::new();
		let info: serde_json::Value = client
			.get(format!("{root}/v1/ledger"))
			.send()
			.await
			.unwrap()
			.json()
			.await
			.unwrap();
		assert!(info["operator"].as_str().is_some_and(|o| o.len() == 64));
		Self { client, root, chain: info["chain"].as_str().unwrap().to_string() }
	}

	async fn post(&self, tx: &SignedTx) -> (u16, serde_json::Value) {
		let res = self.client.post(format!("{}/v1/tx", self.root)).json(tx).send().await.unwrap();
		let status = res.status().as_u16();
		(status, res.json().await.unwrap())
	}

	async fn view(&self, account: &str) -> AccountView {
		let v: serde_json::Value = self
			.client
			.get(format!("{}/v1/accounts/{account}", self.root))
			.send()
			.await
			.unwrap()
			.json()
			.await
			.unwrap();
		serde_json::from_value(v).unwrap()
	}

	/// Sign `payload` on the account stream at its current head.
	async fn sign(&self, who: &TxSigner, account: &str, payload: TxPayload) -> SignedTx {
		let res = self.client.get(format!("{}/v1/accounts/{account}", self.root)).send().await;
		let (seq, head) = match res.unwrap().json::<AccountView>().await {
			Ok(v) => (v.ledger.seq, v.ledger.head),
			Err(_) => (0, GENESIS_PREV.to_string()),
		};
		who.sign(&self.chain, &format!("account/{account}"), seq, &head, payload)
			.unwrap()
	}

	async fn ok(&self, who: &TxSigner, account: &str, payload: TxPayload) -> AccountView {
		let tx = self.sign(who, account, payload).await;
		let (status, body) = self.post(&tx).await;
		assert_eq!(status, 200, "{body}");
		serde_json::from_value(body["view"].clone()).unwrap()
	}
}

fn key(n: u8) -> Seed {
	Seed::from_bytes([n; 32])
}

#[tokio::test]
async fn e2e_register_link_revoke_and_old_routes_gone() {
	let clock = Arc::new(FakeClock::new(NOW));
	let (addr, handle) =
		spawn_test_server(clock, Arc::new(FakeMailer::default())).await.expect("spawn");
	let api = Api::new(base_url(addr)).await;
	let genesis = TxSigner::from_seed(&key(20));
	let acct = genesis.account().to_hex();

	let v = api
		.ok(&genesis, &acct, TxPayload::AccountRegister { label: "Ana".into() })
		.await;
	assert_eq!(v.keys.len(), 1);
	// Registration projected the starter kit and rating.
	let profile: serde_json::Value = api
		.client
		.get(format!("{}/v1/accounts/{acct}", api.root))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(profile["tiles"].as_array().map(Vec::len), Some(15));
	assert_eq!(profile["elo_milli"], 1_500_000);
	assert_eq!(profile["label"], "Ana");

	// Link a phone: the phone co-signs the proof.
	let phone = TxSigner::for_account(&key(21), genesis.account());
	let link = |proof| TxPayload::AccountLinkKey {
		vk: phone.verifying_key().clone(),
		label: "phone".into(),
		proof,
	};
	let forged = TxSigner::from_seed(&key(22)).link_proof(&api.chain, &genesis.account());
	let tx = api.sign(&genesis, &acct, link(forged)).await;
	let (status, err) = api.post(&tx).await;
	assert_eq!(status, 401);
	assert_eq!(err["error"], "bad_signature");
	api.ok(&genesis, &acct, link(phone.link_proof(&api.chain, &genesis.account())))
		.await;

	let keys: Vec<serde_json::Value> = api
		.client
		.get(format!("{}/v1/accounts/{acct}/keys", api.root))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(keys.len(), 2);
	assert_eq!(keys[1]["vk"], hex::encode(&phone.verifying_key().0));
	assert_eq!(keys[1]["status"], "active");

	// A tampered body fails the signature check.
	let mut tampered = api
		.sign(&genesis, &acct, TxPayload::AccountRecoveryCancel { at_unix: NOW })
		.await;
	tampered.body.payload = TxPayload::AccountRevokeKey { vk: phone.verifying_key().clone() };
	assert_eq!(api.post(&tampered).await.0, 401);

	// The phone plays games for the account (key indirection via the fold)…
	let create = |who: &TxSigner, id: &str| {
		who.sign(
			&api.chain,
			&game_stream(id),
			0,
			GENESIS_PREV,
			TxPayload::GameCreate {
				seat: 1,
				label: "p".into(),
				seed_commit: seed_commitment(&game_stream(id), &[1; 32]),
				preview_pk: None,
			},
		)
		.unwrap()
	};
	assert_eq!(api.post(&create(&phone, "linked1")).await.0, 200);
	// …and once revoked, it can't.
	let v = api
		.ok(&genesis, &acct, TxPayload::AccountRevokeKey { vk: phone.verifying_key().clone() })
		.await;
	assert_eq!(v.keys[1].status, KeyState::Revoked);
	let (status, err) = api.post(&create(&phone, "linked2")).await;
	assert_eq!(status, 401);
	assert_eq!(err["error"], "key_not_authorized");
	// Can't revoke the last active key.
	let tx = api
		.sign(&genesis, &acct, TxPayload::AccountRevokeKey { vk: genesis.verifying_key().clone() })
		.await;
	let (status, err) = api.post(&tx).await;
	assert_eq!(status, 409);
	assert_eq!(err["error"], "rejected");

	// The old mutating account routes are gone.
	for path in ["/v1/accounts", &format!("/v1/accounts/{acct}/email")] {
		let res = api
			.client
			.post(format!("{}{path}", api.root))
			.json(&serde_json::json!({}))
			.send()
			.await
			.unwrap();
		assert!(res.status() == 404 || res.status() == 405, "{path}: {}", res.status());
	}
	let missing = api.client.get(format!("{}/v1/accounts/{}", api.root, "ab".repeat(32))).send();
	assert_eq!(missing.await.unwrap().status(), 404);
	handle.abort();
}

#[tokio::test]
async fn e2e_email_recovery_with_operator_challenge_and_cooldown() {
	let clock = Arc::new(FakeClock::new(NOW));
	let mailer = FakeMailer::default();
	let (addr, handle) =
		spawn_test_server(clock.clone(), Arc::new(mailer.clone())).await.expect("spawn");
	let api = Api::new(base_url(addr)).await;
	let genesis = TxSigner::from_seed(&key(30));
	let account = genesis.account();
	let acct = account.to_hex();
	api.ok(&genesis, &acct, TxPayload::AccountRegister { label: String::new() })
		.await;
	let v = api
		.ok(
			&genesis,
			&acct,
			TxPayload::AccountSetEmail {
				email: Some("ana@example.com".into()),
				recovery_opt_in: true,
				approve_via_email: false,
			},
		)
		.await;
	assert!(v.email_set);
	// The address is not in the view.
	assert!(!serde_json::to_string(&v).unwrap().contains("example.com"));

	// New device, lost the old one: request recovery with the pending key.
	let fresh = TxSigner::for_account(&key(31), account);
	let request = |email: &str| TxPayload::AccountRecoveryRequest {
		kind: RecoveryKind::FullRecovery,
		email_proof: email_proof(&account, fresh.verifying_key(), email),
		requested_at_unix: NOW,
	};
	let tx = api.sign(&fresh, &acct, request("eve@example.com")).await;
	assert_eq!(api.post(&tx).await.0, 403);
	let v = api.ok(&fresh, &acct, request("ana@example.com")).await;
	// The host committed the operator's challenge and mailed the token.
	let recovery = v.recovery.clone().unwrap();
	assert_eq!(recovery.stage, RecoveryStage::Challenged);
	assert_eq!(recovery.usable_at_unix, Some(NOW + COOLDOWN_RECOVERY_SECS));
	assert_eq!(v.ledger.seq, 4);
	let token = {
		let sent = mailer.sent.lock().unwrap();
		assert_eq!(sent.len(), 1);
		assert_eq!(sent[0].0, "ana@example.com");
		assert_eq!(sent[0].1, acct);
		sent[0].2.clone()
	};

	let tx = api
		.sign(&fresh, &acct, TxPayload::AccountRecoveryConfirm { token: "x".into() })
		.await;
	assert_eq!(api.post(&tx).await.0, 403);
	let v = api.ok(&fresh, &acct, TxPayload::AccountRecoveryConfirm { token }).await;
	assert_eq!(v.keys.last().unwrap().status, KeyState::PendingRecovery);

	// Cooldown: too early, then fine.
	let tx = api.sign(&fresh, &acct, TxPayload::AccountRecoveryFinish).await;
	let (status, err) = api.post(&tx).await;
	assert_eq!(status, 409);
	assert_eq!(err["error"], "too_early");
	clock.advance_secs(COOLDOWN_RECOVERY_SECS);
	let v = api.ok(&fresh, &acct, TxPayload::AccountRecoveryFinish).await;
	assert_eq!(v.keys.last().unwrap().status, KeyState::Active);
	// Full recovery revoked the old (genesis) key.
	assert_eq!(v.keys[0].status, KeyState::Revoked);
	assert!(v.recovery.is_none());
	assert_eq!(api.view(&acct).await, v);

	// A second request can be cancelled by any active key.
	let other = TxSigner::for_account(&key(32), account);
	let v = api
		.ok(
			&other,
			&acct,
			TxPayload::AccountRecoveryRequest {
				kind: RecoveryKind::FullRecovery,
				email_proof: email_proof(&account, other.verifying_key(), "ana@example.com"),
				requested_at_unix: NOW + COOLDOWN_RECOVERY_SECS,
			},
		)
		.await;
	assert!(v.recovery.is_some());
	let v = api
		.ok(
			&fresh,
			&acct,
			TxPayload::AccountRecoveryCancel { at_unix: NOW + COOLDOWN_RECOVERY_SECS },
		)
		.await;
	assert!(v.recovery.is_none());
	// …and then new requests wait out the retry cooldown.
	let again = api
		.sign(
			&other,
			&acct,
			TxPayload::AccountRecoveryRequest {
				kind: RecoveryKind::FullRecovery,
				email_proof: email_proof(&account, other.verifying_key(), "ana@example.com"),
				requested_at_unix: NOW + COOLDOWN_RECOVERY_SECS,
			},
		)
		.await;
	let (status, err) = api.post(&again).await;
	assert_eq!((status, err["error"].as_str()), (409, Some("too_early")));
	assert_eq!(account_stream(&account), v.ledger.stream);
	handle.abort();
}

#[tokio::test]
async fn e2e_admin_roles_by_operator_only() {
	let clock = Arc::new(FakeClock::new(NOW));
	let (addr, handle) =
		spawn_test_server(clock, Arc::new(FakeMailer::default())).await.expect("spawn");
	let api = Api::new(base_url(addr)).await;
	let alice = TxSigner::from_seed(&key(40));
	let grant = alice
		.sign(
			&api.chain,
			"admin/roles",
			0,
			GENESIS_PREV,
			TxPayload::AdminGrantRole { account: alice.account(), role: "admin".into() },
		)
		.unwrap();
	let (status, err) = api.post(&grant).await;
	assert_eq!(status, 403, "{err}");
	// Admin txs go to POST /v1/tx and are routed by topic: a roles payload
	// on another topic's stream, or an unknown topic, is refused.
	for stream in ["admin/catalog", "admin/Bad"] {
		let tx = alice
			.sign(
				&api.chain,
				stream,
				0,
				GENESIS_PREV,
				TxPayload::AdminGrantRole { account: alice.account(), role: "admin".into() },
			)
			.unwrap();
		assert_eq!(api.post(&tx).await.0, 400, "{stream}");
	}
	let roles: serde_json::Value = api
		.client
		.get(format!("{}/v1/admin/roles", api.root))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap();
	assert_eq!(roles["roles"], serde_json::json!({}));
	handle.abort();
}
