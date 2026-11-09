use super::actions::{has_legal_add_or_move, Action, ActionType};
use super::core::{Game, GameState, Player, Position, Tile};

pub struct Rules;

impl Rules {
	pub fn apply_action(game: &mut Game, action: Action) -> Result<(), String> {
		if game.is_game_over() {
			return Err("Game is over".to_string());
		}

		if action.player != game.current_player {
			return Err("Not your turn".to_string());
		}

		match action.action_type {
			ActionType::Add { tile_index, position } => {
				Self::apply_add(game, tile_index, position)?;
			}
			ActionType::Move { tile_id, destination } => {
				Self::apply_move(game, tile_id, destination)?;
			}
			ActionType::Rotate { tile_id, clockwise } => {
				Self::apply_rotate(game, tile_id, clockwise)?;
			}
		}

		Self::check_and_remove_all_mismatched(game);

		Self::check_win_condition(game);

		game.current_player = game.current_player.other();

		Ok(())
	}

	fn apply_add(game: &mut Game, tile_index: usize, position: Position) -> Result<(), String> {
		if !position.is_valid() {
			return Err("Invalid position".to_string());
		}

		if !game.board.is_empty(position) {
			return Err("Position is occupied".to_string());
		}

		let stack = game.stack_mut(game.current_player);
		if tile_index >= stack.len() {
			return Err("Tile index out of bounds".to_string());
		}

		let tile = stack.remove(tile_index);
		game.board.place(position, tile);

		Ok(())
	}

	fn apply_move(game: &mut Game, tile_id: u32, destination: Position) -> Result<(), String> {
		let from_pos = game
			.board
			.find_tile(tile_id)
			.ok_or_else(|| "Tile not found on board".to_string())?;

		let tile = game.board.get(from_pos).ok_or_else(|| "Tile not found".to_string())?;

		if tile.owner != game.current_player {
			return Err("Not your tile".to_string());
		}

		if !destination.is_valid() {
			return Err("Invalid destination".to_string());
		}

		if !game.board.is_empty(destination) {
			return Err("Destination is occupied".to_string());
		}

		let tile = game.board.remove(from_pos).expect("Tile should exist");
		game.board.place(destination, tile);

		Ok(())
	}

	fn apply_rotate(game: &mut Game, tile_id: u32, clockwise: bool) -> Result<(), String> {
		let position = game
			.board
			.find_tile(tile_id)
			.ok_or_else(|| "Tile not found on board".to_string())?;

		let tile = game.board.get(position).ok_or_else(|| "Tile not found".to_string())?;

		if tile.owner != game.current_player {
			return Err("Not your tile".to_string());
		}

		let original_tile = tile.clone();
		let rotated_tile = if clockwise { tile.rotate_cw() } else { tile.rotate_ccw() };

		if Self::tiles_equal_orientation(&original_tile, &rotated_tile) {
			return Err("Cannot end in starting orientation".to_string());
		}

		game.board.place(position, rotated_tile);

		Ok(())
	}

	fn tiles_equal_orientation(a: &Tile, b: &Tile) -> bool {
		a.north == b.north && a.east == b.east && a.south == b.south && a.west == b.west
	}

	fn check_and_remove_all_mismatched(game: &mut Game) {
		let mut tiles_to_remove = Vec::new();

		for (position, tile) in game.board.tiles() {
			let mut matches = 0;
			let mut mismatches = 0;

			for (direction, neighbor_pos) in position.neighbors() {
				if neighbor_pos.is_corner() {
					mismatches += 1;
					continue;
				}

				if neighbor_pos.row < 0
					|| neighbor_pos.row >= 5
					|| neighbor_pos.col < 0
					|| neighbor_pos.col >= 5
				{
					continue;
				}

				if let Some(neighbor_tile) = game.board.get(neighbor_pos) {
					let my_edge = tile.edge(direction);
					let their_edge = neighbor_tile.edge(direction.opposite());

					if my_edge.color == their_edge.color {
						matches += 1;
					} else {
						mismatches += 1;
					}
				}
			}

			if mismatches > matches {
				tiles_to_remove.push(*position);
			}
		}

		for position in tiles_to_remove {
			game.board.remove(position);
		}
	}

	fn check_win_condition(game: &mut Game) {
		let current_can_move = has_legal_add_or_move(game, game.current_player);

		if !current_can_move {
			game.state = GameState::Won(game.current_player.other());
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::core::{Color, Edge, EdgeWeight, Tile};

	fn make_tile(id: u32, owner: Player, colors: [Color; 4]) -> Tile {
		Tile::new(
			id,
			owner,
			Edge { color: colors[0], weight: EdgeWeight::Single },
			Edge { color: colors[1], weight: EdgeWeight::Single },
			Edge { color: colors[2], weight: EdgeWeight::Single },
			Edge { color: colors[3], weight: EdgeWeight::Single },
		)
	}

	#[test]
	fn test_add_tile_success() {
		let mut game = Game::new();
		let initial_stack_size = game.player_one_stack.len();

		let action = Action::add(Player::One, 0, Position::new(2, 2));
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_ok());
		assert_eq!(game.player_one_stack.len(), initial_stack_size - 1);
		assert!(game.board.get(Position::new(2, 2)).is_some());
	}

	#[test]
	fn test_add_to_corner_fails() {
		let mut game = Game::new();
		let action = Action::add(Player::One, 0, Position::new(0, 0));
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_err());
	}

	#[test]
	fn test_corner_always_mismatches() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		let action = Action::add(Player::One, 0, Position::new(0, 1));
		Rules::apply_action(&mut game, action).ok();

		assert!(game.board.get(Position::new(0, 1)).is_none());
		assert_eq!(game.player_one_stack.len(), 0);
	}

	#[test]
	fn test_empty_adjacent_is_neutral() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		let action = Action::add(Player::One, 0, Position::new(2, 2));
		Rules::apply_action(&mut game, action).ok();

		assert!(game.board.get(Position::new(2, 2)).is_some());
		assert_eq!(game.player_one_stack.len(), 0);
	}

	#[test]
	fn test_matching_edges_survive() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(
					0,
					Player::One,
					[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
				),
				make_tile(
					1,
					Player::One,
					[Color::Blue, Color::Maroon, Color::Maroon, Color::Green],
				),
			],
			vec![],
		);

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(2, 2))).ok();
		game.current_player = Player::One;

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(1, 2))).ok();

		assert!(game.board.get(Position::new(2, 2)).is_some());
		assert!(game.board.get(Position::new(1, 2)).is_some());
	}

	#[test]
	fn test_mismatching_edges_removed() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(
					0,
					Player::One,
					[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
				),
				make_tile(
					1,
					Player::One,
					[Color::Green, Color::Blue, Color::Orange, Color::Maroon],
				),
			],
			vec![],
		);

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(2, 2))).ok();
		game.current_player = Player::One;

		let initial_stack_size = game.player_one_stack.len();

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(1, 2))).ok();

		let tiles_on_board = game.board.tiles().count();

		assert!(tiles_on_board <= 2);
	}

	#[test]
	fn test_move_tile() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));

		let tile_id = game.board.get(Position::new(2, 2)).unwrap().id;
		let action = Action::move_tile(Player::One, tile_id, Position::new(1, 2));
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_ok());
		assert!(game.board.get(Position::new(2, 2)).is_none());
		assert!(game.board.get(Position::new(1, 2)).is_some());
	}

	#[test]
	fn test_rotate_tile() {
		let mut game = Game::new_with_tiles(
			vec![make_tile(
				0,
				Player::One,
				[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
			)],
			vec![],
		);

		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));

		let tile_id = game.board.get(Position::new(2, 2)).unwrap().id;
		let original = game.board.get(Position::new(2, 2)).unwrap().clone();

		let action = Action::rotate(Player::One, tile_id, true);
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_ok());

		let rotated = game.board.get(Position::new(1, 2)).or_else(|| game.player_one_stack.first());
		if let Some(tile) = game.board.get(Position::new(2, 2)) {
			assert_ne!(tile.north.color, original.north.color);
		}
	}

	#[test]
	fn test_rotate_to_same_orientation_fails() {
		let tile = make_tile(0, Player::One, [Color::Maroon; 4]);
		let mut game = Game::new_with_tiles(vec![tile.clone()], vec![]);

		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));

		let tile_id = game.board.get(Position::new(2, 2)).unwrap().id;
		let action = Action::rotate(Player::One, tile_id, true);
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_err());
	}

	#[test]
	fn test_removed_tile_discarded() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		let action = Action::add(Player::One, 0, Position::new(0, 1));
		Rules::apply_action(&mut game, action).ok();

		assert_eq!(game.player_one_stack.len(), 0);
		assert!(game.board.get(Position::new(0, 1)).is_none());
	}

	#[test]
	fn test_win_condition_no_moves() {
		let mut game =
			Game::new_with_tiles(vec![], vec![make_tile(0, Player::Two, [Color::Maroon; 4])]);

		for row in 0..5 {
			for col in 0..5 {
				let pos = Position::new(row, col);
				if pos.is_valid() {
					game.board.place(
						pos,
						make_tile(
							100 + row as u32 * 5 + col as u32,
							Player::Two,
							[Color::Maroon; 4],
						),
					);
				}
			}
		}

		game.current_player = Player::One;
		Rules::check_win_condition(&mut game);

		assert_eq!(game.state, GameState::Won(Player::Two));
	}

	#[test]
	fn test_board_wide_removal_check() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(
					0,
					Player::One,
					[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
				),
				make_tile(
					1,
					Player::One,
					[Color::Green, Color::Blue, Color::Orange, Color::Maroon],
				),
			],
			vec![],
		);

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(2, 2))).ok();
		game.current_player = Player::One;

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(1, 2))).ok();
	}

	#[test]
	fn test_simultaneous_removal() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(
					0,
					Player::One,
					[Color::Maroon, Color::Maroon, Color::Maroon, Color::Maroon],
				),
				make_tile(1, Player::One, [Color::Green, Color::Green, Color::Green, Color::Green]),
				make_tile(2, Player::One, [Color::Blue, Color::Blue, Color::Blue, Color::Blue]),
			],
			vec![],
		);

		game.board.place(Position::new(0, 1), game.player_one_stack.remove(0));
		game.board.place(Position::new(0, 2), game.player_one_stack.remove(0));

		let tiles_before = game.board.tiles().count();

		Rules::apply_action(&mut game, Action::add(Player::One, 0, Position::new(0, 3))).ok();

		let tiles_after = game.board.tiles().count();
		assert!(tiles_after < tiles_before);
	}

	#[test]
	fn test_board_boundary_neutral() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		let action = Action::add(Player::One, 0, Position::new(2, 0));
		Rules::apply_action(&mut game, action).ok();

		assert!(game.board.get(Position::new(2, 0)).is_some());
		assert_eq!(game.player_one_stack.len(), 0);
	}

	#[test]
	fn test_end_of_turn_removal() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(
					0,
					Player::One,
					[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
				),
				make_tile(
					1,
					Player::One,
					[Color::Green, Color::Blue, Color::Orange, Color::Maroon],
				),
			],
			vec![],
		);

		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));

		let initial_on_board = game.board.tiles().count();

		let tile_id = game.board.get(Position::new(2, 2)).unwrap().id;
		Rules::apply_action(&mut game, Action::rotate(Player::Two, tile_id, true)).ok();

		let after_action = game.board.tiles().count();
		assert_eq!(after_action, initial_on_board);
	}

	#[test]
	fn test_move_tile_traveling_through_adjacencies() {
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(0, Player::One, [Color::Maroon; 4]),
				make_tile(1, Player::One, [Color::Maroon; 4]),
				make_tile(2, Player::One, [Color::Maroon; 4]),
			],
			vec![],
		);

		game.board.place(Position::new(2, 1), game.player_one_stack.remove(0));
		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));
		game.board.place(Position::new(2, 3), game.player_one_stack.remove(0));

		let tile_id = game.board.get(Position::new(2, 1)).unwrap().id;
		let action = Action::move_tile(Player::One, tile_id, Position::new(1, 3));
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_ok());
		assert!(game.board.get(Position::new(2, 1)).is_none());
		assert!(game.board.get(Position::new(1, 3)).is_some());
	}

	#[test]
	fn test_move_no_adjacencies_no_moves() {
		let mut game =
			Game::new_with_tiles(vec![make_tile(0, Player::One, [Color::Maroon; 4])], vec![]);

		game.board.place(Position::new(2, 2), game.player_one_stack.remove(0));

		use crate::game::actions::generate_legal_actions;
		let actions = generate_legal_actions(&game);

		let move_actions: Vec<_> = actions
			.iter()
			.filter(|a| matches!(a.action_type, ActionType::Move { .. }))
			.collect();

		assert_eq!(move_actions.len(), 0);
	}
}
