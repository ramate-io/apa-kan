//! Bevy 0.19 desktop UI: both benches, ADD/MOVE targeting, last-action log.

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::ai;
use crate::board::NodeId;
use crate::config::GameConfig;
use crate::game::{Action, Game, GameStatus, Player, Tile, TileColor};

const SCALE: f32 = 210.0;
const TILE_R: f32 = 22.0;
const NODE_R: f32 = 8.0;

pub fn run_app(cfg: GameConfig) {
	let game = Game::new(cfg.rules, cfg.seed);
	let rng = StdRng::seed_from_u64(cfg.seed ^ 0xA5A5_A5A5);
	App::new()
		.add_plugins(DefaultPlugins.set(WindowPlugin {
			primary_window: Some(Window {
				title: "Apa Kan".into(),
				resolution: (1280, 900).into(),
				..default()
			}),
			..default()
		}))
		.insert_resource(ClearColor(Color::srgb(0.07, 0.08, 0.10)))
		.insert_resource(cfg)
		.insert_resource(PlayState {
			game,
			selection: Selection::None,
			log: "Click a spawn then a bench tile to ADD, or click your tile to MOVE.".into(),
			warning: String::new(),
			last_removals: String::new(),
			fingerprint: 0,
		})
		.insert_resource(RngRes(rng))
		.insert_resource(BotClock { wait: 0.4 })
		.add_systems(Startup, setup)
		.add_systems(Update, (draw_gizmos, handle_click, bot_turn, refresh_view, update_hud))
		.run();
}

#[derive(Resource)]
struct PlayState {
	game: Game,
	selection: Selection,
	log: String,
	warning: String,
	last_removals: String,
	fingerprint: u64,
}

#[derive(Resource)]
struct RngRes(StdRng);

#[derive(Resource)]
struct BotClock {
	wait: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Selection {
	None,
	Bench(usize),
	Spawn(NodeId),
	Piece(NodeId),
}

#[derive(Component)]
struct DynamicView;

#[derive(Component)]
struct StatusHud;

#[derive(Component)]
struct LogHud;

#[derive(Component)]
struct WarnHud;

fn setup(mut commands: Commands) {
	commands.spawn(Camera2d::default());
	spawn_hud_line(&mut commands, StatusHud, 16.0, 14.0, 22.0, Color::srgb(0.92, 0.93, 0.90));
	spawn_hud_line(&mut commands, LogHud, 16.0, 44.0, 16.0, Color::srgb(0.78, 0.82, 0.72));
	spawn_hud_line(&mut commands, WarnHud, 16.0, 70.0, 16.0, Color::srgb(0.95, 0.72, 0.28));
}

fn spawn_hud_line<M: Component>(
	commands: &mut Commands,
	marker: M,
	left: f32,
	top: f32,
	size: f32,
	color: Color,
) {
	commands.spawn((
		marker,
		Text::new(""),
		TextFont { font_size: FontSize::Px(size), ..default() },
		TextColor(color),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(top),
			left: Val::Px(left),
			width: Val::Px(1240.0),
			..default()
		},
	));
}

fn draw_gizmos(mut gizmos: Gizmos, play: Res<PlayState>) {
	let board = play.game.board();
	let arc_color = Color::srgb(0.38, 0.40, 0.45);
	for arc in board.arcs() {
		let pts = arc.sample(24);
		for w in pts.windows(2) {
			gizmos.line_2d(world(w[0]), world(w[1]), arc_color);
		}
	}
	let dests: Vec<NodeId> = match play.selection {
		Selection::Piece(n) => play.game.destinations(n),
		_ => Vec::new(),
	};
	for id in 0..board.node_count() as NodeId {
		let p = world(board.position(id));
		if dests.contains(&id) {
			gizmos.circle_2d(p, TILE_R + 6.0, Color::srgb(0.55, 0.85, 1.0));
		}
		if let Selection::Spawn(s) = play.selection {
			if s == id {
				gizmos.circle_2d(p, TILE_R + 8.0, Color::srgb(0.95, 0.82, 0.20));
			}
		}
		if play.game.tile_at(id).is_none() {
			if board.is_spawn(id) {
				gizmos.circle_2d(p, NODE_R + 3.0, Color::srgb(0.93, 0.84, 0.32));
			} else if id == board.center() {
				gizmos.circle_2d(p, NODE_R + 2.0, Color::srgb(0.85, 0.85, 0.85));
			} else {
				gizmos.circle_2d(p, NODE_R, Color::srgb(0.75, 0.75, 0.75));
			}
		}
	}
}

fn world(p: [f64; 2]) -> Vec2 {
	Vec2::new(p[0] as f32 * SCALE, p[1] as f32 * SCALE)
}

fn handle_click(
	buttons: Res<ButtonInput<MouseButton>>,
	windows: Query<&Window>,
	cameras: Query<(&Camera, &GlobalTransform)>,
	mut play: ResMut<PlayState>,
	cfg: Res<GameConfig>,
	keys: Res<ButtonInput<KeyCode>>,
) {
	if keys.just_pressed(KeyCode::Escape) {
		play.selection = Selection::None;
		play.warning.clear();
		return;
	}
	if !buttons.just_pressed(MouseButton::Left) {
		return;
	}
	if play.game.status().is_over() {
		return;
	}
	let actor = cfg.actor(play.game.current_player());
	if !actor.is_human() {
		return;
	}
	let Some(window) = windows.iter().next() else {
		return;
	};
	let Some(cursor) = window.cursor_position() else {
		return;
	};
	let Some((camera, cam_tf)) = cameras.iter().next() else {
		return;
	};
	let world_pos = match camera.viewport_to_world_2d(cam_tf, cursor) {
		Ok(p) => p,
		Err(_) => return,
	};
	try_click(&mut play, world_pos);
}

fn try_click(play: &mut PlayState, pos: Vec2) {
	let current = play.game.current_player();
	if let Some(idx) = hit_bench(&play.game, current, pos) {
		match play.selection {
			Selection::Spawn(node) => {
				attempt_add(play, idx, node);
			}
			_ => {
				play.selection = Selection::Bench(idx);
				play.warning.clear();
				update_add_warning(play);
			}
		}
		return;
	}
	if let Some(node) = hit_node(&play.game, pos) {
		match play.selection {
			Selection::Bench(idx)
				if play.game.board().is_spawn(node) && play.game.tile_at(node).is_none() =>
			{
				attempt_add(play, idx, node);
				return;
			}
			Selection::Piece(from) => {
				if play.game.destinations(from).contains(&node) {
					apply_action(play, Action::Move { from, to: node });
					return;
				}
			}
			_ => {}
		}
		if let Some(tile) = play.game.tile_at(node) {
			if tile.owner == current {
				play.selection = Selection::Piece(node);
				play.warning.clear();
				return;
			}
		} else if play.game.board().is_spawn(node) {
			play.selection = Selection::Spawn(node);
			update_add_warning(play);
			return;
		}
	}
	play.selection = Selection::None;
	play.warning.clear();
}

fn attempt_add(play: &mut PlayState, bench_index: usize, node: NodeId) {
	apply_action(play, Action::Add { bench_index, node });
}

fn apply_action(play: &mut PlayState, action: Action) {
	match play.game.apply(action) {
		Ok(outcome) => {
			play.log = outcome.log_line;
			if outcome.removals.is_empty() {
				play.last_removals.clear();
			} else {
				play.last_removals = outcome
					.removals
					.iter()
					.map(|r| {
						format!(
							"{} {} gone at {} (m{} / x{})",
							r.tile.owner.label(),
							r.tile.kind_label(),
							r.node,
							r.matches,
							r.mismatches
						)
					})
					.collect::<Vec<_>>()
					.join(" · ");
			}
			play.selection = Selection::None;
			play.warning.clear();
		}
		Err(err) => {
			play.warning = err;
		}
	}
}

fn update_add_warning(play: &mut PlayState) {
	let player = play.game.current_player();
	match play.selection {
		Selection::Spawn(node) => {
			let killers: Vec<String> = play
				.game
				.bench(player)
				.iter()
				.filter(|t| play.game.would_overwhelm_add(**t, node))
				.map(|t| t.kind_label())
				.collect();
			if killers.is_empty() {
				play.warning = format!("Spawn {node}: pick a bench tile to ADD.");
			} else {
				play.warning = format!(
					"Warning: spawn {node} would immediately overwhelm: {}",
					killers.join(", ")
				);
			}
		}
		Selection::Bench(idx) => {
			if let Some(tile) = play.game.bench(player).get(idx).copied() {
				let bad: Vec<String> = play
					.game
					.board()
					.spawns()
					.iter()
					.copied()
					.filter(|&s| {
						play.game.tile_at(s).is_none() && play.game.would_overwhelm_add(tile, s)
					})
					.map(|s| s.to_string())
					.collect();
				if bad.is_empty() {
					play.warning = format!(
						"Selected {} — click a yellow spawn (ADD markers).",
						tile.kind_label()
					);
				} else {
					play.warning = format!(
						"Warning: {} would be immediately overwhelmed on spawn(s) {}.",
						tile.kind_label(),
						bad.join(", ")
					);
				}
			}
		}
		_ => play.warning.clear(),
	}
}

fn hit_node(game: &Game, pos: Vec2) -> Option<NodeId> {
	let mut best = None;
	let mut best_d = (TILE_R + 8.0) * (TILE_R + 8.0);
	for id in 0..game.board().node_count() as NodeId {
		let p = world(game.board().position(id));
		let d = pos.distance_squared(p);
		let rad = if game.tile_at(id).is_some() { TILE_R + 4.0 } else { NODE_R + 10.0 };
		if d <= rad * rad && d < best_d {
			best_d = d;
			best = Some(id);
		}
	}
	best
}

fn hit_bench(game: &Game, player: Player, pos: Vec2) -> Option<usize> {
	let bench = game.bench(player);
	for (i, _) in bench.iter().enumerate() {
		let p = bench_pos(player, i, bench.len());
		if pos.distance_squared(p) <= (TILE_R + 6.0) * (TILE_R + 6.0) {
			return Some(i);
		}
	}
	None
}

fn bench_pos(player: Player, index: usize, len: usize) -> Vec2 {
	let y = match player {
		Player::Two => 390.0,
		Player::One => -390.0,
	};
	let spacing = 56.0;
	let total = spacing * len.saturating_sub(1) as f32;
	let start = -total / 2.0;
	Vec2::new(start + index as f32 * spacing, y)
}

fn bot_turn(
	time: Res<Time>,
	mut clock: ResMut<BotClock>,
	cfg: Res<GameConfig>,
	mut play: ResMut<PlayState>,
	mut rng: ResMut<RngRes>,
) {
	if play.game.status().is_over() {
		return;
	}
	let actor = cfg.actor(play.game.current_player());
	if actor.is_human() {
		clock.wait = cfg.delay.as_secs_f32();
		return;
	}
	clock.wait -= time.delta().as_secs_f32();
	if clock.wait > 0.0 {
		return;
	}
	clock.wait = cfg.delay.as_secs_f32();
	let action = ai::choose(&play.game, actor, &mut rng.0, cfg.think);
	apply_action(&mut play, action);
}

fn refresh_view(
	mut commands: Commands,
	mut play: ResMut<PlayState>,
	existing: Query<Entity, With<DynamicView>>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<ColorMaterial>>,
) {
	let fp = view_fingerprint(&play);
	if fp == play.fingerprint && existing.iter().next().is_some() {
		return;
	}
	play.fingerprint = fp;
	for e in existing.iter() {
		commands.entity(e).despawn();
	}
	let board = play.game.board().clone();
	for id in 0..board.node_count() as NodeId {
		if let Some(tile) = play.game.tile_at(id) {
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				world(board.position(id)),
				tile,
				matches!(play.selection, Selection::Piece(n) if n == id),
			);
		} else if board.is_spawn(id) {
			let mesh = meshes.add(Circle::new(NODE_R + 2.0));
			let mat = materials.add(ColorMaterial::from(Color::srgb(0.93, 0.84, 0.43)));
			commands.spawn((
				DynamicView,
				Mesh2d(mesh),
				MeshMaterial2d(mat),
				Transform::from_translation(world(board.position(id)).extend(1.0)),
			));
			if matches!(play.selection, Selection::Bench(_)) {
				spawn_label(&mut commands, world(board.position(id)) + Vec2::Y * 18.0, "ADD");
			}
		}
	}
	for player in [Player::Two, Player::One] {
		let bench = play.game.bench(player).to_vec();
		let selected = play.game.current_player() == player;
		for (i, tile) in bench.iter().enumerate() {
			let highlight = selected && play.selection == Selection::Bench(i);
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				bench_pos(player, i, bench.len()),
				*tile,
				highlight,
			);
		}
	}
}

fn spawn_tile_at(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	tile: Tile,
	highlight: bool,
) {
	let fill = match tile.owner {
		Player::One => Color::srgb(0.91, 0.89, 0.82),
		Player::Two => Color::srgb(0.12, 0.12, 0.14),
	};
	let ring = tile_color(tile.color);
	let z = 2.0;
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(TILE_R))),
		MeshMaterial2d(materials.add(ColorMaterial::from(fill))),
		Transform::from_translation(pos.extend(z)),
	));
	for k in 0..tile.weight {
		let r = TILE_R - 3.0 - f32::from(k) * 3.5;
		if r > 6.0 {
			commands.spawn((
				DynamicView,
				Mesh2d(meshes.add(Annulus::new(r - 1.6, r))),
				MeshMaterial2d(materials.add(ColorMaterial::from(ring))),
				Transform::from_translation(pos.extend(z + 0.1 + f32::from(k) * 0.05)),
			));
		}
	}
	let dots = tile.dots.min(3);
	let spread = 7.0;
	let start = -spread * (f32::from(dots) - 1.0) / 2.0;
	for i in 0..dots {
		let dx = start + f32::from(i) * spread;
		commands.spawn((
			DynamicView,
			Mesh2d(meshes.add(Circle::new(2.8))),
			MeshMaterial2d(materials.add(ColorMaterial::from(ring))),
			Transform::from_translation((pos + Vec2::new(dx, 0.0)).extend(z + 0.4)),
		));
	}
	if highlight {
		commands.spawn((
			DynamicView,
			Mesh2d(meshes.add(Annulus::new(TILE_R + 2.0, TILE_R + 5.0))),
			MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.95, 0.85, 0.25)))),
			Transform::from_translation(pos.extend(z - 0.1)),
		));
	}
}

fn spawn_label(commands: &mut Commands, pos: Vec2, text: &str) {
	commands.spawn((
		DynamicView,
		Text2d::new(text),
		TextFont { font_size: FontSize::Px(12.0), ..default() },
		TextColor(Color::srgb(0.95, 0.86, 0.30)),
		Transform::from_translation(pos.extend(4.0)),
	));
}

fn tile_color(color: TileColor) -> Color {
	match color {
		TileColor::Green => Color::srgb(0.18, 0.66, 0.32),
		TileColor::Blue => Color::srgb(0.24, 0.48, 0.90),
		TileColor::Orange => Color::srgb(0.91, 0.52, 0.16),
	}
}

fn update_hud(
	play: Res<PlayState>,
	cfg: Res<GameConfig>,
	mut status: Query<&mut Text, (With<StatusHud>, Without<LogHud>, Without<WarnHud>)>,
	mut log: Query<&mut Text, (With<LogHud>, Without<StatusHud>, Without<WarnHud>)>,
	mut warn: Query<&mut Text, (With<WarnHud>, Without<StatusHud>, Without<LogHud>)>,
) {
	let g = &play.game;
	let a1 = cfg.bot1.name();
	let a2 = cfg.bot2.name();
	let turn = match g.status() {
		GameStatus::InProgress => format!(
			"Turn {}  ·  {} ({}) to play  ·  P1 light {}/{}  ·  P2 dark {}/{}",
			g.turn_count() + 1,
			g.current_player().label(),
			cfg.actor(g.current_player()).name(),
			g.bench(Player::One).len(),
			g.bag_len(Player::One),
			g.bench(Player::Two).len(),
			g.bag_len(Player::Two)
		),
		GameStatus::Won { winner, reason } => {
			format!("Game over: {} wins by {:?}", winner.label(), reason)
		}
		GameStatus::Draw { reason } => format!("Draw ({reason:?})"),
	};
	let header = format!("Apa Kan   P1 {a1} vs P2 {a2}   {turn}");
	if let Some(mut t) = status.iter_mut().next() {
		*t = Text::new(header);
	}
	if let Some(mut t) = log.iter_mut().next() {
		let extra = if play.last_removals.is_empty() {
			String::new()
		} else {
			format!("  |  {}", play.last_removals)
		};
		*t = Text::new(format!("Last: {}{}", play.log, extra));
	}
	if let Some(mut t) = warn.iter_mut().next() {
		*t = Text::new(play.warning.clone());
	}
}

fn view_fingerprint(play: &PlayState) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut h = DefaultHasher::new();
	play.game.turn_count().hash(&mut h);
	play.game.current_player().hash(&mut h);
	play.selection.hash(&mut h);
	for (n, t) in play.game.tiles() {
		n.hash(&mut h);
		t.id.hash(&mut h);
	}
	for p in [Player::One, Player::Two] {
		for t in play.game.bench(p) {
			t.id.hash(&mut h);
		}
	}
	h.finish()
}
