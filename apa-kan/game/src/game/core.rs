use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationDirection {
	Clockwise,
	CounterClockwise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
	pub id: TileId,
	pub owner: Player,
	pub north: Edge,
	pub east: Edge,
	pub south: Edge,
	pub west: Edge,
	pub rotation_direction: RotationDirection,
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
		use rand::Rng;
		let rotation_direction = if rand::thread_rng().gen_bool(0.5) {
			RotationDirection::Clockwise
		} else {
			RotationDirection::CounterClockwise
		};
		Self { id, owner, north, east, south, west, rotation_direction }
	}

	pub fn new_with_rotation(
		id: TileId,
		owner: Player,
		north: Edge,
		east: Edge,
		south: Edge,
		west: Edge,
		rotation_direction: RotationDirection,
	) -> Self {
		Self { id, owner, north, east, south, west, rotation_direction }
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
			rotation_direction: self.rotation_direction,
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
			rotation_direction: self.rotation_direction,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleSet {
	pub fixed_rotation: bool,
	pub add_bank: bool,
	pub add_bank_size: u8,
	pub moved_tile_immunity: bool,
	pub edge_add_only: bool,
	pub scored_mode: bool,
	pub board_size: u8,
	pub stack_size: u8,
	/// How many ADD/MOVE actions a player may take in one turn. Existing
	/// rulesets keep `1`. Two-action variants set this to `2`.
	pub actions_per_turn: u8,
	/// When true and `actions_per_turn > 1`, Player One's first turn is only
	/// one action (Connect6-style opening). Default on for two-action constructors.
	pub opening_handicap: bool,
}

impl RuleSet {
	pub fn old() -> Self {
		Self {
			fixed_rotation: false,
			add_bank: false,
			add_bank_size: 100,
			moved_tile_immunity: false,
			edge_add_only: false,
			scored_mode: false,
			board_size: 5,
			stack_size: 11,
			actions_per_turn: 1,
			opening_handicap: false,
		}
	}

	pub fn new() -> Self {
		Self {
			fixed_rotation: true,
			add_bank: true,
			add_bank_size: 7,
			moved_tile_immunity: false,
			edge_add_only: true,
			scored_mode: false,
			board_size: 5,
			stack_size: 11,
			actions_per_turn: 1,
			opening_handicap: false,
		}
	}

	pub fn scored() -> Self {
		Self {
			fixed_rotation: true,
			add_bank: true,
			add_bank_size: 5,
			moved_tile_immunity: false,
			edge_add_only: true,
			scored_mode: true,
			board_size: 5,
			stack_size: 11,
			actions_per_turn: 1,
			opening_handicap: false,
		}
	}

	pub fn scored_7x7() -> Self {
		Self { board_size: 7, ..Self::scored() }
	}

	pub fn scored_double() -> Self {
		Self { actions_per_turn: 2, opening_handicap: true, ..Self::scored() }
	}

	pub fn scored_7x7_double() -> Self {
		Self { board_size: 7, ..Self::scored_double() }
	}

	pub fn old_double() -> Self {
		Self { actions_per_turn: 2, opening_handicap: true, ..Self::old() }
	}

	pub fn with_bank_size(bank_size: u8) -> Self {
		Self {
			fixed_rotation: true,
			add_bank: true,
			add_bank_size: bank_size,
			moved_tile_immunity: false,
			edge_add_only: true,
			scored_mode: false,
			board_size: 5,
			stack_size: 11,
			actions_per_turn: 1,
			opening_handicap: false,
		}
	}

	pub fn with_immunity() -> Self {
		Self {
			fixed_rotation: true,
			add_bank: true,
			add_bank_size: 7,
			moved_tile_immunity: true,
			edge_add_only: true,
			scored_mode: false,
			board_size: 5,
			stack_size: 11,
			actions_per_turn: 1,
			opening_handicap: false,
		}
	}
}

impl Default for RuleSet {
	fn default() -> Self {
		Self::new()
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

	pub fn neighbor(&self, direction: Direction) -> Position {
		match direction {
			Direction::North => Position::new(self.row - 1, self.col),
			Direction::East => Position::new(self.row, self.col + 1),
			Direction::South => Position::new(self.row + 1, self.col),
			Direction::West => Position::new(self.row, self.col - 1),
		}
	}

	pub fn neighbors(&self) -> Vec<(Direction, Position)> {
		self.neighbors_in(5)
	}

	pub fn neighbors_in(&self, size: u8) -> Vec<(Direction, Position)> {
		let max = size as i8;
		Direction::all()
			.iter()
			.map(|&dir| (dir, self.neighbor(dir)))
			.filter(|(_, pos)| pos.row >= 0 && pos.row < max && pos.col >= 0 && pos.col < max)
			.collect()
	}
}

#[derive(Debug, Clone)]
pub struct Board {
	tiles: HashMap<Position, Tile>,
	size: u8,
}

impl Board {
	pub fn new(size: u8) -> Self {
		Self { tiles: HashMap::new(), size }
	}

	pub fn size(&self) -> u8 {
		self.size
	}

	pub fn is_valid(&self, pos: Position) -> bool {
		let max = self.size as i8;
		pos.row >= 0 && pos.row < max && pos.col >= 0 && pos.col < max && !self.is_corner(pos)
	}

	pub fn is_corner(&self, pos: Position) -> bool {
		let max = (self.size - 1) as i8;
		(pos.row == 0 || pos.row == max) && (pos.col == 0 || pos.col == max)
	}

	pub fn is_edge(&self, pos: Position) -> bool {
		if self.is_corner(pos) {
			return false;
		}
		let max = (self.size - 1) as i8;
		pos.row == 0 || pos.row == max || pos.col == 0 || pos.col == max
	}

	pub fn get(&self, pos: Position) -> Option<&Tile> {
		self.tiles.get(&pos)
	}

	pub fn place(&mut self, pos: Position, tile: Tile) {
		assert!(self.is_valid(pos), "Cannot place tile at invalid position {:?}", pos);
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
		Self::new(5)
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
	pub rule_set: RuleSet,
	pub add_bank_p1: u8,
	pub add_bank_p2: u8,
	pub last_moved_tile: Option<TileId>,
	pub last_moved_turn: usize,
	pub consecutive_passes: u8,
	pub p1_has_added: bool,
	pub p2_has_added: bool,
	/// Actions already taken by `current_player` during this turn.
	pub actions_this_turn: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
	InProgress,
	Won(Player),
	Draw,
}

impl Game {
	pub fn new() -> Self {
		Self::new_with_rules(RuleSet::default())
	}

	pub fn new_with_rules(rule_set: RuleSet) -> Self {
		let add_bank = if rule_set.add_bank { rule_set.add_bank_size } else { 100 };
		let mut game = Self {
			board: Board::new(rule_set.board_size),
			player_one_stack: Vec::new(),
			player_two_stack: Vec::new(),
			current_player: Player::One,
			state: GameState::InProgress,
			next_tile_id: 0,
			position_history: Vec::new(),
			turn_count: 0,
			rule_set,
			add_bank_p1: add_bank,
			add_bank_p2: add_bank,
			last_moved_tile: None,
			last_moved_turn: 0,
			consecutive_passes: 0,
			p1_has_added: false,
			p2_has_added: false,
			actions_this_turn: 0,
		};
		game.initialize_random_tiles();
		game
	}

	pub fn new_with_tiles(player_one_tiles: Vec<Tile>, player_two_tiles: Vec<Tile>) -> Self {
		Self::new_with_tiles_and_rules(player_one_tiles, player_two_tiles, RuleSet::default())
	}

	pub fn new_with_tiles_and_rules(
		player_one_tiles: Vec<Tile>,
		player_two_tiles: Vec<Tile>,
		rule_set: RuleSet,
	) -> Self {
		let next_id = player_one_tiles
			.iter()
			.chain(player_two_tiles.iter())
			.map(|t| t.id)
			.max()
			.unwrap_or(0)
			+ 1;

		let add_bank = if rule_set.add_bank { rule_set.add_bank_size } else { 100 };

		Self {
			board: Board::new(rule_set.board_size),
			player_one_stack: player_one_tiles,
			player_two_stack: player_two_tiles,
			current_player: Player::One,
			state: GameState::InProgress,
			next_tile_id: next_id,
			position_history: Vec::new(),
			turn_count: 0,
			rule_set,
			add_bank_p1: add_bank,
			add_bank_p2: add_bank,
			last_moved_tile: None,
			last_moved_turn: 0,
			consecutive_passes: 0,
			p1_has_added: false,
			p2_has_added: false,
			actions_this_turn: 0,
		}
	}

	fn initialize_random_tiles(&mut self) {
		let mut rng = rand::thread_rng();

		for player in [Player::One, Player::Two] {
			let mut tiles = Vec::new();
			for _ in 0..self.rule_set.stack_size {
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

	pub fn actions_allowed_this_turn(&self) -> u8 {
		let n = self.rule_set.actions_per_turn.max(1);
		if n > 1
			&& self.rule_set.opening_handicap
			&& self.turn_count == 0
			&& self.current_player == Player::One
		{
			1
		} else {
			n
		}
	}

	pub fn action_number_this_turn(&self) -> u8 {
		self.actions_this_turn.saturating_add(1)
	}

	pub fn compute_position_hash(&self) -> u64 {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};

		let mut hasher = DefaultHasher::new();

		self.current_player.hash(&mut hasher);
		self.actions_this_turn.hash(&mut hasher);

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
		let board = Board::new(5);
		assert!(board.is_corner(Position::new(0, 0)));
		assert!(board.is_corner(Position::new(0, 4)));
		assert!(board.is_corner(Position::new(4, 0)));
		assert!(board.is_corner(Position::new(4, 4)));
		assert!(!board.is_corner(Position::new(0, 2)));
		assert!(!board.is_corner(Position::new(2, 2)));
	}

	#[test]
	fn test_position_validity() {
		let board = Board::new(5);
		assert!(!board.is_valid(Position::new(0, 0))); // corner
		assert!(!board.is_valid(Position::new(-1, 0))); // out of bounds
		assert!(!board.is_valid(Position::new(5, 0))); // out of bounds
		assert!(board.is_valid(Position::new(2, 2))); // center
		assert!(board.is_valid(Position::new(0, 2))); // edge but not corner
	}

	#[test]
	fn test_position_edge_detection() {
		let board = Board::new(5);
		// Corners are not edges
		assert!(!board.is_edge(Position::new(0, 0)));
		assert!(!board.is_edge(Position::new(0, 4)));
		assert!(!board.is_edge(Position::new(4, 0)));
		assert!(!board.is_edge(Position::new(4, 4)));

		// Top edge (row 0, not corners)
		assert!(board.is_edge(Position::new(0, 1)));
		assert!(board.is_edge(Position::new(0, 2)));
		assert!(board.is_edge(Position::new(0, 3)));

		// Bottom edge (row 4, not corners)
		assert!(board.is_edge(Position::new(4, 1)));
		assert!(board.is_edge(Position::new(4, 2)));
		assert!(board.is_edge(Position::new(4, 3)));

		// Left edge (col 0, not corners)
		assert!(board.is_edge(Position::new(1, 0)));
		assert!(board.is_edge(Position::new(2, 0)));
		assert!(board.is_edge(Position::new(3, 0)));

		// Right edge (col 4, not corners)
		assert!(board.is_edge(Position::new(1, 4)));
		assert!(board.is_edge(Position::new(2, 4)));
		assert!(board.is_edge(Position::new(3, 4)));

		// Interior is not edge
		assert!(!board.is_edge(Position::new(1, 1)));
		assert!(!board.is_edge(Position::new(2, 2)));
		assert!(!board.is_edge(Position::new(3, 3)));
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
		let mut board = Board::new(5);
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
