use crate::config::GameConfig;
use crate::game::core::{Game, Player, Position, TileId};
use crate::game::rules::Rules;
use crate::game::{Action, ActionType, TileRemoval};
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

pub struct LoggedAction {
	pub text: String,
	pub squares: Vec<Position>,
	pub removals: Vec<TileRemoval>,
}

pub fn player_tag(player: Player) -> &'static str {
	match player {
		Player::One => "P1",
		Player::Two => "P2",
	}
}

pub fn format_removals(removals: &[TileRemoval]) -> String {
	if removals.is_empty() {
		return String::new();
	}

	let mut items = removals.to_vec();
	items.sort_by_key(|r| (r.position.row, r.position.col));

	if items.len() == 2 {
		return format!(
			"; both tiles removed (mismatch): {} at row {}, col {} and {} at row {}, col {}",
			player_tag(items[0].owner),
			items[0].position.row,
			items[0].position.col,
			player_tag(items[1].owner),
			items[1].position.row,
			items[1].position.col
		);
	}

	if items.len() == 1 {
		return format!(
			"; {} tile at row {}, col {} removed (mismatch)",
			player_tag(items[0].owner),
			items[0].position.row,
			items[0].position.col
		);
	}

	let listed: Vec<String> = items
		.iter()
		.map(|r| {
			format!("{} at row {}, col {}", player_tag(r.owner), r.position.row, r.position.col)
		})
		.collect();
	format!("; tiles removed (mismatch): {}", listed.join(", "))
}

pub fn describe_action(
	who: &str,
	action: Action,
	from: Option<Position>,
	captured: usize,
	removals: &[TileRemoval],
) -> String {
	let mut text = match action.action_type {
		ActionType::Add { position, .. } => {
			format!("{who} ADDed tile at row {}, col {}", position.row, position.col)
		}
		ActionType::Move { destination, rotation_count, .. } => {
			let origin = from
				.map(|p| format!("row {} col {}", p.row, p.col))
				.unwrap_or_else(|| "?".into());
			let mut line =
				format!("{who} MOVEd {origin} -> row {} col {}", destination.row, destination.col);
			if rotation_count > 0 {
				line.push_str(&format!(", rotated {rotation_count}"));
			}
			if captured > 0 {
				line.push_str(&format!(", captured {captured}"));
			}
			line
		}
		ActionType::Pass => format!("{who} passed"),
	};
	text.push_str(&format_removals(removals));
	text
}

pub fn log_applied_action(
	game: &mut Game,
	who: &str,
	action: Action,
) -> Result<LoggedAction, String> {
	let from = match action.action_type {
		ActionType::Move { tile_id, .. } => {
			game.board.tiles().find(|(_, t)| t.id == tile_id).map(|(pos, _)| *pos)
		}
		_ => None,
	};
	let opp_before = opponent_count(game, action.player);
	let action_num = game.action_number_this_turn();
	let allowed = game.actions_allowed_this_turn();
	let multi = game.rule_set.actions_per_turn > 1;
	let outcome = Rules::apply_action(game, action)?;
	let captured = opp_before.saturating_sub(opponent_count(game, action.player));
	let mut text = describe_action(who, action, from, captured, &outcome.removals);
	if multi {
		text.push_str(&format!(" (action {action_num} of {allowed})"));
	}
	let mut squares = action_squares(action, from);
	for removal in &outcome.removals {
		if !squares.contains(&removal.position) {
			squares.push(removal.position);
		}
	}
	Ok(LoggedAction { text, squares, removals: outcome.removals })
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
