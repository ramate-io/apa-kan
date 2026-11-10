use crate::config::GameConfig;
use crate::game::actions::generate_legal_actions;
use crate::game::core::{Player, Position};
use crate::game::rules::Rules as GameRules;
use crate::game::{Action, ActionType};
use crate::ui::board::GameResource;
use crate::ui::menu::GameScreen;
use crate::ui::state::{InputState, LastMove, Selection};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::PathBuf;
pub struct SnapshotPlugin;

impl Plugin for SnapshotPlugin {
	fn build(&self, app: &mut App) {
		if let Ok(dir) = std::env::var("APA_KAN_SNAPSHOT_DIR") {
			app.insert_resource(SnapshotJob {
				dir: PathBuf::from(dir),
				step: 0,
				frames_in_step: 0,
			})
			.add_systems(Update, drive_snapshots);
		}
	}
}

#[derive(Resource)]
struct SnapshotJob {
	dir: PathBuf,
	step: u32,
	frames_in_step: u32,
}

fn drive_snapshots(
	mut commands: Commands,
	mut job: ResMut<SnapshotJob>,
	mut game: ResMut<GameResource>,
	mut input: ResMut<InputState>,
	config: Res<GameConfig>,
	state: Res<State<GameScreen>>,
	mut exit: MessageWriter<AppExit>,
) {
	if *state.get() != GameScreen::Playing {
		return;
	}
	job.frames_in_step += 1;
	if job.frames_in_step < 20 {
		return;
	}

	match job.step {
		0 => {
			take(&mut commands, &job, "a-start-vs-ologbon.png");
			advance(&mut job);
		}
		1 => {
			if let Some(pos) = first_add_pos(&game.0) {
				input.selection = Selection::PendingAdd { position: pos };
			}
			advance(&mut job);
		}
		2 => {
			take(&mut commands, &job, "b-add-bench-highlighted.png");
			advance(&mut job);
		}
		3 => {
			setup_move_position(&mut game, &mut input, &config);
			advance(&mut job);
		}
		4 => {
			take(&mut commands, &job, "c-move-destinations.png");
			advance(&mut job);
		}
		5 => {
			input.selection = Selection::None;
			if input.last_move.is_none() {
				input.last_move = Some(LastMove {
					text: "Bot (Ologbon) ADDed tile at row 0, col 2".into(),
					squares: vec![Position::new(0, 2)],
				});
			}
			advance(&mut job);
		}
		6 => {
			take(&mut commands, &job, "d-after-bot-move.png");
			advance(&mut job);
		}
		7 => {
			if job.frames_in_step > 45 {
				exit.write(AppExit::Success);
			}
		}
		_ => {}
	}
}

fn advance(job: &mut SnapshotJob) {
	job.step += 1;
	job.frames_in_step = 0;
}

fn take(commands: &mut Commands, job: &SnapshotJob, name: &str) {
	let path = job.dir.join(name);
	commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

fn first_add_pos(game: &crate::game::core::Game) -> Option<Position> {
	generate_legal_actions(game)
		.into_iter()
		.find_map(|action| match action.action_type {
			ActionType::Add { position, .. } => Some(position),
			_ => None,
		})
}

fn setup_move_position(game: &mut GameResource, input: &mut InputState, config: &GameConfig) {
	let p1_pos = Position::new(0, 2);
	let p2_pos = Position::new(0, 1);
	if let Some(stable) = find_stable_adjacent_pair(&game.0, p1_pos, p2_pos) {
		*game = GameResource(stable);
	}
	if let Some((from, tile)) = game.0.board.tiles().find(|(_, t)| t.owner == Player::One) {
		input.selection = Selection::MoveTile { tile_id: tile.id, from: *from };
		input.rotation = 0;
	}
	if let Some(last) = game.0.board.tiles().find(|(pos, _)| **pos == p2_pos) {
		input.last_move = Some(LastMove {
			text: format!("P2 Bot (Ologbon) ADDed tile at row {}, col {}", last.0.row, last.0.col),
			squares: vec![*last.0],
		});
	}
	let _ = config;
}

fn find_stable_adjacent_pair(
	start: &crate::game::core::Game,
	p1_pos: Position,
	p2_pos: Position,
) -> Option<crate::game::core::Game> {
	let n1 = start.stack(Player::One).len();
	let n2 = start.stack(Player::Two).len();
	for i in 0..n1 {
		let mut g1 = start.clone();
		if g1.current_player != Player::One {
			continue;
		}
		if GameRules::apply_action(&mut g1, Action::add(Player::One, i, p1_pos)).is_err() {
			continue;
		}
		if g1.board.get(p1_pos).is_none() {
			continue;
		}
		for j in 0..n2.min(g1.stack(Player::Two).len()) {
			if g1.current_player != Player::Two {
				break;
			}
			let mut g2 = g1.clone();
			if GameRules::apply_action(&mut g2, Action::add(Player::Two, j, p2_pos)).is_err() {
				continue;
			}
			if g2.board.get(p1_pos).is_some() && g2.board.get(p2_pos).is_some() {
				return Some(g2);
			}
		}
	}
	None
}
