use crate::config::GameConfig;
use crate::game::core::{Player, Position, TileId};
use crate::game::{Action, ActionType};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct InputState {
	pub selection: Selection,
	pub rotation: u8,
	pub last_move: Option<LastMove>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
	#[default]
	None,
	PendingAdd {
		position: Position,
	},
	MoveTile {
		tile_id: TileId,
		from: Position,
	},
}

#[derive(Clone, Debug)]
pub struct LastMove {
	pub text: String,
	pub squares: Vec<Position>,
}

pub fn human_player(config: &GameConfig) -> Option<Player> {
	if config.bot.is_none() {
		return None;
	}
	if config.human_seat == 2 {
		Some(Player::Two)
	} else {
		Some(Player::One)
	}
}

pub fn is_human_turn(config: &GameConfig, current: Player) -> bool {
	match human_player(config) {
		None => true,
		Some(human) => current == human,
	}
}

pub fn actor_label(config: &GameConfig, player: Player) -> String {
	let seat = match player {
		Player::One => "P1",
		Player::Two => "P2",
	};
	match human_player(config) {
		None => seat.to_string(),
		Some(human) if human == player => format!("{seat} (You)"),
		Some(_) => {
			let bot = config.bot.map(|b| b.name()).unwrap_or("Bot");
			format!("{seat} Bot ({bot})")
		}
	}
}

pub fn describe_action(
	who: &str,
	action: Action,
	from: Option<Position>,
	captured: usize,
	self_removed: bool,
) -> String {
	match action.action_type {
		ActionType::Add { position, .. } => {
			let mut text =
				format!("{who} ADDed tile at row {}, col {}", position.row, position.col);
			if self_removed {
				text.push_str(" (removed immediately - mismatched)");
			}
			text
		}
		ActionType::Move { destination, rotation_count, .. } => {
			let origin = from
				.map(|p| format!("row {} col {}", p.row, p.col))
				.unwrap_or_else(|| "?".into());
			let mut text =
				format!("{who} MOVEd {origin} -> row {} col {}", destination.row, destination.col);
			if rotation_count > 0 {
				text.push_str(&format!(", rotated {rotation_count}"));
			}
			if captured > 0 {
				text.push_str(&format!(", captured {captured}"));
			}
			if self_removed {
				text.push_str(" (own tile removed)");
			}
			text
		}
		ActionType::Pass => format!("{who} passed"),
	}
}

pub fn action_squares(action: Action, from: Option<Position>) -> Vec<Position> {
	match action.action_type {
		ActionType::Add { position, .. } => vec![position],
		ActionType::Move { destination, .. } => {
			let mut squares = Vec::new();
			if let Some(origin) = from {
				squares.push(origin);
			}
			squares.push(destination);
			squares
		}
		ActionType::Pass => Vec::new(),
	}
}

pub fn opponent_count(game: &crate::game::core::Game, player: Player) -> usize {
	game.board.tiles().filter(|(_, t)| t.owner == player.other()).count()
}
