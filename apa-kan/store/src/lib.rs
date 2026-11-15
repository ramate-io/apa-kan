//! Persistence traits and backends for accounts, keys, inventory, and ratings.

pub mod error;
pub mod model;
pub mod repos;
pub mod time;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use error::StoreError;
pub use model::*;
pub use repos::*;
pub use time::{Clock, FakeClock, SystemClock};
