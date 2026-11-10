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
	use apa_kan::game::Action;
	use apa_kan::ui::state::describe_action;

	let add = Action::add(apa_kan::game::Player::One, 0, Position::new(0, 2));
	assert_eq!(
		describe_action("Bot (Ologbon)", add, None, 0, false),
		"Bot (Ologbon) ADDed tile at row 0, col 2"
	);

	let mv = Action::move_tile(apa_kan::game::Player::One, 0, Position::new(1, 2), 0);
	assert_eq!(
		describe_action("You", mv, Some(Position::new(1, 0)), 1, false),
		"You MOVEd row 1 col 0 -> row 1 col 2, captured 1"
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
