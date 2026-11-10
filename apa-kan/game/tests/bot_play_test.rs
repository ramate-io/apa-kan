use apa_kan::config::{GameConfig, Rules};
use apa_kan::game::{AiConfig, AiPlayer, BotType, Game, Rules as GameRules};
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
