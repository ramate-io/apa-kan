use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

pub trait Clock: Send + Sync {
	fn now_unix(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
	fn now_unix(&self) -> i64 {
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_secs() as i64)
			.unwrap_or(0)
	}
}

#[derive(Clone, Default)]
pub struct FakeClock {
	secs: Arc<AtomicI64>,
}

impl FakeClock {
	pub fn new(start_unix: i64) -> Self {
		Self {
			secs: Arc::new(AtomicI64::new(start_unix)),
		}
	}

	pub fn advance_secs(&self, delta: i64) {
		self.secs.fetch_add(delta, Ordering::SeqCst);
	}

	pub fn set(&self, unix: i64) {
		self.secs.store(unix, Ordering::SeqCst);
	}
}

impl Clock for FakeClock {
	fn now_unix(&self) -> i64 {
		self.secs.load(Ordering::SeqCst)
	}
}