use apa_kan_account::auth::{AuthChallengePayload, AuthChallengeResponse};
use apa_kan_account::crypto::{generate_keypair, sign_bytes, Seed};
use apa_kan_account::id::AccountId;
use reqwest::multipart;
use serde_json::Value;

#[derive(Clone)]
pub struct Api {
	pub root: String,
	http: reqwest::Client,
}

impl Api {
	pub fn new(root: String) -> Self {
		Self { root, http: reqwest::Client::new() }
	}

	pub async fn ensure_account(
		&self,
		seed: &Seed,
	) -> Result<(String, apa_kan_account::crypto::VerifyingKeyBytes), String> {
		let (_sk, vk) = generate_keypair(seed);
		let account_id = AccountId::from_genesis_vk(&vk).to_hex();
		let resp = self
			.http
			.post(format!("{}/v1/accounts", self.root))
			.json(&serde_json::json!({ "genesis_vk_hex": hex::encode(&vk.0) }))
			.send()
			.await
			.map_err(|e| e.to_string())?;
		if resp.status() == 409 || resp.status().is_success() {
			return Ok((account_id, vk));
		}
		Err(format!("create account: {}", resp.status()))
	}

	pub async fn sign_in(&self, seed: &Seed) -> Result<(String, String, bool), String> {
		let (sk, vk) = generate_keypair(seed);
		let (account_id, vk) = match self.ensure_account(seed).await {
			Ok(pair) => pair,
			Err(_) => (AccountId::from_genesis_vk(&vk).to_hex(), vk),
		};
		let challenge: Value = self
			.http
			.post(format!("{}/v1/auth/challenge", self.root))
			.json(&serde_json::json!({
				"account_id": account_id,
				"vk_hex": hex::encode(&vk.0),
			}))
			.send()
			.await
			.map_err(|e| e.to_string())?
			.json()
			.await
			.map_err(|e| e.to_string())?;
		let payload: AuthChallengePayload =
			serde_json::from_value(challenge["payload"].clone()).map_err(|e| e.to_string())?;
		let id = payload.message_id().map_err(|e| e.to_string())?;
		let response = AuthChallengeResponse { payload, signature: sign_bytes(&sk, &id) };
		let session: Value = self
			.http
			.post(format!("{}/v1/auth/session", self.root))
			.json(&response)
			.send()
			.await
			.map_err(|e| e.to_string())?
			.json()
			.await
			.map_err(|e| e.to_string())?;
		let token = session["token"].as_str().ok_or("no token")?.to_string();
		let is_admin = session["is_admin"].as_bool().unwrap_or(false);
		Ok((account_id, token, is_admin))
	}

	pub async fn get(&self, token: &str, path: &str) -> Result<Value, String> {
		self.http
			.get(format!("{}{path}", self.root))
			.header("Authorization", format!("Bearer {token}"))
			.send()
			.await
			.map_err(|e| e.to_string())?
			.json()
			.await
			.map_err(|e| e.to_string())
	}

	pub async fn send_json(
		&self,
		token: &str,
		method: reqwest::Method,
		path: &str,
		body: &Value,
	) -> Result<Value, String> {
		let resp = self
			.http
			.request(method, format!("{}{path}", self.root))
			.header("Authorization", format!("Bearer {token}"))
			.json(body)
			.send()
			.await
			.map_err(|e| e.to_string())?;
		let status = resp.status();
		let val: Value = resp.json().await.unwrap_or(Value::Null);
		if !status.is_success() {
			return Err(val.to_string());
		}
		Ok(val)
	}

	pub async fn upload_png(&self, token: &str, id: &str, bytes: Vec<u8>) -> Result<Value, String> {
		let part = multipart::Part::bytes(bytes)
			.file_name("character.png")
			.mime_str("image/png")
			.map_err(|e| e.to_string())?;
		let form = multipart::Form::new().part("image", part);
		self.http
			.put(format!("{}/v1/admin/characters/{id}/image", self.root))
			.header("Authorization", format!("Bearer {token}"))
			.multipart(form)
			.send()
			.await
			.map_err(|e| e.to_string())?
			.json()
			.await
			.map_err(|e| e.to_string())
	}
}
