use super::core::{Direction, Game, Player, Position, TileId};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
	Add { tile_index: usize, position: Position },
	Move { tile_id: TileId, destination: Position, rotation_count: u8 },
	Pass,
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

	pub fn move_tile(
		player: Player,
		tile_id: TileId,
		destination: Position,
		rotation_count: u8,
	) -> Self {
		Self { player, action_type: ActionType::Move { tile_id, destination, rotation_count } }
	}

	pub fn pass(player: Player) -> Self {
		Self { player, action_type: ActionType::Pass }
	}
}

pub fn generate_legal_actions(game: &Game) -> Vec<Action> {
	let mut actions = Vec::new();
	let player = game.current_player;

	actions.extend(generate_add_actions(game, player));
	actions.extend(generate_move_actions(game, player));

	if game.rule_set.scored_mode && (actions.is_empty() || game.rule_set.actions_per_turn > 1) {
		actions.push(Action::pass(player));
	}

	actions
}

fn generate_add_actions(game: &Game, player: Player) -> Vec<Action> {
	let mut actions = Vec::new();
	let stack = game.stack(player);

	if stack.is_empty() {
		return actions;
	}

	if game.rule_set.add_bank {
		let add_bank = match player {
			Player::One => game.add_bank_p1,
			Player::Two => game.add_bank_p2,
		};
		if add_bank == 0 {
			return actions;
		}
	}

	for tile_index in 0..stack.len() {
		for row in 0..game.board.size() as i8 {
			for col in 0..game.board.size() as i8 {
				let pos = Position::new(row, col);
				let is_legal_position = if game.rule_set.edge_add_only {
					game.board.is_edge(pos) && game.board.is_empty(pos)
				} else {
					game.board.is_valid(pos) && game.board.is_empty(pos)
				};

				if is_legal_position {
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

		let rotation_range = if game.rule_set.fixed_rotation { 0..2 } else { 0..4 };

		for rotation_count in rotation_range {
			let rotated_tile = if game.rule_set.fixed_rotation && rotation_count == 1 {
				match tile.rotation_direction {
					super::core::RotationDirection::Clockwise => tile.rotate_cw(),
					super::core::RotationDirection::CounterClockwise => tile.rotate_ccw(),
				}
			} else {
				tile.rotated_n_times(rotation_count)
			};

			let reachable = find_reachable_destinations(game, *pos, &rotated_tile);
			for dest in reachable {
				if dest != *pos {
					actions.push(Action::move_tile(player, tile.id, dest, rotation_count));
				}
			}
		}
	}

	actions
}

fn find_reachable_destinations(
	game: &Game,
	start: Position,
	tile: &super::core::Tile,
) -> Vec<Position> {
	let mut reachable = Vec::new();
	let mut visited = HashSet::new();
	let mut queue = VecDeque::new();

	let max = game.board.size() as i8;
	let has_adjacency = Direction::all().iter().any(|&dir| {
		let neighbor = start.neighbor(dir);
		neighbor.row >= 0
			&& neighbor.row < max
			&& neighbor.col >= 0
			&& neighbor.col < max
			&& game.board.get(neighbor).is_some()
	});

	if !has_adjacency {
		return reachable;
	}

	queue.push_back(start);
	visited.insert(start);

	while let Some(current_pos) = queue.pop_front() {
		for direction in Direction::all() {
			let neighbor_pos = current_pos.neighbor(direction);

			if neighbor_pos.row < 0
				|| neighbor_pos.row >= max
				|| neighbor_pos.col < 0
				|| neighbor_pos.col >= max
			{
				continue;
			}

			if visited.contains(&neighbor_pos) {
				continue;
			}

			visited.insert(neighbor_pos);

			if game.board.get(neighbor_pos).is_some() {
				if can_cross_edge(game, tile, direction, neighbor_pos) {
					queue.push_back(neighbor_pos);
				}
			} else if game.board.is_valid(neighbor_pos) {
				reachable.push(neighbor_pos);
			}
		}
	}

	reachable
}

fn can_cross_edge(
	game: &Game,
	moving_tile: &super::core::Tile,
	direction: Direction,
	crossing_pos: Position,
) -> bool {
	let crossing_tile = game.board.get(crossing_pos).unwrap();
	let beyond_pos = crossing_pos.neighbor(direction);

	let crossing_edge_weight = crossing_tile.edge(direction).weight.value();

	let max = game.board.size() as i8;
	let effective_weight = if beyond_pos.row >= 0
		&& beyond_pos.row < max
		&& beyond_pos.col >= 0
		&& beyond_pos.col < max
	{
		if let Some(beyond_tile) = game.board.get(beyond_pos) {
			crossing_edge_weight.max(beyond_tile.edge(direction.opposite()).weight.value())
		} else {
			crossing_edge_weight
		}
	} else {
		crossing_edge_weight
	};

	let mover_edge_weight = moving_tile.edge(direction).weight.value();
	effective_weight >= mover_edge_weight
}

fn has_legal_add_square(game: &Game) -> bool {
	let max = game.board.size() as i8;
	for row in 0..max {
		for col in 0..max {
			let pos = Position::new(row, col);
			let is_legal_position = if game.rule_set.edge_add_only {
				game.board.is_edge(pos) && game.board.is_empty(pos)
			} else {
				game.board.is_valid(pos) && game.board.is_empty(pos)
			};
			if is_legal_position {
				return true;
			}
		}
	}
	false
}

pub fn has_legal_add_or_move(game: &Game, player: Player) -> bool {
	if !game.stack(player).is_empty() {
		if game.rule_set.add_bank {
			let add_bank = match player {
				Player::One => game.add_bank_p1,
				Player::Two => game.add_bank_p2,
			};
			if add_bank > 0 && has_legal_add_square(game) {
				return true;
			}
		} else if has_legal_add_square(game) {
			return true;
		}
	}

	for (pos, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		for direction in Direction::all() {
			let target = pos.neighbor(direction);
			if game.board.is_valid(target) && game.board.is_empty(target) {
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
		let game = Game::new_with_rules(crate::game::core::RuleSet::old());
		let actions = generate_add_actions(&game, Player::One);

		let valid_positions = 21;
		assert_eq!(actions.len(), valid_positions * 11);
	}

	#[test]
	fn test_generate_add_actions_edge_only() {
		let game = Game::new();
		let actions = generate_add_actions(&game, Player::One);

		// 12 edge positions: (0,1), (0,2), (0,3), (1,0), (1,4), (2,0), (2,4), (3,0), (3,4), (4,1), (4,2), (4,3)
		// Times 11 tiles in stack
		let edge_positions = 12;
		assert_eq!(actions.len(), edge_positions * 11);
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
		assert_eq!(actions.len(), 0);
	}

	#[test]
	fn test_move_with_rotation() {
		let mut game = Game::new();
		let tile1 = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
		);
		let tile2 = Tile::new(
			1,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
		);

		game.board.place(Position::new(2, 2), tile1);
		game.board.place(Position::new(2, 3), tile2);

		let actions = generate_move_actions(&game, Player::One);
		assert!(actions.len() > 0);

		let move_from_2_2: Vec<_> = actions
			.iter()
			.filter(|a| {
				if let ActionType::Move { tile_id, .. } = a.action_type {
					tile_id == 0
				} else {
					false
				}
			})
			.collect();
		assert!(move_from_2_2.len() >= 4);
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

	#[test]
	fn test_fixed_rotation_respects_ccw_direction() {
		use crate::game::core::RotationDirection;

		let mut game = Game::new_with_rules(crate::game::core::RuleSet::new());
		let mut ccw_tile = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Green, weight: EdgeWeight::Single },
			Edge { color: Color::Blue, weight: EdgeWeight::Single },
			Edge { color: Color::Orange, weight: EdgeWeight::Single },
		);
		ccw_tile.rotation_direction = RotationDirection::CounterClockwise;

		let helper_tile = Tile::new(
			1,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
		);

		game.board.place(Position::new(0, 1), ccw_tile);
		game.board.place(Position::new(0, 2), helper_tile);

		let actions = generate_move_actions(&game, Player::One);

		let ccw_rotations: Vec<_> = actions
			.iter()
			.filter(|a| {
				if let ActionType::Move { tile_id, rotation_count, .. } = a.action_type {
					tile_id == 0 && rotation_count == 1
				} else {
					false
				}
			})
			.collect();

		assert!(ccw_rotations.len() > 0, "Should generate rotation=1 moves for CCW tile");
	}

	#[test]
	fn test_two_action_scored_includes_pass_even_with_legal_adds() {
		let one = Game::new_with_rules(crate::game::core::RuleSet::scored());
		assert!(
			!generate_legal_actions(&one)
				.iter()
				.any(|a| matches!(a.action_type, ActionType::Pass)),
			"one-action scored must not offer Pass while ADD is legal"
		);

		let two = Game::new_with_rules(crate::game::core::RuleSet::scored_double());
		assert!(
			generate_legal_actions(&two)
				.iter()
				.any(|a| matches!(a.action_type, ActionType::Pass)),
			"two-action scored must offer Pass so the player can end the turn early"
		);
	}
}
