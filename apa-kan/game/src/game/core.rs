use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
	Maroon,
	Green,
	Blue,
	Orange,
}

impl Color {
	pub fn all() -> [Color; 4] {
		[Color::Maroon, Color::Green, Color::Blue, Color::Orange]
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeWeight {
	Single,
	Double,
}

impl EdgeWeight {
	pub fn value(&self) -> u8 {
		match self {
			EdgeWeight::Single => 1,
			EdgeWeight::Double => 2,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
	North,
	East,
	South,
	West,
}

impl Direction {
	pub fn all() -> [Direction; 4] {
		[Direction::North, Direction::East, Direction::South, Direction::West]
	}

	pub fn opposite(&self) -> Direction {
		match self {
			Direction::North => Direction::South,
			Direction::East => Direction::West,
			Direction::South => Direction::North,
			Direction::West => Direction::East,
		}
	}

	pub fn rotate_cw(&self) -> Direction {
		match self {
			Direction::North => Direction::East,
			Direction::East => Direction::South,
			Direction::South => Direction::West,
			Direction::West => Direction::North,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
	pub color: Color,
	pub weight: EdgeWeight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
	pub id: TileId,
	pub owner: Player,
	pub north: Edge,
	pub east: Edge,
	pub south: Edge,
	pub west: Edge,
}

impl Tile {
	pub fn new(
		id: TileId,
		owner: Player,
		north: Edge,
		east: Edge,
		south: Edge,
		west: Edge,
	) -> Self {
		Self { id, owner, north, east, south, west }
	}

	pub fn edge(&self, direction: Direction) -> Edge {
		match direction {
			Direction::North => self.north,
			Direction::East => self.east,
			Direction::South => self.south,
			Direction::West => self.west,
		}
	}

	pub fn rotate_cw(&self) -> Self {
		Self {
			id: self.id,
			owner: self.owner,
			north: self.west,
			east: self.north,
			south: self.east,
			west: self.south,
		}
	}

	pub fn rotated_n_times(&self, n: u8) -> Self {
		let mut tile = self.clone();
		for _ in 0..(n % 4) {
			tile = tile.rotate_cw();
		}
		tile
	}

	pub fn rotate_ccw(&self) -> Self {
		Self {
			id: self.id,
			owner: self.owner,
			north: self.east,
			east: self.south,
			south: self.west,
			west: self.north,
		}
	}
}

pub type TileId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Player {
	One,
	Two,
}

impl Player {
	pub fn other(&self) -> Player {
		match self {
			Player::One => Player::Two,
			Player::Two => Player::One,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
	pub row: i8,
	pub col: i8,
}

impl Position {
	pub fn new(row: i8, col: i8) -> Self {
		Self { row, col }
	}

	pub fn is_corner(&self) -> bool {
		(self.row == 0 || self.row == 4) && (self.col == 0 || self.col == 4)
	}

	pub fn is_valid(&self) -> bool {
		self.row >= 0 && self.row < 5 && self.col >= 0 && self.col < 5 && !self.is_corner()
	}

	pub fn neighbor(&self, direction: Direction) -> Position {
		match direction {
			Direction::North => Position::new(self.row - 1, self.col),
			Direction::East => Position::new(self.row, self.col + 1),
			Direction::South => Position::new(self.row + 1, self.col),
			Direction::West => Position::new(self.row, self.col - 1),
		}
	}

	pub fn neighbors(&self) -> Vec<(Direction, Position)> {
		Direction::all()
			.iter()
			.map(|&dir| (dir, self.neighbor(dir)))
			.filter(|(_, pos)| pos.row >= 0 && pos.row < 5 && pos.col >= 0 && pos.col < 5)
			.collect()
	}
}

#[derive(Debug, Clone)]
pub struct Board {
	tiles: HashMap<Position, Tile>,
}

impl Board {
	pub fn new() -> Self {
		Self { tiles: HashMap::new() }
	}

	pub fn get(&self, pos: Position) -> Option<&Tile> {
		self.tiles.get(&pos)
	}

	pub fn place(&mut self, pos: Position, tile: Tile) {
		assert!(pos.is_valid(), "Cannot place tile at invalid position {:?}", pos);
		self.tiles.insert(pos, tile);
	}

	pub fn remove(&mut self, pos: Position) -> Option<Tile> {
		self.tiles.remove(&pos)
	}

	pub fn find_tile(&self, tile_id: TileId) -> Option<Position> {
		self.tiles.iter().find(|(_, t)| t.id == tile_id).map(|(pos, _)| *pos)
	}

	pub fn tiles(&self) -> impl Iterator<Item = (&Position, &Tile)> {
		self.tiles.iter()
	}

	pub fn is_empty(&self, pos: Position) -> bool {
		!self.tiles.contains_key(&pos)
	}
}

impl Default for Board {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug, Clone)]
pub struct Game {
	pub board: Board,
	pub player_one_stack: Vec<Tile>,
	pub player_two_stack: Vec<Tile>,
	pub current_player: Player,
	pub state: GameState,
	next_tile_id: TileId,
	pub position_history: Vec<u64>,
	pub turn_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
	InProgress,
	Won(Player),
	Draw,
}

impl Game {
	pub fn new() -> Self {
		let mut game = Self {
			board: Board::new(),
			player_one_stack: Vec::new(),
			player_two_stack: Vec::new(),
			current_player: Player::One,
			state: GameState::InProgress,
			next_tile_id: 0,
			position_history: Vec::new(),
			turn_count: 0,
		};
		game.initialize_random_tiles();
		game
	}

	pub fn new_with_tiles(player_one_tiles: Vec<Tile>, player_two_tiles: Vec<Tile>) -> Self {
		let next_id = player_one_tiles
			.iter()
			.chain(player_two_tiles.iter())
			.map(|t| t.id)
			.max()
			.unwrap_or(0)
			+ 1;

		Self {
			board: Board::new(),
			player_one_stack: player_one_tiles,
			player_two_stack: player_two_tiles,
			current_player: Player::One,
			state: GameState::InProgress,
			next_tile_id: next_id,
			position_history: Vec::new(),
			turn_count: 0,
		}
	}

	fn initialize_random_tiles(&mut self) {
		use rand::Rng;
		let mut rng = rand::thread_rng();

		for player in [Player::One, Player::Two] {
			let mut tiles = Vec::new();
			for _ in 0..11 {
				let tile = self.generate_random_tile(&mut rng, player);
				tiles.push(tile);
			}
			let stack = self.stack_mut(player);
			stack.extend(tiles);
		}
	}

	fn generate_random_tile(&mut self, rng: &mut impl rand::Rng, player: Player) -> Tile {
		let id = self.next_tile_id;
		self.next_tile_id += 1;

		let mut random_edge = || {
			let colors = Color::all();
			let color = colors[rng.gen_range(0..colors.len())];
			let weight = if rng.gen_bool(0.2) { EdgeWeight::Double } else { EdgeWeight::Single };
			Edge { color, weight }
		};

		Tile::new(id, player, random_edge(), random_edge(), random_edge(), random_edge())
	}

	pub fn stack(&self, player: Player) -> &Vec<Tile> {
		match player {
			Player::One => &self.player_one_stack,
			Player::Two => &self.player_two_stack,
		}
	}

	pub fn stack_mut(&mut self, player: Player) -> &mut Vec<Tile> {
		match player {
			Player::One => &mut self.player_one_stack,
			Player::Two => &mut self.player_two_stack,
		}
	}

	pub fn is_game_over(&self) -> bool {
		matches!(self.state, GameState::Won(_) | GameState::Draw)
	}

	pub fn compute_position_hash(&self) -> u64 {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};

		let mut hasher = DefaultHasher::new();

		self.current_player.hash(&mut hasher);

		let mut positions: Vec<_> = self.board.tiles().collect();
		positions.sort_by_key(|(pos, _)| (pos.row, pos.col));
		for (pos, tile) in positions {
			pos.hash(&mut hasher);
			tile.id.hash(&mut hasher);
			tile.north.color.hash(&mut hasher);
			tile.north.weight.hash(&mut hasher);
			tile.east.color.hash(&mut hasher);
			tile.east.weight.hash(&mut hasher);
			tile.south.color.hash(&mut hasher);
			tile.south.weight.hash(&mut hasher);
			tile.west.color.hash(&mut hasher);
			tile.west.weight.hash(&mut hasher);
		}

		self.player_one_stack.len().hash(&mut hasher);
		self.player_two_stack.len().hash(&mut hasher);

		hasher.finish()
	}
}

impl Default for Game {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_position_corner_detection() {
		assert!(Position::new(0, 0).is_corner());
		assert!(Position::new(0, 4).is_corner());
		assert!(Position::new(4, 0).is_corner());
		assert!(Position::new(4, 4).is_corner());
		assert!(!Position::new(0, 2).is_corner());
		assert!(!Position::new(2, 2).is_corner());
	}

	#[test]
	fn test_position_validity() {
		assert!(!Position::new(0, 0).is_valid()); // corner
		assert!(!Position::new(-1, 0).is_valid()); // out of bounds
		assert!(!Position::new(5, 0).is_valid()); // out of bounds
		assert!(Position::new(2, 2).is_valid()); // center
		assert!(Position::new(0, 2).is_valid()); // edge but not corner
	}

	#[test]
	fn test_tile_rotation() {
		let tile = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Green, weight: EdgeWeight::Single },
			Edge { color: Color::Blue, weight: EdgeWeight::Single },
			Edge { color: Color::Orange, weight: EdgeWeight::Single },
		);

		let rotated = tile.rotate_cw();
		assert_eq!(rotated.north.color, Color::Orange);
		assert_eq!(rotated.east.color, Color::Maroon);
		assert_eq!(rotated.south.color, Color::Green);
		assert_eq!(rotated.west.color, Color::Blue);

		let rotated_back = rotated.rotate_ccw();
		assert_eq!(rotated_back.north.color, tile.north.color);
		assert_eq!(rotated_back.east.color, tile.east.color);
	}

	#[test]
	fn test_board_operations() {
		let mut board = Board::new();
		let tile = Tile::new(
			0,
			Player::One,
			Edge { color: Color::Maroon, weight: EdgeWeight::Single },
			Edge { color: Color::Green, weight: EdgeWeight::Single },
			Edge { color: Color::Blue, weight: EdgeWeight::Single },
			Edge { color: Color::Orange, weight: EdgeWeight::Single },
		);

		let pos = Position::new(2, 2);
		assert!(board.is_empty(pos));

		board.place(pos, tile.clone());
		assert!(!board.is_empty(pos));
		assert_eq!(board.get(pos).unwrap().id, tile.id);

		let removed = board.remove(pos);
		assert!(removed.is_some());
		assert!(board.is_empty(pos));
	}

	#[test]
	fn test_game_initialization() {
		let game = Game::new();
		assert_eq!(game.player_one_stack.len(), 11);
		assert_eq!(game.player_two_stack.len(), 11);
		assert_eq!(game.current_player, Player::One);
		assert_eq!(game.state, GameState::InProgress);
	}
}
