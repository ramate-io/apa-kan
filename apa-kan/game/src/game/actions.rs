use super::core::{Direction, Game, Player, Position, TileId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
	Add { tile_index: usize, position: Position },
	Move { tile_id: TileId, direction: Direction },
	Rotate { tile_id: TileId, clockwise: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
	pub player: Player,
	pub action_type: ActionType,
}

impl Action {
	pub fn add(player: Player, tile_index: usize, position: Position) -> Self {
		Self { player, action_type: ActionType::Add { tile_index, position } }
	}

	pub fn move_tile(player: Player, tile_id: TileId, direction: Direction) -> Self {
		Self { player, action_type: ActionType::Move { tile_id, direction } }
	}

	pub fn rotate(player: Player, tile_id: TileId, clockwise: bool) -> Self {
		Self { player, action_type: ActionType::Rotate { tile_id, clockwise } }
	}
}

pub fn generate_legal_actions(game: &Game) -> Vec<Action> {
	let mut actions = Vec::new();
	let player = game.current_player;

	actions.extend(generate_add_actions(game, player));
	actions.extend(generate_move_actions(game, player));
	actions.extend(generate_rotate_actions(game, player));

	actions
}

fn generate_add_actions(game: &Game, player: Player) -> Vec<Action> {
	let mut actions = Vec::new();
	let stack = game.stack(player);

	if stack.is_empty() {
		return actions;
	}

	for tile_index in 0..stack.len() {
		for row in 0..5 {
			for col in 0..5 {
				let pos = Position::new(row, col);
				if pos.is_valid() && game.board.is_empty(pos) {
					actions.push(Action::add(player, tile_index, pos));
				}
			}
		}
	}

	actions
}

fn generate_move_actions(game: &Game, player: Player) -> Vec<Action> {
	let mut actions = Vec::new();

	for (pos, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		for direction in Direction::all() {
			let target = pos.neighbor(direction);
			if target.is_valid() && game.board.is_empty(target) {
				actions.push(Action::move_tile(player, tile.id, direction));
			}
		}
	}

	actions
}

fn generate_rotate_actions(game: &Game, player: Player) -> Vec<Action> {
	let mut actions = Vec::new();

	for (_, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		actions.push(Action::rotate(player, tile.id, true));
		actions.push(Action::rotate(player, tile.id, false));
	}

	actions
}

pub fn has_legal_add_or_move(game: &Game, player: Player) -> bool {
	if !game.stack(player).is_empty() {
		for row in 0..5 {
			for col in 0..5 {
				let pos = Position::new(row, col);
				if pos.is_valid() && game.board.is_empty(pos) {
					return true;
				}
			}
		}
	}

	for (pos, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		for direction in Direction::all() {
			let target = pos.neighbor(direction);
			if target.is_valid() && game.board.is_empty(target) {
				return true;
			}
		}
	}

	false
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::core::{Color, Edge, EdgeWeight, Tile};

	#[test]
	fn test_generate_add_actions_empty_board() {
		let game = Game::new();
		let actions = generate_add_actions(&game, Player::One);

		let valid_positions = 21;
		assert_eq!(actions.len(), valid_positions * 11);
	}

	#[test]
	fn test_generate_move_actions_single_tile() {
		let mut game = Game::new();
		let tile = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Green, weight: EdgeWeight::Single },
			Edge { color: Color::Blue, weight: EdgeWeight::Single },
			Edge { color: Color::Orange, weight: EdgeWeight::Single },
		);

		game.board.place(Position::new(2, 2), tile);

		let actions = generate_move_actions(&game, Player::One);
		assert_eq!(actions.len(), 4);
	}

	#[test]
	fn test_generate_rotate_actions() {
		let mut game = Game::new();
		let tile = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Green, weight: EdgeWeight::Single },
			Edge { color: Color::Blue, weight: EdgeWeight::Single },
			Edge { color: Color::Orange, weight: EdgeWeight::Single },
		);

		game.board.place(Position::new(2, 2), tile);

		let actions = generate_rotate_actions(&game, Player::One);
		assert_eq!(actions.len(), 2);
	}

	#[test]
	fn test_has_legal_add_or_move_initial() {
		let game = Game::new();
		assert!(has_legal_add_or_move(&game, Player::One));
	}

	#[test]
	fn test_has_legal_add_or_move_no_moves() {
		let mut game = Game::new();
		game.player_one_stack.clear();

		assert!(!has_legal_add_or_move(&game, Player::One));
	}
}
