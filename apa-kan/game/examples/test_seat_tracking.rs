use apa_kan::game::{AiConfig, AiPlayer, BotType, Game, GameState, Player, RuleSet, Rules};

fn main() {
	println!("=== Seat Tracking Diagnostic ===\n");

	// Test with Old Rules
	let rule_set = RuleSet::old();
	println!("Testing with: {:?}", rule_set);
	println!(
		"Old rules: fixed_rotation={}, add_bank={}, add_bank_size={}\n",
		rule_set.fixed_rotation, rule_set.add_bank, rule_set.add_bank_size
	);

	let seed = 42u64;
	let depth = 2;

	// Create bots
	let olusare_p1 = AiPlayer::new_seeded(
		AiConfig {
			depth,
			bot_type: BotType::Olusare,
			time_budget: None,
			use_transposition_table: true,
			use_move_ordering: true,
		},
		seed,
	);
	let olusare_p2 = AiPlayer::new_seeded(
		AiConfig {
			depth,
			bot_type: BotType::Olusare,
			time_budget: None,
			use_transposition_table: true,
			use_move_ordering: true,
		},
		seed + 1,
	);

	// Play one game
	let mut game = Game::new_with_rules(rule_set);

	println!("Initial game state:");
	println!("  Current player: {:?}", game.current_player);
	println!("  P1 ADD bank: {}", game.add_bank_p1);
	println!("  P2 ADD bank: {}", game.add_bank_p2);
	println!("  Turn count: {}\n", game.turn_count);

	let mut move_num = 0;
	while game.turn_count < 100 && matches!(game.state, GameState::InProgress) {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { &olusare_p1 } else { &olusare_p2 };

		if let Some(action) = ai.choose_action(&game) {
			move_num += 1;
			if move_num <= 3 || game.turn_count >= 20 {
				println!(
					"Move {}: Player {:?} chooses {:?}",
					move_num, current_player, action.action_type
				);
			} else if move_num == 4 {
				println!("... (moves 4-20 omitted) ...");
			}

			if Rules::apply_action(&mut game, action).is_err() {
				println!("ERROR: Action failed!");
				break;
			}
		} else {
			println!("\nMove {}: Player {:?} has NO LEGAL MOVES", move_num + 1, current_player);
			println!("Player {:?} LOSES (cannot move)", current_player);
			println!("Player {:?} WINS", current_player.other());
			break;
		}
	}

	println!("\n=== Game Result ===");
	println!("Final state: {:?}", game.state);
	println!("Turn count: {}", game.turn_count);

	match game.state {
		GameState::Won(winner) => {
			println!("Winner: {:?}", winner);
			println!("First player (Player::One) won: {}", winner == Player::One);
			println!("Second player (Player::Two) won: {}", winner == Player::Two);
		}
		GameState::Draw => {
			println!("Game ended in draw");
		}
		GameState::InProgress => {
			println!("Game still in progress (but reached turn limit or error)");
		}
	}

	println!("\n=== Testing run_pairing logic ===");
	println!("In first half of run_pairing:");
	println!("  bot1 (Olusare) sits as Player::One (first seat)");
	println!("  bot2 (Olusare) sits as Player::Two (second seat)");
	match game.state {
		GameState::Won(winner) => {
			if winner == Player::One {
				println!("  → first_seat_wins should increment (bot1 won as P1)");
			} else {
				println!("  → second_seat_wins should increment (bot2 won as P2)");
			}
		}
		GameState::InProgress => {
			let current_player = game.current_player;
			let winner = current_player.other();
			if winner == Player::One {
				println!("  → first_seat_wins should increment");
			} else {
				println!("  → second_seat_wins should increment");
			}
		}
		_ => {}
	}

	println!("\n=== Bank behavior check ===");
	println!("Final P1 bank: {}", game.add_bank_p1);
	println!("Final P2 bank: {}", game.add_bank_p2);
	println!(
		"Did bank change from initial 100? {}",
		game.add_bank_p1 != 100 || game.add_bank_p2 != 100
	);
}
