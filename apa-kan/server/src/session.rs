//! In-process sessions issued after a verified ML-DSA challenge.

use std::collections::HashMap;
use std::sync::Mutex;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use rand::RngCore;

const SESSION_TTL_SECS: i64 = 24 * 3600;

#[derive(Clone, Debug)]
pub struct Session {
	pub token: String,
	pub account_id: AccountId,
	pub vk: VerifyingKeyBytes,
	pub is_admin: bool,
	pub expires_at_unix: i64,
}

#[derive(Default)]
pub struct SessionStore {
	inner: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn issue(
		&self,
		account_id: AccountId,
		vk: VerifyingKeyBytes,
		is_admin: bool,
		now_unix: i64,
	) -> Session {
		let mut bytes = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut bytes);
		let token = hex::encode(bytes);
		let session = Session {
			token: token.clone(),
			account_id,
			vk,
			is_admin,
			expires_at_unix: now_unix + SESSION_TTL_SECS,
		};
		self.inner.lock().expect("session mutex").insert(token, session.clone());
		session
	}

	pub fn get(&self, token: &str, now_unix: i64) -> Option<Session> {
		let map = self.inner.lock().expect("session mutex");
		let session = map.get(token)?;
		(now_unix <= session.expires_at_unix).then(|| session.clone())
	}
}
