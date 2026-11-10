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
			ActionType::Move { tile_id, destination, rotation_count } => {
				Self::apply_move(game, tile_id, destination, rotation_count)?;
			}
		}

		Self::check_and_remove_all_mismatched(game);

		Self::check_win_condition(game);

		game.current_player = game.current_player.other();
		game.turn_count += 1;

		Self::check_turn_limit_draw(game);
		Self::check_repetition_draw(game);

		Ok(())
	}

	fn apply_add(game: &mut Game, tile_index: usize, position: Position) -> Result<(), String> {
		let is_valid_position = if game.rule_set.edge_add_only {
			game.board.is_edge(position)
		} else {
			game.board.is_valid(position)
		};

		if !is_valid_position {
			return Err("Invalid position for ADD".to_string());
		}

		if !game.board.is_empty(position) {
			return Err("Position is occupied".to_string());
		}

		let player = game.current_player;

		if game.rule_set.add_bank {
			match player {
				Player::One => {
					if game.add_bank_p1 == 0 {
						return Err("No ADD actions remaining".to_string());
					}
					game.add_bank_p1 -= 1;
				}
				Player::Two => {
					if game.add_bank_p2 == 0 {
						return Err("No ADD actions remaining".to_string());
					}
					game.add_bank_p2 -= 1;
				}
			}
		}

		let stack = game.stack_mut(player);
		if tile_index >= stack.len() {
			return Err("Tile index out of bounds".to_string());
		}

		let tile = stack.remove(tile_index);
		game.board.place(position, tile);

		Ok(())
	}

	fn apply_move(
		game: &mut Game,
		tile_id: u32,
		destination: Position,
		rotation_count: u8,
	) -> Result<(), String> {
		let from_pos = game
			.board
			.find_tile(tile_id)
			.ok_or_else(|| "Tile not found on board".to_string())?;

		if from_pos == destination {
			return Err(
				"Cannot move to same position (use rotation_count if reorienting)".to_string()
			);
		}

		let tile = game.board.get(from_pos).ok_or_else(|| "Tile not found".to_string())?;
		let tile_id = tile.id;

		if tile.owner != game.current_player {
			return Err("Not your tile".to_string());
		}

		if !game.board.is_valid(destination) {
			return Err("Invalid destination".to_string());
		}

		if !game.board.is_empty(destination) {
			return Err("Destination is occupied".to_string());
		}

		let mut tile = game.board.remove(from_pos).expect("Tile should exist");
		tile = if game.rule_set.fixed_rotation {
			if rotation_count == 0 {
				tile
			} else if rotation_count == 1 {
				match tile.rotation_direction {
					super::core::RotationDirection::Clockwise => tile.rotate_cw(),
					super::core::RotationDirection::CounterClockwise => tile.rotate_ccw(),
				}
			} else {
				return Err("Invalid rotation count for fixed rotation rules".to_string());
			}
		} else {
			tile.rotated_n_times(rotation_count)
		};
		game.board.place(destination, tile);
		game.last_moved_tile = Some(tile_id);
		game.last_moved_turn = game.turn_count;

		Ok(())
	}

	fn check_and_remove_all_mismatched(game: &mut Game) {
		let mut tiles_to_remove = Vec::new();

		let tile_count = game.board.tiles().count();
		let immunity_active = game.rule_set.moved_tile_immunity && tile_count > 7;

		for (position, tile) in game.board.tiles() {
			if immunity_active
				&& Some(tile.id) == game.last_moved_tile
				&& game.turn_count == game.last_moved_turn
			{
				continue;
			}

			let mut matches = 0;
			let mut mismatches = 0;

		for (direction, neighbor_pos) in position.neighbors() {
			if game.board.is_corner(neighbor_pos) {
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
			if let Some(tile) = game.board.remove(position) {
				match tile.owner {
					Player::One => {
						if game.rule_set.add_bank {
							game.add_bank_p2 = (game.add_bank_p2 + 1).min(255);
						}
					}
					Player::Two => {
						if game.rule_set.add_bank {
							game.add_bank_p1 = (game.add_bank_p1 + 1).min(255);
						}
					}
				}
			}
		}
	}

	fn check_win_condition(game: &mut Game) {
		let current_can_move = has_legal_add_or_move(game, game.current_player);

		if !current_can_move {
			game.state = GameState::Won(game.current_player.other());
		}
	}

	fn check_turn_limit_draw(game: &mut Game) {
		if game.state != GameState::InProgress {
			return;
		}

		if game.turn_count >= 100 {
			game.state = GameState::Draw;
		}
	}

	fn check_repetition_draw(game: &mut Game) {
		if game.state != GameState::InProgress {
			return;
		}

		let current_hash = game.compute_position_hash();
		game.position_history.push(current_hash);

		if game.position_history.len() >= 6 {
			let len = game.position_history.len();
			let latest = game.position_history[len - 1];
			let two_ago = game.position_history[len - 3];
			let four_ago = game.position_history[len - 5];

			if latest == two_ago && latest == four_ago {
				game.state = GameState::Draw;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::core::RuleSet;
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
		let mut game = Game::new_with_rules(RuleSet::old());
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
		let mut game = Game::new_with_tiles_and_rules(
			vec![make_tile(0, Player::One, [Color::Maroon; 4])],
			vec![],
			RuleSet::old(),
		);

		let action = Action::add(Player::One, 0, Position::new(2, 2));
		Rules::apply_action(&mut game, action).ok();

		assert!(game.board.get(Position::new(2, 2)).is_some());
		assert_eq!(game.player_one_stack.len(), 0);
	}

	#[test]
	fn test_matching_edges_survive() {
		let mut game = Game::new_with_tiles_and_rules(
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
			RuleSet::old(),
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
		let action = Action::move_tile(Player::One, tile_id, Position::new(1, 2), 0);
		let result = Rules::apply_action(&mut game, action);

		assert!(result.is_ok());
		assert!(game.board.get(Position::new(2, 2)).is_none());
		assert!(game.board.get(Position::new(1, 2)).is_some());
	}

	#[test]
	fn test_move_with_rotation() {
		let actions = crate::game::actions::generate_legal_actions(&Game::new());
		let move_actions: Vec<_> = actions
			.iter()
			.filter(|a| matches!(a.action_type, ActionType::Move { .. }))
			.collect();
		assert!(move_actions.len() == 0);
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
			if game.board.is_valid(pos) {
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
		let mut game = Game::new_with_tiles_and_rules(
			vec![make_tile(0, Player::One, [Color::Maroon; 4])],
			vec![],
			RuleSet::old(),
		);

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
		let action = Action::move_tile(Player::One, tile_id, Position::new(1, 3), 0);
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
