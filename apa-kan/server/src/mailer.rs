use std::sync::{Arc, Mutex};

pub trait Mailer: Send + Sync {
	fn send_recovery_token(&self, email: &str, account_hex: &str, token: &str);
}

#[derive(Clone, Default)]
pub struct FakeMailer {
	pub sent: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl Mailer for FakeMailer {
	fn send_recovery_token(&self, email: &str, account_hex: &str, token: &str) {
		self.sent
			.lock()
			.unwrap()
			.push((email.to_string(), account_hex.to_string(), token.to_string()));
	}
}

pub struct NoopMailer;

impl Mailer for NoopMailer {
	fn send_recovery_token(&self, _email: &str, _account_hex: &str, _token: &str) {}
}