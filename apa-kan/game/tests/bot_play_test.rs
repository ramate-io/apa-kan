use apa_kan::config::{GameConfig, Rules};
use apa_kan::game::actions::generate_legal_actions;
use apa_kan::game::{ActionType, AiConfig, AiPlayer, BotType, Game, Rules as GameRules};
use std::time::Duration;

#[test]
fn test_config_parsing() {
	let config = GameConfig::default();
	assert_eq!(config.bot, None);
	assert_eq!(config.human_seat, 1);
	assert_eq!(config.board_size, 5);
	assert_eq!(config.rules, Rules::Scored);
	assert_eq!(config.think_ms, 500);
}

#[test]
fn test_ruleset_mapping_scored_5x5() {
	let mut config = GameConfig::default();
	config.rules = Rules::Scored;
	config.board_size = 5;

	let rule_set = config.rule_set();
	assert_eq!(rule_set.board_size, 5);
	assert_eq!(rule_set.scored_mode, true);
	assert_eq!(rule_set.add_bank_size, 5);
}

#[test]
fn test_ruleset_mapping_scored_7x7() {
	let mut config = GameConfig::default();
	config.rules = Rules::Scored;
	config.board_size = 7;

	let rule_set = config.rule_set();
	assert_eq!(rule_set.board_size, 7);
	assert_eq!(rule_set.scored_mode, true);
	assert_eq!(rule_set.add_bank_size, 5);
}

#[test]
fn test_ruleset_mapping_old() {
	let mut config = GameConfig::default();
	config.rules = Rules::Old;

	let rule_set = config.rule_set();
	assert_eq!(rule_set.board_size, 5);
	assert_eq!(rule_set.scored_mode, false);
	assert_eq!(rule_set.add_bank_size, 100);
}

#[test]
fn test_bot_vs_human_stub_completes() {
	let config = GameConfig {
		bot: Some(BotType::Ore),
		human_seat: 1,
		board_size: 5,
		rules: Rules::Scored,
		think_ms: 10,
	};

	let mut game = Game::new_with_rules(config.rule_set());
	let bot = AiPlayer::new(AiConfig {
		depth: 3,
		bot_type: BotType::Ore,
		time_budget: Some(Duration::from_millis(10)),
		use_transposition_table: true,
		use_move_ordering: true,
		mcts_iterations: None,
		mcts_exploration: 1.41,
		mcts_rollout_depth: 20,
	});

	let mut turn_count = 0;
	while !game.is_game_over() && turn_count < 200 {
		if let Some(action) = bot.choose_action(&game) {
			if GameRules::apply_action(&mut game, action).is_err() {
				break;
			}
		} else {
			break;
		}
		turn_count += 1;
	}

	assert!(game.is_game_over() || turn_count >= 100);
}

#[test]
fn test_all_bots_can_play() {
	let bot_types = vec![BotType::Ore, BotType::Olusare, BotType::Ologbon, BotType::Mcts];

	for bot_type in bot_types {
		let config = GameConfig {
			bot: Some(bot_type),
			human_seat: 1,
			board_size: 5,
			rules: Rules::Scored,
			think_ms: 10,
		};

		let mut game = Game::new_with_rules(config.rule_set());
		let bot = AiPlayer::new(AiConfig {
			depth: 3,
			bot_type,
			time_budget: Some(Duration::from_millis(10)),
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		});

		let mut moves = 0;
		while !game.is_game_over() && moves < 20 {
			if let Some(action) = bot.choose_action(&game) {
				if GameRules::apply_action(&mut game, action).is_ok() {
					moves += 1;
				} else {
					break;
				}
			} else {
				break;
			}
		}

		assert!(moves > 0, "Bot {:?} failed to make any moves", bot_type.name());
	}
}

#[test]
fn test_scored_legal_adds_are_non_corner_edges() {
	let game = Game::new_with_rules(apa_kan::game::RuleSet::scored());
	let actions = generate_legal_actions(&game);
	let adds: Vec<_> = actions
		.iter()
		.filter_map(|a| match a.action_type {
			ActionType::Add { position, .. } => Some(position),
			_ => None,
		})
		.collect();
	assert!(!adds.is_empty());
	for pos in adds {
		assert!(game.board.is_edge(pos), "ADD at {pos:?} is not an edge");
		assert!(!game.board.is_corner(pos), "ADD at {pos:?} is a corner");
	}
}

#[test]
fn test_action_log_plain_words() {
	use apa_kan::game::core::Position;
	use apa_kan::game::{Action, Player, TileRemoval};
	use apa_kan::ui::state::describe_action;

	let add = Action::add(apa_kan::game::Player::One, 0, Position::new(0, 2));
	assert_eq!(
		describe_action("Bot (Ologbon)", add, None, 0, &[]),
		"Bot (Ologbon) ADDed tile at row 0, col 2"
	);

	let mv = Action::move_tile(apa_kan::game::Player::One, 0, Position::new(1, 2), 0);
	assert_eq!(
		describe_action("You", mv, Some(Position::new(1, 0)), 1, &[]),
		"You MOVEd row 1 col 0 -> row 1 col 2, captured 1"
	);

	let both = [
		TileRemoval { position: Position::new(1, 0), owner: Player::Two },
		TileRemoval { position: Position::new(2, 0), owner: Player::One },
	];
	let wipe = Action::add(Player::Two, 0, Position::new(1, 0));
	assert_eq!(
		describe_action("P2 Bot (Ologbon)", wipe, None, 0, &both),
		"P2 Bot (Ologbon) ADDed tile at row 1, col 0; both tiles removed (mismatch): P2 at row 1, col 0 and P1 at row 2, col 0"
	);
}

#[test]
fn test_can_place_two_adjacent_surviving_tiles() {
	use apa_kan::game::core::Position;
	use apa_kan::game::Action;
	use apa_kan::game::Player;

	let start = Game::new_with_rules(apa_kan::game::RuleSet::scored());
	let p1_pos = Position::new(0, 2);
	let p2_pos = Position::new(0, 1);
	let mut found = false;
	for i in 0..start.stack(Player::One).len() {
		let mut g1 = start.clone();
		if GameRules::apply_action(&mut g1, Action::add(Player::One, i, p1_pos)).is_err() {
			continue;
		}
		if g1.board.get(p1_pos).is_none() {
			continue;
		}
		for j in 0..g1.stack(Player::Two).len() {
			let mut g2 = g1.clone();
			if g2.current_player != Player::Two {
				break;
			}
			if GameRules::apply_action(&mut g2, Action::add(Player::Two, j, p2_pos)).is_err() {
				continue;
			}
			if g2.board.get(p1_pos).is_some() && g2.board.get(p2_pos).is_some() {
				found = true;
				break;
			}
		}
		if found {
			break;
		}
	}
	assert!(found, "should find an adjacent ADD pair that both survive");
}

fn solid_tile(
	id: u32,
	owner: apa_kan::game::Player,
	color: apa_kan::game::Color,
) -> apa_kan::game::Tile {
	use apa_kan::game::core::{Edge, EdgeWeight, Tile};
	let edge = Edge { color, weight: EdgeWeight::Single };
	Tile::new(id, owner, edge, edge, edge, edge)
}

fn occupancy(game: &Game) -> std::collections::HashMap<(i8, i8), apa_kan::game::Player> {
	game.board.tiles().map(|(pos, tile)| ((pos.row, pos.col), tile.owner)).collect()
}

fn assert_log_matches_board_diff(
	before: &std::collections::HashMap<(i8, i8), apa_kan::game::Player>,
	after: &std::collections::HashMap<(i8, i8), apa_kan::game::Player>,
	logged: &apa_kan::ui::state::LoggedAction,
) {
	for (pos, owner) in before {
		let still_there = after.get(pos) == Some(owner);
		let named = logged
			.removals
			.iter()
			.any(|r| r.position.row == pos.0 && r.position.col == pos.1 && r.owner == *owner);
		if still_there {
			assert!(
				!named,
				"log claims {owner:?} at {pos:?} was removed, but it is still on the board: {}",
				logged.text
			);
		} else {
			assert!(
				named,
				"{owner:?} tile at {pos:?} vanished but the log did not name it: {}",
				logged.text
			);
			let mention = format!("row {}, col {}", pos.0, pos.1);
			assert!(
				logged.text.contains(&mention),
				"{owner:?} tile at {mention} vanished but log did not name it: {}",
				logged.text
			);
		}
	}

	for removal in &logged.removals {
		let key = (removal.position.row, removal.position.col);
		assert!(
			!after.contains_key(&key),
			"log lists {key:?} as removed, but it is still occupied: {}",
			logged.text
		);
		let mention = format!("row {}, col {}", removal.position.row, removal.position.col);
		assert!(
			logged.text.contains(&mention),
			"removal at {mention} missing from log text: {}",
			logged.text
		);
	}

	if logged.removals.is_empty() {
		for (pos, owner) in before {
			assert_eq!(
				after.get(pos),
				Some(owner),
				"tile at {pos:?} vanished even though no removal was logged"
			);
		}
	}
}

#[test]
fn test_human_tile_at_2_0_vanishes_from_mutual_mismatch() {
	use apa_kan::game::core::Position;
	use apa_kan::game::{Action, Color, Player};
	use apa_kan::ui::state::log_applied_action;

	let p1: Vec<_> = (0..5).map(|i| solid_tile(i, Player::One, Color::Maroon)).collect();
	let p2: Vec<_> = (10..15).map(|i| solid_tile(i, Player::Two, Color::Green)).collect();
	let mut game = Game::new_with_tiles_and_rules(p1, p2, apa_kan::game::RuleSet::scored());

	for round in 0..3 {
		assert_eq!(game.current_player, Player::One);
		let before = occupancy(&game);
		let logged = log_applied_action(
			&mut game,
			"P1 (You)",
			Action::add(Player::One, 0, Position::new(2, 0)),
		)
		.unwrap_or_else(|e| panic!("round {round} P1 ADD failed: {e}"));
		let after = occupancy(&game);
		assert_log_matches_board_diff(&before, &after, &logged);
		assert!(
			game.board.get(Position::new(2, 0)).is_some(),
			"round {round}: P1 ADD at (2,0) must stay until the bot replies"
		);
		assert!(
			!logged.text.contains("removed"),
			"round {round}: P1's surviving ADD must not be logged as removed: {}",
			logged.text
		);

		assert_eq!(game.current_player, Player::Two);
		let before = occupancy(&game);
		let dest = if round % 2 == 0 { Position::new(1, 0) } else { Position::new(3, 0) };
		let logged =
			log_applied_action(&mut game, "P2 Bot (Ologbon)", Action::add(Player::Two, 0, dest))
				.unwrap_or_else(|e| panic!("round {round} P2 ADD failed: {e}"));
		let after = occupancy(&game);
		assert_log_matches_board_diff(&before, &after, &logged);
		assert!(
			game.board.get(Position::new(2, 0)).is_none(),
			"round {round}: P1 tile at (2,0) is removed by mutual mismatch"
		);
		assert!(
			game.board.get(dest).is_none(),
			"round {round}: P2 tile at {dest:?} is also removed"
		);
		assert!(
			logged.text.contains("both tiles removed (mismatch)"),
			"round {round}: log must name the mutual wipe: {}",
			logged.text
		);
		assert!(
			logged.text.contains("row 2, col 0"),
			"round {round}: log must name the human tile: {}",
			logged.text
		);
	}
}

#[test]
fn test_matching_adjacent_tiles_stay_on_board() {
	use apa_kan::game::core::{Edge, EdgeWeight, Position, Tile};
	use apa_kan::game::{Action, Color, Player};
	use apa_kan::ui::state::log_applied_action;

	let edge = |color| Edge { color, weight: EdgeWeight::Single };
	let p1 = Tile::new(
		0,
		Player::One,
		edge(Color::Maroon),
		edge(Color::Blue),
		edge(Color::Orange),
		edge(Color::Green),
	);
	let p2 = Tile::new(
		1,
		Player::Two,
		edge(Color::Blue),
		edge(Color::Orange),
		edge(Color::Maroon),
		edge(Color::Green),
	);
	let mut game =
		Game::new_with_tiles_and_rules(vec![p1], vec![p2], apa_kan::game::RuleSet::scored());

	let before = occupancy(&game);
	let logged =
		log_applied_action(&mut game, "P1 (You)", Action::add(Player::One, 0, Position::new(2, 0)))
			.expect("P1 ADD");
	assert_log_matches_board_diff(&before, &occupancy(&game), &logged);
	assert!(game.board.get(Position::new(2, 0)).is_some());

	let before = occupancy(&game);
	let logged = log_applied_action(
		&mut game,
		"P2 Bot (Ologbon)",
		Action::add(Player::Two, 0, Position::new(1, 0)),
	)
	.expect("P2 ADD");
	assert_log_matches_board_diff(&before, &occupancy(&game), &logged);
	assert!(
		logged.removals.is_empty(),
		"matching colors plus corner tie must keep both tiles: {}",
		logged.text
	);
	assert!(game.board.get(Position::new(2, 0)).is_some());
	assert!(game.board.get(Position::new(1, 0)).is_some());
}

#[test]
fn test_scripted_human_vs_ologbon_logs_match_board() {
	use apa_kan::game::core::Position;
	use apa_kan::game::{Action, Color, Player, RuleSet};
	use apa_kan::ui::state::log_applied_action;

	let p1: Vec<_> = (0..8).map(|i| solid_tile(i, Player::One, Color::Maroon)).collect();
	let p2: Vec<_> = (20..28).map(|i| solid_tile(i, Player::Two, Color::Green)).collect();
	let mut game = Game::new_with_tiles_and_rules(p1, p2, RuleSet::scored());

	let bot = AiPlayer::new_seeded(
		AiConfig {
			depth: 3,
			bot_type: BotType::Ologbon,
			time_budget: Some(Duration::from_millis(40)),
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		},
		7,
	);

	let human_adds =
		[Position::new(2, 0), Position::new(0, 2), Position::new(2, 4), Position::new(4, 2)];
	let mut human_add_i = 0;

	for turn in 0..8 {
		if game.is_game_over() {
			break;
		}
		let before = occupancy(&game);
		let (who, action) = if game.current_player == Player::One {
			let pos = human_adds[human_add_i % human_adds.len()];
			human_add_i += 1;
			let action = if game.board.is_empty(pos) {
				Action::add(Player::One, 0, pos)
			} else {
				Action::pass(Player::One)
			};
			("P1 (You)", action)
		} else {
			let action = bot.choose_action(&game).expect("Ologbon should choose");
			("P2 Bot (Ologbon)", action)
		};
		let logged = log_applied_action(&mut game, who, action)
			.unwrap_or_else(|e| panic!("turn {turn} {who} failed: {e}"));
		assert_log_matches_board_diff(&before, &occupancy(&game), &logged);
	}
}

#[test]
fn test_malformed_cli_arg_exits_nonzero() {
	let Some(exe) = option_env!("CARGO_BIN_EXE_apa_kan") else {
		let err = GameConfig::parse_args(["apa-kan", "--bot", "ologbon", "--rulesscored"])
			.expect_err("glued --rulesscored must fail");
		assert!(err.contains("--rulesscored"), "{err}");
		return;
	};
	let output = std::process::Command::new(exe)
		.args(["--bot", "ologbon", "--seat", "1", "--size", "5", "--rulesscored"])
		.output()
		.expect("spawn apa-kan");
	assert!(!output.status.success(), "malformed --rulesscored must exit nonzero");
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("--rulesscored"), "stderr: {stderr}");
	assert!(stderr.to_lowercase().contains("usage"), "stderr: {stderr}");
}
