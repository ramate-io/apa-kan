//! Persistence traits and backends for accounts, keys, inventory, ratings,
//! character catalog, symbolic pass versions, and ANN lookup.

pub mod ann;
pub mod catalog;
pub mod error;
pub mod ids;
pub mod model;
pub mod pass;
pub mod repos;
pub mod seed;
pub mod time;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

pub use ann::AnnHit;
pub use catalog::*;
pub use error::StoreError;
pub use ids::{core_tile_id, tile_type_id, tile_type_id_from_core, IssuedTileId};
pub use model::*;
pub use pass::{PassPointer, ROLE_ADMIN};
pub use repos::*;
pub use seed::seed_bundled_v1;
pub use time::{Clock, FakeClock, SystemClock};
