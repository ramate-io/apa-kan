use super::core::{Direction, Game, Player, Position, TileId};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
	Add { tile_index: usize, position: Position },
	Move { tile_id: TileId, destination: Position, rotation_count: u8 },
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
}

pub fn generate_legal_actions(game: &Game) -> Vec<Action> {
	let mut actions = Vec::new();
	let player = game.current_player;

	actions.extend(generate_add_actions(game, player));
	actions.extend(generate_move_actions(game, player));

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

		for rotation_count in 0..4 {
			let rotated_tile = tile.rotated_n_times(rotation_count);
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

	let has_adjacency = Direction::all().iter().any(|&dir| {
		let neighbor = start.neighbor(dir);
		neighbor.row >= 0
			&& neighbor.row < 5
			&& neighbor.col >= 0
			&& neighbor.col < 5
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
				|| neighbor_pos.row >= 5
				|| neighbor_pos.col < 0
				|| neighbor_pos.col >= 5
			{
				continue;
			}

			if visited.contains(&neighbor_pos) {
				continue;
			}

			visited.insert(neighbor_pos);

			if let Some(neighbor_tile) = game.board.get(neighbor_pos) {
				if can_cross_edge(tile, direction, neighbor_tile) {
					queue.push_back(neighbor_pos);
				}
			} else if neighbor_pos.is_valid() {
				reachable.push(neighbor_pos);
			}
		}
	}

	reachable
}

fn can_cross_edge(
	moving_tile: &super::core::Tile,
	direction: Direction,
	adjacent_tile: &super::core::Tile,
) -> bool {
	let moving_edge = moving_tile.edge(direction);
	let adjacent_edge = adjacent_tile.edge(direction.opposite());

	let edge_weight = moving_edge.weight.value().max(adjacent_edge.weight.value());
	moving_edge.weight.value() <= edge_weight
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
}
