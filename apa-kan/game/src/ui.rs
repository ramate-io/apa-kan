//! Bevy 0.19 desktop UI: both benches, ADD/MOVE targeting, last-action log.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Capturing, Screenshot};
use bevy::window::WindowResolution;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::ai;
use crate::board::NodeId;
use crate::config::GameConfig;
use crate::game::{Action, Game, Player, Tile};
use crate::visual::{self, ArcBody, ViewLayout};

pub fn run_app(cfg: GameConfig) {
	let screenshot = cfg.screenshot.clone();
	let game = if screenshot.is_some() {
		visual::demo_midgame()
	} else {
		match Game::start(cfg.rules, cfg.seed, cfg.scenario.as_ref()) {
			Ok(g) => g,
			Err(err) => {
				eprintln!("{err}");
				std::process::exit(1);
			}
		}
	};
	let rng = StdRng::seed_from_u64(cfg.seed ^ 0xA5A5_A5A5);
	let width = cfg.width;
	let height = cfg.height;
	let mut resolution = WindowResolution::new(width, height);
	resolution.set_scale_factor_override(Some(1.0));
	App::new()
		.add_plugins(DefaultPlugins.set(WindowPlugin {
			primary_window: Some(Window { title: "Apa Kan".into(), resolution, ..default() }),
			..default()
		}))
		.insert_resource(ClearColor(Color::srgb(
			visual::BG_RGB[0],
			visual::BG_RGB[1],
			visual::BG_RGB[2],
		)))
		.insert_resource(ConfigRes(cfg))
		.insert_resource(PlayState {
			game,
			selection: Selection::None,
			log: if screenshot.is_some() {
				"P1 ADD orange w2 d2 at center".into()
			} else {
				String::new()
			},
			warning: String::new(),
			last_removals: String::new(),
			fingerprint: 0,
		})
		.insert_resource(RngRes(rng))
		.insert_resource(BotClock { wait: 0.4 })
		.insert_resource(UiLayout(ViewLayout::from_window(width as f32, height as f32)))
		.insert_resource(ShotState {
			frames: 0,
			requested_at: 0,
			requested: false,
			finished: false,
		})
		.add_systems(Startup, setup)
		.add_systems(
			Update,
			(sync_layout, handle_click, bot_turn, refresh_view, update_hud, screenshot_capture),
		)
		.run();
}

#[derive(Resource)]
struct ConfigRes(GameConfig);

impl std::ops::Deref for ConfigRes {
	type Target = GameConfig;
	fn deref(&self) -> &GameConfig {
		&self.0
	}
}

#[derive(Resource, Clone)]
struct UiLayout(ViewLayout);

impl std::ops::Deref for UiLayout {
	type Target = ViewLayout;
	fn deref(&self) -> &ViewLayout {
		&self.0
	}
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

#[derive(Resource)]
struct ShotState {
	frames: u32,
	requested_at: u32,
	requested: bool,
	finished: bool,
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
struct TurnHud;

#[derive(Component)]
struct LogHud;

#[derive(Component)]
struct WarnHud;

#[derive(Component)]
struct HelpHud;

fn setup(mut commands: Commands) {
	commands.spawn(Camera2d::default());
	spawn_hud_line(&mut commands, StatusHud, 16.0, 8.0, 17.0, Color::srgb(0.92, 0.93, 0.90));
	spawn_hud_line(&mut commands, TurnHud, 16.0, 30.0, 15.0, Color::srgb(0.88, 0.90, 0.86));
	spawn_hud_line(&mut commands, LogHud, 16.0, 52.0, 15.0, Color::srgb(0.78, 0.82, 0.72));
	spawn_hud_line(&mut commands, WarnHud, 16.0, 74.0, 15.0, Color::srgb(0.95, 0.72, 0.28));
	commands.spawn((
		HelpHud,
		Text::new(visual::HELP_TEXT),
		TextFont { font_size: FontSize::Px(15.0), ..default() },
		TextColor(Color::srgb(0.70, 0.74, 0.68)),
		TextLayout::no_wrap(),
		Node {
			position_type: PositionType::Absolute,
			bottom: Val::Px(10.0),
			left: Val::Px(16.0),
			right: Val::Px(16.0),
			height: Val::Px(22.0),
			..default()
		},
	));
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
		TextLayout::no_wrap(),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(top),
			left: Val::Px(left),
			right: Val::Px(16.0),
			height: Val::Px(size + 4.0),
			..default()
		},
	));
}

fn sync_layout(windows: Query<&Window>, mut layout: ResMut<UiLayout>, mut play: ResMut<PlayState>) {
	let Some(window) = windows.iter().next() else {
		return;
	};
	let next = ViewLayout::from_window(window.width(), window.height());
	if (next.width - layout.width).abs() > 0.5 || (next.height - layout.height).abs() > 0.5 {
		layout.0 = next;
		play.fingerprint = 0;
	}
}

fn world_v(layout: &ViewLayout, p: [f64; 2]) -> Vec2 {
	let w = layout.board_to_world(p);
	Vec2::new(w[0], w[1])
}

fn handle_click(
	buttons: Res<ButtonInput<MouseButton>>,
	windows: Query<&Window>,
	cameras: Query<(&Camera, &GlobalTransform)>,
	mut play: ResMut<PlayState>,
	cfg: Res<ConfigRes>,
	keys: Res<ButtonInput<KeyCode>>,
	layout: Res<UiLayout>,
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
	try_click(&mut play, &layout.0, world_pos);
}

fn try_click(play: &mut PlayState, layout: &ViewLayout, pos: Vec2) {
	let current = play.game.current_player();
	if let Some(idx) = hit_bench(&play.game, layout, current, pos) {
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
	if let Some(node) = hit_node(&play.game, layout, pos) {
		match play.selection {
			Selection::Bench(idx)
				if play.game.board().is_spawn(node) && play.game.tile_at(node).is_none() =>
			{
				attempt_add(play, idx, node);
				return;
			}
			Selection::Piece(from) => {
				if play.game.legal_destinations(from).contains(&node) {
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
					.join(visual::HUD_SEP);
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
						"Selected {} - click a yellow spawn (ADD markers).",
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

fn hit_node(game: &Game, layout: &ViewLayout, pos: Vec2) -> Option<NodeId> {
	let mut best = None;
	let mut best_d = (layout.tile_radius + 8.0) * (layout.tile_radius + 8.0);
	for id in 0..game.board().node_count() as NodeId {
		let p = world_v(layout, game.board().position(id));
		let d = pos.distance_squared(p);
		let rad = if game.tile_at(id).is_some() {
			layout.tile_radius + 4.0
		} else {
			layout.node_radius + 10.0
		};
		if d <= rad * rad && d < best_d {
			best_d = d;
			best = Some(id);
		}
	}
	best
}

fn hit_bench(game: &Game, layout: &ViewLayout, player: Player, pos: Vec2) -> Option<usize> {
	let bench = game.bench(player);
	for (i, _) in bench.iter().enumerate() {
		let p = layout.bench_pos(player, i, bench.len());
		let p = Vec2::new(p[0], p[1]);
		if pos.distance_squared(p)
			<= (layout.bench_tile_radius + 6.0) * (layout.bench_tile_radius + 6.0)
		{
			return Some(i);
		}
	}
	None
}

fn bot_turn(
	time: Res<Time>,
	mut clock: ResMut<BotClock>,
	cfg: Res<ConfigRes>,
	mut play: ResMut<PlayState>,
	mut rng: ResMut<RngRes>,
) {
	if play.game.status().is_over() {
		return;
	}
	if cfg.screenshot.is_some() {
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
	layout: Res<UiLayout>,
	existing: Query<Entity, With<DynamicView>>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<ColorMaterial>>,
) {
	let fp = view_fingerprint(&play, &layout.0);
	if fp == play.fingerprint && existing.iter().next().is_some() {
		return;
	}
	play.fingerprint = fp;
	for e in existing.iter() {
		commands.entity(e).despawn();
	}
	let layout = &layout.0;
	let board = play.game.board().clone();
	let dests: Vec<NodeId> = match play.selection {
		Selection::Piece(n) => play.game.legal_destinations(n),
		_ => Vec::new(),
	};

	for arc in board.arcs() {
		spawn_arc(
			&mut commands,
			&mut meshes,
			&mut materials,
			layout,
			arc,
			play.game.tile_at(arc.from),
			play.game.tile_at(arc.to),
			board.center(),
		);
	}

	for id in 0..board.node_count() as NodeId {
		let p = world_v(layout, board.position(id));
		if play.game.tile_at(id).is_none() {
			spawn_empty_node(
				&mut commands,
				&mut meshes,
				&mut materials,
				layout,
				p,
				board.is_spawn(id),
				id == board.center(),
			);
		}
		if dests.contains(&id) {
			spawn_ring(
				&mut commands,
				&mut meshes,
				&mut materials,
				p,
				layout.tile_radius + 4.0,
				layout.tile_radius + 7.0,
				rgb(visual::DEST_RGB),
				visual::Z_HIGHLIGHT,
			);
		}
		if let Selection::Spawn(s) = play.selection {
			if s == id {
				spawn_ring(
					&mut commands,
					&mut meshes,
					&mut materials,
					p,
					layout.tile_radius + 6.0,
					layout.tile_radius + 9.0,
					rgb(visual::HIGHLIGHT_RGB),
					visual::Z_HIGHLIGHT,
				);
			}
		}
	}

	for id in 0..board.node_count() as NodeId {
		if let Some(tile) = play.game.tile_at(id) {
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				world_v(layout, board.position(id)),
				tile,
				matches!(play.selection, Selection::Piece(n) if n == id),
				layout.tile_radius,
			);
		} else if board.is_spawn(id) && matches!(play.selection, Selection::Bench(_)) {
			spawn_label(
				&mut commands,
				world_v(layout, board.position(id)) + Vec2::Y * (layout.tile_radius * 0.55 + 12.0),
				"ADD",
			);
		}
	}

	for player in [Player::Two, Player::One] {
		let bench = play.game.bench(player).to_vec();
		let selected = play.game.current_player() == player;
		for (i, tile) in bench.iter().enumerate() {
			let highlight = selected && play.selection == Selection::Bench(i);
			let p = layout.bench_pos(player, i, bench.len());
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				Vec2::new(p[0], p[1]),
				*tile,
				highlight,
				layout.bench_tile_radius,
			);
		}
	}
}

fn spawn_empty_node(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	layout: &ViewLayout,
	pos: Vec2,
	spawn: bool,
	center: bool,
) {
	let (r, color) = if spawn {
		(layout.spawn_radius, rgb(visual::SPAWN_RGB))
	} else if center {
		(layout.node_radius + 1.5, rgb(visual::CENTER_RGB))
	} else {
		(layout.node_radius, rgb(visual::NODE_RGB))
	};
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(r))),
		MeshMaterial2d(materials.add(ColorMaterial::from(color))),
		Transform::from_translation(pos.extend(visual::Z_NODE)),
	));
}

fn spawn_arc(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	layout: &ViewLayout,
	arc: &crate::board::ArcEdge,
	from: Option<Tile>,
	to: Option<Tile>,
	center: NodeId,
) {
	let samples = arc.sample(40);
	let pts: Vec<[f64; 2]> = samples
		.iter()
		.map(|p| {
			let w = layout.board_to_world(*p);
			[f64::from(w[0]), f64::from(w[1])]
		})
		.collect();
	let width = layout.arc_width;
	let paint = visual::arc_paint(from, to);
	match paint.body {
		ArcBody::Neutral => {
			spawn_stroke(
				commands,
				meshes,
				materials,
				&pts,
				width,
				rgb(visual::NEUTRAL_ARC_RGB),
				visual::Z_ARC,
			);
		}
		ArcBody::Solid { color, thick } => {
			let w = if thick { width * visual::ARC_SAME_COLOR_WIDTH_SCALE } else { width };
			spawn_stroke(
				commands,
				meshes,
				materials,
				&pts,
				w,
				rgb(visual::tile_rgb(color)),
				visual::Z_ARC,
			);
		}
		ArcBody::Braid { a, b } => {
			let amp = if visual::arc_touches_center(arc, center) {
				f64::from(layout.braid_amp_center)
			} else {
				f64::from(layout.braid_amp)
			};
			let turns = visual::braid_turns(arc);
			let braid = visual::braid_strands(&pts, amp, turns);
			let strand_w = width * visual::BRAID_STRAND_WIDTH_SCALE;
			spawn_braid_strand(
				commands,
				meshes,
				materials,
				&braid.a,
				&braid.phase,
				true,
				strand_w,
				rgb(visual::tile_rgb(a)),
			);
			spawn_braid_strand(
				commands,
				meshes,
				materials,
				&braid.b,
				&braid.phase,
				false,
				strand_w,
				rgb(visual::tile_rgb(b)),
			);
		}
	}
	let zag_amp = if visual::arc_touches_center(arc, center) {
		f64::from(layout.zag_amp_center)
	} else {
		f64::from(layout.zag_amp)
	};
	let zag_w = width * visual::ZAG_STRAND_WIDTH_SCALE;
	for (i, zag) in paint.zags.iter().enumerate() {
		let Some(color) = *zag else {
			continue;
		};
		let teeth = visual::ZAG_TEETH + f64::from(i as u32);
		let zig =
			visual::zigzag_polyline(&pts, zag_amp * (1.0 + 0.35 * f64::from(i as u32)), teeth);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&zig,
			zag_w,
			rgb(visual::tile_rgb(color)),
			visual::Z_ARC_OVER + 0.02 * i as f32,
		);
	}
}

fn spawn_braid_strand(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	phase: &[f64],
	strand_a: bool,
	width: f32,
	color: Color,
) {
	if pts.len() < 2 {
		return;
	}
	let mut start = 0usize;
	for i in 1..pts.len() {
		let front_a = phase[i].cos() >= 0.0;
		let this_front = if strand_a { front_a } else { !front_a };
		let prev_front_a = phase[i - 1].cos() >= 0.0;
		let prev_front = if strand_a { prev_front_a } else { !prev_front_a };
		if this_front != prev_front || i + 1 == pts.len() {
			let end = if i + 1 == pts.len() { i } else { i };
			if end > start {
				let slice = &pts[start..=end];
				let z = if prev_front { visual::Z_ARC_OVER } else { visual::Z_ARC };
				spawn_stroke(commands, meshes, materials, slice, width, color, z);
			}
			start = i.saturating_sub(1);
		}
	}
}

fn spawn_stroke(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	width: f32,
	color: Color,
	z: f32,
) {
	if pts.len() < 2 {
		return;
	}
	let mesh = polyline_mesh(pts, width);
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(mesh)),
		MeshMaterial2d(materials.add(ColorMaterial::from(color))),
		Transform::from_xyz(0.0, 0.0, z),
	));
}

fn polyline_mesh(pts: &[[f64; 2]], width: f32) -> Mesh {
	let n = pts.len();
	let hw = f64::from(width) / 2.0;
	let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
	let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
	let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * 2);
	let mut indices: Vec<u32> = Vec::with_capacity((n.saturating_sub(1)) * 6);
	for i in 0..n {
		let prev = if i == 0 { pts[0] } else { pts[i - 1] };
		let next = if i + 1 == n { pts[i] } else { pts[i + 1] };
		let dx = next[0] - prev[0];
		let dy = next[1] - prev[1];
		let len = (dx * dx + dy * dy).sqrt().max(1e-9);
		let nx = -dy / len;
		let ny = dx / len;
		let p = pts[i];
		positions.push([(p[0] + nx * hw) as f32, (p[1] + ny * hw) as f32, 0.0]);
		positions.push([(p[0] - nx * hw) as f32, (p[1] - ny * hw) as f32, 0.0]);
		normals.push([0.0, 0.0, 1.0]);
		normals.push([0.0, 0.0, 1.0]);
		let v = i as f32 / n.max(1) as f32;
		uvs.push([0.0, v]);
		uvs.push([1.0, v]);
		if i + 1 < n {
			let i0 = (i * 2) as u32;
			indices.extend_from_slice(&[i0, i0 + 2, i0 + 1, i0 + 1, i0 + 2, i0 + 3]);
		}
	}
	Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
		.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
		.with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
		.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
		.with_inserted_indices(Indices::U32(indices))
}

fn spawn_ring(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	inner: f32,
	outer: f32,
	color: Color,
	z: f32,
) {
	if outer <= inner {
		return;
	}
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Annulus::new(inner, outer))),
		MeshMaterial2d(materials.add(ColorMaterial::from(color))),
		Transform::from_translation(pos.extend(z)),
	));
}

fn spawn_tile_at(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	tile: Tile,
	highlight: bool,
	radius: f32,
) {
	let fill = rgb(visual::owner_fill_rgb(tile.owner));
	let ring_colors = tile.ring_colors();
	let primary = rgb(visual::tile_rgb(tile.primary()));
	let z = visual::Z_TILE;
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(radius))),
		MeshMaterial2d(materials.add(ColorMaterial::from(fill))),
		Transform::from_translation(pos.extend(z)),
	));
	let scale = radius / visual::PREV_TILE_RADIUS_PX;
	let ring_gap = 3.5 * scale;
	let ring_half = 1.6 * scale;
	let strokes = tile.weight.max(tile.n_rings);
	for k in 0..strokes {
		let r = radius - 3.0 * scale - f32::from(k) * ring_gap;
		if r > 6.0 * scale {
			let idx = (k as usize).min(ring_colors.len().saturating_sub(1));
			let color = ring_colors.get(idx).copied().unwrap_or(tile.primary());
			let z = visual::Z_TILE_RING + f32::from(k) * 0.02;
			if tile.ring_is_zag(idx) {
				let zig = visual::zigzag_circle(
					[f64::from(pos.x), f64::from(pos.y)],
					f64::from(r - ring_half * 0.5),
					f64::from((2.4 * scale).max(1.6)),
					12,
					48,
				);
				spawn_stroke(
					commands,
					meshes,
					materials,
					&zig,
					(2.4 * scale).max(1.8),
					rgb(visual::tile_rgb(color)),
					z,
				);
			} else {
				spawn_ring(
					commands,
					meshes,
					materials,
					pos,
					(r - ring_half).max(1.0),
					r,
					rgb(visual::tile_rgb(color)),
					z,
				);
			}
		}
	}
	let dots = tile.dots.min(3);
	let spread = 7.0 * scale;
	let start = -spread * (f32::from(dots) - 1.0) / 2.0;
	let dot_r = (2.8 * scale).max(2.0);
	for i in 0..dots {
		let dx = start + f32::from(i) * spread;
		commands.spawn((
			DynamicView,
			Mesh2d(meshes.add(Circle::new(dot_r))),
			MeshMaterial2d(materials.add(ColorMaterial::from(primary))),
			Transform::from_translation((pos + Vec2::new(dx, 0.0)).extend(visual::Z_TILE_DOT)),
		));
	}
	if highlight {
		spawn_ring(
			commands,
			meshes,
			materials,
			pos,
			radius + 2.0 * scale,
			radius + 5.0 * scale,
			rgb(visual::HIGHLIGHT_RGB),
			visual::Z_HIGHLIGHT,
		);
	}
}

fn spawn_label(commands: &mut Commands, pos: Vec2, text: &str) {
	commands.spawn((
		DynamicView,
		Text2d::new(text),
		TextFont { font_size: FontSize::Px(12.0), ..default() },
		TextColor(Color::srgb(0.95, 0.86, 0.30)),
		Transform::from_translation(pos.extend(visual::Z_MARKER)),
	));
}

fn rgb(c: [f32; 3]) -> Color {
	Color::srgb(c[0], c[1], c[2])
}

fn update_hud(
	play: Res<PlayState>,
	cfg: Res<ConfigRes>,
	mut status: Query<
		&mut Text,
		(With<StatusHud>, Without<TurnHud>, Without<LogHud>, Without<WarnHud>, Without<HelpHud>),
	>,
	mut turn: Query<
		&mut Text,
		(With<TurnHud>, Without<StatusHud>, Without<LogHud>, Without<WarnHud>, Without<HelpHud>),
	>,
	mut log: Query<
		&mut Text,
		(With<LogHud>, Without<StatusHud>, Without<TurnHud>, Without<WarnHud>, Without<HelpHud>),
	>,
	mut warn: Query<
		&mut Text,
		(With<WarnHud>, Without<StatusHud>, Without<TurnHud>, Without<LogHud>, Without<HelpHud>),
	>,
) {
	let g = &play.game;
	let a1 = cfg.bot1.name();
	let a2 = cfg.bot2.name();
	if let Some(mut t) = status.iter_mut().next() {
		*t = Text::new(visual::format_title(a1, a2));
	}
	if let Some(mut t) = turn.iter_mut().next() {
		*t = Text::new(visual::format_turn_line(g, cfg.actor(g.current_player()).name()));
	}
	if let Some(mut t) = log.iter_mut().next() {
		*t = Text::new(visual::format_log_line(&play.log, &play.last_removals));
	}
	if let Some(mut t) = warn.iter_mut().next() {
		*t = Text::new(play.warning.clone());
	}
}

fn screenshot_capture(
	mut commands: Commands,
	cfg: Res<ConfigRes>,
	mut shot: ResMut<ShotState>,
	views: Query<Entity, With<DynamicView>>,
	capturing: Query<Entity, With<Capturing>>,
	mut exit: MessageWriter<AppExit>,
	time: Res<Time>,
) {
	let Some(path) = cfg.screenshot.clone() else {
		return;
	};
	shot.frames = shot.frames.saturating_add(1);
	if time.elapsed_secs() > 20.0 && !shot.finished {
		eprintln!("screenshot timed out before capture finished");
		exit.write(AppExit::error());
		shot.finished = true;
		return;
	}
	if shot.requested {
		if shot.finished {
			return;
		}
		if capturing.is_empty() && shot.frames > shot.requested_at.saturating_add(12) {
			shot.finished = true;
			exit.write(AppExit::Success);
		}
		return;
	}
	if time.elapsed_secs() < 4.0 || shot.frames < 30 || views.iter().next().is_none() {
		return;
	}
	commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
	shot.requested = true;
	shot.requested_at = shot.frames;
}

fn view_fingerprint(play: &PlayState, layout: &ViewLayout) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut h = DefaultHasher::new();
	play.game.turn_count().hash(&mut h);
	play.game.current_player().hash(&mut h);
	play.selection.hash(&mut h);
	((layout.width * 10.0) as i32).hash(&mut h);
	((layout.height * 10.0) as i32).hash(&mut h);
	((layout.scale * 100.0) as i32).hash(&mut h);
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
