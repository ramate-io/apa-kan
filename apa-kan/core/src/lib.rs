//! Apa Kan rules, scoring, bots, JSONL replay, and match clocks.
//!
//! This crate must not depend on Bevy. The Bevy client (`apa-kan`) is display
//! and in-match UI only; it depends on this crate, never the other way around.
//! See rfc/rfc-002-001-001-core-split.
//!
//! # What this crate owns
//!
//! - Board geometry, tiles, `RuleSet`, `Action`, `Game::legal_actions` / `apply`
//! - Live locked defaults (ratchet reward, 2-2-1-1 opening, additive braid-only-exact)
//! - Deck / bench / reserves, beads, emblems, capture marks, JSONL replay
//! - Bots (`ai`) for M0 — a later `apa-kan-ai` split can `git mv` this module
//! - Optional Fischer clocks ([`TimeControl::FiveMin`] 5+0, [`TimeControl::TwoPlusOne`] 2+1)
//!
//! Default games are unclocked (`Game` with no [`Clock`]). Time control is a
//! match axis, not a `RuleSet` knob. Core never reads a wall clock; the caller
//! passes `elapsed_ms`. Rating / account / store types stay out of this crate
//! so a platform server can depend on it without pulling Bevy.
//!
//! Character art (`apa-kan/game/src/characters.rs`) stays in the client.

pub mod ai;
pub mod bench_log;
pub mod board;
pub mod clock;
pub mod deck;
pub mod game;
pub mod scenario;

pub use board::{Board, NodeId};
pub use clock::{Clock, ClockTick, TimeControl};
pub use game::{Action, ActionOutcome, EndReason, Game, GameStatus, Player, RuleSet};
