//! Apa Kan rules, scoring, bots, JSONL replay, and clocks.
//!
//! This crate must not depend on Bevy. The Bevy client (`apa-kan`) is display
//! and in-match UI only; it depends on this crate, never the other way around.
//! See rfc/rfc-002-001-001-core-split.

pub mod ai;
pub mod bench_log;
pub mod board;
pub mod clock;
pub mod deck;
pub mod game;
pub mod scenario;
