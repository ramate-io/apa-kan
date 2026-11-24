//! Bevy 0.19 UI: home / H2H shell plus in-match board.

use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Capturing, Screenshot};
use bevy::window::WindowResolution;
use bevy::winit::{UpdateMode, WinitSettings};
use rand::rngs::StdRng;
use rand::SeedableRng;

mod smoke;

use crate::ai::{self, SearchLimit, SearchStats};
use crate::bench_log::{self, GameLog};
use crate::board::NodeId;
use crate::config::GameConfig;
use crate::deck::{
	self, pick_from_pool, pick_plan, respects_type_cap, would_respect_cap, SetupConfig, SetupRecord,
};
use crate::game::{Action, BudgetToken, CaptureDest, CaptureMark, DeckSlot, Game, Player, Tile};
use crate::platform::{replace_browser_route, IncomingUrl, PlatformPlugin};
use crate::play_log;
use crate::remote;
use crate::route::{DeepLink, Route};
use crate::shell::{self, AppScreen, LeaveMatch, StartMatch};
use crate::visual::{self, ArcBody, BraidLane, ViewLayout};

/// SIL OFL Expletus Sans, same face as the wasm host page.
const EXPLETUS_SANS: &[u8] = include_bytes!("../assets/fonts/ExpletusSans.ttf");

#[derive(Resource, Clone)]
pub(crate) struct UiFont(pub Handle<Font>);

pub(crate) fn ui_text_font(font: &UiFont, size: f32) -> TextFont {
	TextFont::from_font_size(FontSize::Px(size))
		.with_font(font.0.clone())
		.with_font_weight(FontWeight::MEDIUM)
}

pub(crate) fn ui_title_font(font: &UiFont, size: f32) -> TextFont {
	TextFont::from_font_size(FontSize::Px(size))
		.with_font(font.0.clone())
		.with_font_weight(FontWeight::SEMIBOLD)
}

/// Insert [`UiFont`] while building the app. It must exist before the first
/// `OnEnter(AppScreen::…)`, which Bevy runs ahead of `PreStartup`; loading it in
/// a `Startup` system made the shell panic on a missing resource.
fn insert_ui_font(app: &mut App) {
	let handle = app
		.world_mut()
		.resource_mut::<Assets<Font>>()
		.add(Font::from_bytes(EXPLETUS_SANS.to_vec()));
	app.insert_resource(UiFont(handle));
}

pub fn run_app(cfg: GameConfig) {
	let width = cfg.width;
	let height = cfg.height;
	let play = play_state_from_config(&cfg);
	let deck_slots = play.game.rules().bag_size as usize;
	let rng = StdRng::seed_from_u64(cfg.seed ^ 0xA5A5_A5A5);
	let host = shell::HostRes(crate::host::HostConfig::detect(cfg.host.clone()));
	let screen = if cfg.show_shell && cfg.screenshot.is_none() {
		AppScreen::Shell
	} else {
		AppScreen::Match
	};
	let mut resolution = WindowResolution::new(width, height);
	resolution.set_scale_factor_override(Some(1.0));
	let mut app = App::new();
	app.add_plugins(
		DefaultPlugins.set(WindowPlugin {
			primary_window: Some(primary_window(&cfg, resolution)),
			..default()
		}),
	)
	.insert_resource(ClearColor(Color::srgb(
		visual::BG_RGB[0],
		visual::BG_RGB[1],
		visual::BG_RGB[2],
	)))
	// Keep ticking when the window/canvas is unfocused. Bevy's default drops to
	// reactive low-power then; on the web the canvas usually is not focused
	// (pointer events are prevent-defaulted), and the low-power wakeup could be
	// lost, freezing bot turns, the clock and hosted-game polling until the
	// next input event.
	.insert_resource(WinitSettings {
		focused_mode: UpdateMode::Continuous,
		unfocused_mode: UpdateMode::Continuous,
	})
	.insert_resource(ConfigRes(cfg))
	.insert_resource(play)
	.insert_resource(RngRes(rng))
	.insert_resource(BotClock { wait: 0.4 })
	.insert_resource(UiLayout(ViewLayout::from_window_with_deck(
		width as f32,
		height as f32,
		deck_slots,
	)))
	.insert_resource(ShotState { frames: 0, requested_at: 0, requested: false, finished: false })
	.insert_state(screen)
	.insert_resource(host)
	.add_plugins(PlatformPlugin)
	.add_plugins(shell::plugin)
	.add_plugins(smoke::plugin)
	.add_systems(Startup, setup_camera)
	.add_systems(OnEnter(AppScreen::Match), spawn_match_hud)
	.add_systems(OnExit(AppScreen::Match), despawn_match_hud)
	.add_systems(
		Update,
		(
			sync_layout,
			handle_click,
			bot_turn,
			tick_clock,
			refresh_view,
			update_hud,
			screenshot_capture,
			record_division_result,
		)
			.run_if(in_state(AppScreen::Match)),
	)
	.add_systems(
		Update,
		(
			handle_start_match,
			handle_leave_match,
			(handle_game_deep_link, attach_remote_session).chain(),
			clear_division_on_game_link,
			remote_sync,
		),
	);
	insert_ui_font(&mut app);
	app.run();
}

fn primary_window(cfg: &GameConfig, resolution: WindowResolution) -> Window {
	Window {
		title: "Apa Kan".into(),
		resolution,
		canvas: Some("#bevy-canvas".into()),
		fit_canvas_to_parent: true,
		prevent_default_event_handling: true,
		visible: cfg.screenshot.is_none() || !cfg!(target_arch = "wasm32"),
		..default()
	}
}

fn play_state_from_config(cfg: &GameConfig) -> PlayState {
	let screenshot = cfg.screenshot.clone();
	let from_start = cfg.screenshot_from_start;
	let interactive_setup = cfg.bot1.is_human() || cfg.bot2.is_human();
	let (mut game, setup_session) = if screenshot.is_some() && !from_start {
		(visual::demo_midgame(), None)
	} else if screenshot.is_some() || !interactive_setup {
		match Game::start_with(cfg.rules, cfg.seed, cfg.scenario.as_ref(), cfg.setup) {
			Ok(g) => (g, None),
			Err(err) => fatal(&err),
		}
	} else {
		match SetupSession::begin(cfg) {
			Ok(session) => (Game::empty(cfg.rules), Some(session)),
			Err(err) => fatal(&err),
		}
	};
	if let Some(tc) = cfg.time_control {
		if setup_session.is_none() {
			game.set_time_control(tc);
		}
	}
	let mut play = PlayState {
		game,
		setup: setup_session,
		selection: Selection::None,
		log: if screenshot.is_some() && !from_start {
			"P1 ADD orange w2 d2 at center".into()
		} else {
			String::new()
		},
		warning: String::new(),
		last_removals: String::new(),
		flash: visual::CaptureFlash::empty(),
		fingerprint: 0,
		search1: ai::Searcher::with_config(cfg.search),
		search2: ai::Searcher::with_config(cfg.search),
		bot_thinking: false,
		from_shell: cfg.show_shell,
		log_meta: play_log_meta(cfg, screenshot.is_some()),
		game_log: None,
		remote: None,
		outbox: None,
	};
	if play.setup.is_none() {
		begin_play_log(&mut play);
	}
	play
}

fn fatal(err: &str) -> ! {
	#[cfg(target_arch = "wasm32")]
	{
		panic!("{err}");
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		eprintln!("{err}");
		std::process::exit(1);
	}
}

fn search_slice() -> Duration {
	if cfg!(target_arch = "wasm32") {
		Duration::from_millis(8)
	} else {
		Duration::from_secs(60 * 60)
	}
}

#[derive(Resource)]
pub(crate) struct ConfigRes(pub GameConfig);

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
	setup: Option<SetupSession>,
	selection: Selection,
	log: String,
	warning: String,
	last_removals: String,
	flash: visual::CaptureFlash,
	fingerprint: u64,
	search1: ai::Searcher,
	search2: ai::Searcher,
	bot_thinking: bool,
	from_shell: bool,
	log_meta: Option<PlayLogMeta>,
	game_log: Option<GameLog>,
	/// Server-hosted `/game/<id>` this match mirrors, if any.
	remote: Option<remote::RemoteSession>,
	/// Transaction for the move just played locally, waiting to be sent.
	outbox: Option<apa_kan_tx::Transaction>,
}

struct PlayLogMeta {
	path: std::path::PathBuf,
	think_ms: u64,
	pairing: String,
	p1: String,
	p2: String,
	search: String,
	scenario: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SetupStage {
	Opening(Player),
	Reveal,
	Extend(Player),
	Done,
}

#[derive(Clone)]
struct SetupSession {
	rules: crate::game::RuleSet,
	seed: u64,
	cfg: SetupConfig,
	decks: [Vec<Tile>; 2],
	pick7: [Vec<usize>; 2],
	extend: [Vec<usize>; 2],
	selected: Vec<usize>,
	stage: SetupStage,
}

impl SetupSession {
	fn begin(cfg: &GameConfig) -> Result<Self, String> {
		let mut next_id = 1u32;
		let mut rules = cfg.rules;
		if let Some(s) = cfg.scenario.as_ref() {
			if s.generative {
				rules.identical_bags = s.mirror;
			}
		}
		let decks = if let Some(s) = cfg.scenario.as_ref() {
			if s.generative {
				deck::generate_both_collections(&rules, &cfg.setup, cfg.seed, &mut next_id)
			} else {
				let game = Game::from_scenario_with(rules, cfg.seed, s, cfg.setup)?;
				let rec = game.setup_record().cloned().ok_or("scenario produced no setup")?;
				return Ok(Self {
					rules,
					seed: cfg.seed,
					cfg: cfg.setup,
					decks: rec.decks,
					pick7: [Vec::new(), Vec::new()],
					extend: [Vec::new(), Vec::new()],
					selected: Vec::new(),
					stage: SetupStage::Opening(Player::One),
				});
			}
		} else {
			deck::generate_both_collections(&rules, &cfg.setup, cfg.seed, &mut next_id)
		};
		Ok(Self {
			rules,
			seed: cfg.seed,
			cfg: cfg.setup,
			decks,
			pick7: [Vec::new(), Vec::new()],
			extend: [Vec::new(), Vec::new()],
			selected: Vec::new(),
			stage: SetupStage::Opening(Player::One),
		})
	}

	fn actor_player(&self) -> Option<Player> {
		match self.stage {
			SetupStage::Opening(p) | SetupStage::Extend(p) => Some(p),
			SetupStage::Reveal | SetupStage::Done => None,
		}
	}

	fn need(&self) -> usize {
		match self.stage {
			SetupStage::Opening(p) => pick_plan(self.decks[p.index()].len()).0,
			SetupStage::Extend(p) => pick_plan(self.decks[p.index()].len()).1,
			SetupStage::Reveal | SetupStage::Done => 0,
		}
	}

	fn reserve_tiles(&self, p: Player) -> Vec<Tile> {
		let used = self.picked_tiles(p, true);
		self.decks[p.index()]
			.iter()
			.copied()
			.filter(|t| !used.iter().any(|u| u.id == t.id))
			.collect()
	}

	fn picked_tiles(&self, p: Player, opening_only: bool) -> Vec<Tile> {
		let deck = &self.decks[p.index()];
		let mut out: Vec<Tile> =
			self.pick7[p.index()].iter().filter_map(|&i| deck.get(i).copied()).collect();
		if !opening_only {
			out.extend(self.extend[p.index()].iter().filter_map(|&i| deck.get(i).copied()));
		}
		out
	}

	fn committed_tiles(&self, p: Player) -> Vec<Tile> {
		match self.stage {
			SetupStage::Opening(_) => Vec::new(),
			SetupStage::Extend(_) | SetupStage::Reveal | SetupStage::Done => {
				self.picked_tiles(p, true)
			}
		}
	}

	fn pool_tiles(&self) -> Vec<Tile> {
		match self.stage {
			SetupStage::Opening(p) => self.decks[p.index()].clone(),
			SetupStage::Extend(p) => self.reserve_tiles(p),
			SetupStage::Reveal | SetupStage::Done => Vec::new(),
		}
	}

	fn candidate_tiles(&self, p: Player) -> Vec<Tile> {
		let mut cur = self.committed_tiles(p);
		let pool = self.pool_tiles();
		for &i in &self.selected {
			if let Some(t) = pool.get(i) {
				cur.push(*t);
			}
		}
		cur
	}

	fn can_confirm(&self) -> bool {
		match self.stage {
			SetupStage::Reveal | SetupStage::Done => true,
			SetupStage::Opening(p) | SetupStage::Extend(p) => {
				self.selected.len() == self.need() && respects_type_cap(&self.candidate_tiles(p))
			}
		}
	}

	fn toggle(&mut self, index: usize) -> Result<(), String> {
		let Some(p) = self.actor_player() else {
			return Ok(());
		};
		if let Some(pos) = self.selected.iter().position(|&i| i == index) {
			self.selected.remove(pos);
			return Ok(());
		}
		if self.selected.len() >= self.need() {
			return Err(format!("already picked {} tiles", self.need()));
		}
		let pool = self.pool_tiles();
		let Some(tile) = pool.get(index).copied() else {
			return Err("no tile there".into());
		};
		let mut cur = self.committed_tiles(p);
		for &i in &self.selected {
			if let Some(t) = pool.get(i) {
				cur.push(*t);
			}
		}
		if !would_respect_cap(&cur, tile) {
			cur.push(tile);
			return Err(format!(
				"illegal: exceeds the 6-ring-per-type cap ({})",
				visual::format_setup_cap(&cur)
			));
		}
		self.selected.push(index);
		Ok(())
	}

	fn visible_bench(&self, p: Player) -> Vec<Tile> {
		match self.stage {
			SetupStage::Opening(cur) if cur == p => self.candidate_tiles(p),
			SetupStage::Opening(_) => Vec::new(),
			SetupStage::Extend(cur) if cur == p => self.candidate_tiles(p),
			SetupStage::Reveal | SetupStage::Extend(_) | SetupStage::Done => {
				self.picked_tiles(p, true)
			}
		}
	}

	fn auto_pick(&mut self, p: Player) {
		let (opening, ext) = pick_plan(self.decks[p.index()].len());
		match self.stage {
			SetupStage::Opening(cur) if cur == p => {
				let tiles = pick_from_pool(
					&self.decks[p.index()],
					opening,
					&[],
					&[],
					ext,
					p,
					&self.rules,
					self.cfg.pick,
					self.cfg.limit,
					self.cfg.deck_seed(self.seed) ^ 0x0707,
					self.cfg.search_for(p),
				);
				self.pick7[p.index()] = indices_of(&self.decks[p.index()], &tiles);
				self.selected.clear();
				self.stage = next_after_opening(p);
			}
			SetupStage::Extend(cur) if cur == p => {
				let mine7 = self.picked_tiles(p, true);
				let opp7 = self.picked_tiles(p.other(), true);
				let reserves = self.reserve_tiles(p);
				let tiles = pick_from_pool(
					&reserves,
					ext,
					&mine7,
					&opp7,
					0,
					p,
					&self.rules,
					self.cfg.pick,
					self.cfg.limit,
					self.cfg.deck_seed(self.seed) ^ 0xE4E4,
					self.cfg.search_for(p),
				);
				self.extend[p.index()] = indices_of(&self.decks[p.index()], &tiles);
				self.selected.clear();
				self.stage = next_after_extend(p);
			}
			_ => {}
		}
	}

	fn confirm_human(&mut self) -> Result<(), String> {
		match self.stage {
			SetupStage::Reveal => {
				self.stage = SetupStage::Extend(Player::One);
				self.selected.clear();
				return Ok(());
			}
			SetupStage::Done => return Ok(()),
			SetupStage::Opening(_) | SetupStage::Extend(_) => {}
		}
		let Some(p) = self.actor_player() else {
			return Ok(());
		};
		if !self.can_confirm() {
			return Err(
				"pick a legal set under the 6-ring-per-type cap (braids count by slots)".into()
			);
		}
		match self.stage {
			SetupStage::Opening(_) => {
				self.pick7[p.index()] = self.selected.clone();
				self.selected.clear();
				self.stage = next_after_opening(p);
			}
			SetupStage::Extend(_) => {
				let reserves = self.reserve_tiles(p);
				let chosen: Vec<Tile> =
					self.selected.iter().filter_map(|&i| reserves.get(i).copied()).collect();
				self.extend[p.index()] = indices_of(&self.decks[p.index()], &chosen);
				self.selected.clear();
				self.stage = next_after_extend(p);
			}
			SetupStage::Reveal | SetupStage::Done => {}
		}
		Ok(())
	}

	fn is_ready(&self) -> bool {
		matches!(self.stage, SetupStage::Done)
	}

	fn finish(self) -> SetupRecord {
		let tiles = |deck: &[Tile], idx: &[usize]| {
			idx.iter().filter_map(|&i| deck.get(i).copied()).collect::<Vec<_>>()
		};
		SetupRecord {
			decks: self.decks.clone(),
			pick7: [tiles(&self.decks[0], &self.pick7[0]), tiles(&self.decks[1], &self.pick7[1])],
			extend: [
				tiles(&self.decks[0], &self.extend[0]),
				tiles(&self.decks[1], &self.extend[1]),
			],
			strategy: self.cfg.pick,
			deck_seed: self.cfg.deck_seed(self.seed),
			deck_noise: self.cfg.deck_noise,
			p1_deck: self.cfg.p1_deck,
			p2_deck: self.cfg.p2_deck,
		}
	}

	fn prompt(&self) -> String {
		match self.stage {
			SetupStage::Opening(p) => format!(
				"{}: pick {} tiles for your bench ({})",
				p.label(),
				self.need(),
				visual::format_setup_cap(&self.candidate_tiles(p))
			),
			SetupStage::Reveal => {
				"Both 7-tile benches are public. Click confirm to pick 4 from reserves.".into()
			}
			SetupStage::Done => "Starting...".into(),
			SetupStage::Extend(p) => format!(
				"{}: pick {} from reserves ({})",
				p.label(),
				self.need(),
				visual::format_setup_cap(&self.candidate_tiles(p))
			),
		}
	}
}

fn next_after_opening(p: Player) -> SetupStage {
	match p {
		Player::One => SetupStage::Opening(Player::Two),
		Player::Two => SetupStage::Reveal,
	}
}

fn next_after_extend(p: Player) -> SetupStage {
	match p {
		Player::One => SetupStage::Extend(Player::Two),
		Player::Two => SetupStage::Done,
	}
}

fn indices_of(deck: &[Tile], chosen: &[Tile]) -> Vec<usize> {
	chosen.iter().filter_map(|t| deck.iter().position(|d| d.id == t.id)).collect()
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

fn setup_camera(mut commands: Commands) {
	commands.spawn(Camera2d::default());
}

fn spawn_match_hud(mut commands: Commands, font: Res<UiFont>) {
	spawn_hud_line(&mut commands, &font, StatusHud, 16.0, 8.0, 17.0, Color::srgb(0.92, 0.93, 0.90));
	spawn_hud_line(&mut commands, &font, TurnHud, 16.0, 30.0, 15.0, Color::srgb(0.88, 0.90, 0.86));
	spawn_hud_line(&mut commands, &font, LogHud, 16.0, 52.0, 15.0, Color::srgb(0.78, 0.82, 0.72));
	spawn_hud_line(&mut commands, &font, WarnHud, 16.0, 74.0, 15.0, Color::srgb(0.95, 0.72, 0.28));
	commands.spawn((
		HelpHud,
		Text::new(visual::HELP_TEXT),
		ui_text_font(&font, 15.0),
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

fn despawn_match_hud(
	mut commands: Commands,
	hud: Query<
		Entity,
		Or<(
			With<StatusHud>,
			With<TurnHud>,
			With<LogHud>,
			With<WarnHud>,
			With<HelpHud>,
			With<DynamicView>,
		)>,
	>,
) {
	for e in &hud {
		commands.entity(e).despawn();
	}
}

fn spawn_hud_line<M: Component>(
	commands: &mut Commands,
	font: &UiFont,
	marker: M,
	left: f32,
	top: f32,
	size: f32,
	color: Color,
) {
	commands.spawn((
		marker,
		Text::new(""),
		ui_text_font(font, size),
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
	let next = ViewLayout::from_window_with_deck(
		window.width(),
		window.height(),
		play.game.rules().bag_size as usize,
	);
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
	mut leave: MessageWriter<LeaveMatch>,
) {
	if keys.just_pressed(KeyCode::Escape) {
		let setup_sel = play.setup.as_ref().is_some_and(|s| !s.selected.is_empty());
		if play.selection != Selection::None || setup_sel {
			if let Some(session) = play.setup.as_mut() {
				session.selected.clear();
			}
			play.selection = Selection::None;
			play.warning.clear();
			return;
		}
		if play.from_shell {
			leave.write(LeaveMatch);
			return;
		}
		play.warning.clear();
		return;
	}
	if !buttons.just_pressed(MouseButton::Left) {
		return;
	}
	if play.game.status().is_over() {
		return;
	}
	if let Some(r) = play.remote.as_ref() {
		if let Err(msg) = r.can_act(&play.game) {
			play.warning = msg.to_string();
			return;
		}
	}
	let allow_setup = match play.setup.as_ref().map(|s| s.stage) {
		Some(SetupStage::Reveal | SetupStage::Done) => true,
		Some(SetupStage::Opening(p) | SetupStage::Extend(p)) => cfg.actor(p).is_human(),
		None => false,
	};
	if play.setup.is_some() && !allow_setup {
		return;
	}
	if play.setup.is_none() {
		let actor = cfg.actor(play.game.current_player());
		if !actor.is_human() {
			return;
		}
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
	if play.setup.is_some() {
		try_click_setup(&mut play, &layout.0, world_pos);
		complete_setup_if_ready(&mut play, cfg.time_control);
		return;
	}
	try_click(&mut play, &layout.0, world_pos);
}

fn try_click_setup(play: &mut PlayState, layout: &ViewLayout, pos: Vec2) {
	let Some(session) = play.setup.as_mut() else {
		return;
	};
	if hit_setup_confirm(layout, pos) {
		match session.confirm_human() {
			Ok(()) => {
				play.warning.clear();
				play.log = session.prompt();
			}
			Err(err) => play.warning = err,
		}
		return;
	}
	let pool_len = session.pool_tiles().len();
	if let Some(i) = hit_setup_grid(pool_len, layout, pos) {
		match session.toggle(i) {
			Ok(()) => {
				play.warning.clear();
				play.log = session.prompt();
			}
			Err(err) => play.warning = err,
		}
	}
}

fn hit_setup_confirm(layout: &ViewLayout, pos: Vec2) -> bool {
	let p = layout.setup_confirm_pos();
	pos.distance_squared(Vec2::new(p[0], p[1])) <= 52.0 * 52.0
}

fn hit_setup_grid(len: usize, layout: &ViewLayout, pos: Vec2) -> Option<usize> {
	let r = layout.bench_tile_radius + 8.0;
	let mut best = None;
	let mut best_d = r * r;
	for i in 0..len {
		let p = layout.setup_grid_pos(i, len);
		let d = pos.distance_squared(Vec2::new(p[0], p[1]));
		if d <= best_d {
			best_d = d;
			best = Some(i);
		}
	}
	best
}

fn complete_setup_if_ready(play: &mut PlayState, time_control: Option<crate::clock::TimeControl>) {
	let Some(session) = play.setup.take() else {
		return;
	};
	if !session.is_ready() {
		play.setup = Some(session);
		return;
	}
	let rules = session.rules;
	let seed = session.seed;
	let rec = session.finish();
	play.game = Game::from_setup_record(rules, seed, rec);
	if let Some(tc) = time_control {
		play.game.set_time_control(tc);
	}
	play.log = "Setup complete. 11-tile benches are public.".into();
	play.warning.clear();
	play.selection = Selection::None;
	play.fingerprint = 0;
	begin_play_log(play);
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
				if play.game.legal_swap_partners(from).contains(&node) {
					apply_action(play, Action::Swap { from, to: node });
					return;
				}
			}
			_ => {}
		}
		if let Some(tile) = play.game.tile_at(node) {
			if play.game.legal_converts().contains(&Action::Convert { node }) {
				apply_action(play, Action::Convert { node });
				return;
			}
			if play.game.rules().capture_mode.allows_claim() && play.game.is_overwhelmed(node) {
				apply_action(play, Action::Claim { node });
				return;
			}
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
	apply_action_with_stats(play, action, None);
}

fn apply_action_with_stats(play: &mut PlayState, action: Action, stats: Option<SearchStats>) {
	let piece = bench_log::piece_for_action(&play.game, action);
	let partner = bench_log::partner_for_action(&play.game, action);
	let pending_tx = play.remote.as_ref().and_then(|r| r.transaction(action));
	match play.game.apply(action) {
		Ok(outcome) => {
			if let Some(tx) = pending_tx {
				play.outbox = Some(tx);
				if let Some(r) = play.remote.as_mut() {
					r.inflight = true;
				}
			}
			play.flash = visual::CaptureFlash::from_outcome(&outcome);
			play.log = outcome.log_line.clone();
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
			play.bot_thinking = false;
			play.game.credit_increment(outcome.player);
			play.search1.advance(&play.game);
			play.search2.advance(&play.game);
			record_play_action(play, action, piece, partner, &outcome, stats);
		}
		Err(err) => {
			play.warning = err;
		}
	}
}

fn play_log_meta(cfg: &GameConfig, screenshot: bool) -> Option<PlayLogMeta> {
	if screenshot || cfg.games_log_off {
		return None;
	}
	let path = cfg.games_log.clone().unwrap_or_else(|| play_log::new_path(cfg.seed));
	let p1 = cfg.bot1.name();
	let p2 = cfg.bot2.name();
	Some(PlayLogMeta {
		path,
		think_ms: cfg.think.as_millis() as u64,
		pairing: format!("{p1} vs {p2}"),
		p1: p1.to_string(),
		p2: p2.to_string(),
		search: cfg.search.label(),
		scenario: cfg
			.scenario
			.as_ref()
			.map(|s| s.name.clone())
			.unwrap_or_else(|| "default".into()),
	})
}

fn begin_play_log(play: &mut PlayState) {
	let Some(meta) = play.log_meta.as_ref() else {
		return;
	};
	if play.game_log.is_some() {
		return;
	}
	let mut log = GameLog::begin(
		1,
		meta.pairing.clone(),
		meta.p1.clone(),
		meta.p2.clone(),
		meta.think_ms,
		meta.scenario.clone(),
		&play.game,
	);
	log.p1_search = meta.search.clone();
	log.p2_search = meta.search.clone();
	let path = meta.path.clone();
	play.game_log = Some(log);
	if let Some(log) = play.game_log.as_ref() {
		if let Err(err) = play_log::write_log(&path, log) {
			eprintln!("{err}");
		} else {
			eprintln!("game log: {}", path.display());
		}
	}
}

fn record_play_action(
	play: &mut PlayState,
	action: Action,
	piece: Option<crate::game::Tile>,
	partner: Option<crate::game::Tile>,
	outcome: &crate::game::ActionOutcome,
	stats: Option<SearchStats>,
) {
	let Some(log) = play.game_log.as_mut() else {
		return;
	};
	if let Some(st) = stats {
		log.push_action_with_search_partner(action, piece, partner, outcome, &play.game, st);
	} else {
		log.push_action_partner(action, piece, partner, outcome, &play.game);
	}
	if play.game.status().is_over() {
		log.finish(&play.game, bench_log::low_value_of(&play.game));
	}
	let path = play.log_meta.as_ref().map(|m| m.path.clone());
	if let (Some(path), Some(log)) = (path, play.game_log.as_ref()) {
		if let Err(err) = play_log::write_log(&path, log) {
			eprintln!("{err}");
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
				play.warning = format!("Spawn {node}: pick an unplaced deck tile to ADD.");
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
	let deck = game.deck_slots(player);
	let n = deck.len();
	for (slot_i, slot) in deck.iter().enumerate() {
		let DeckSlot::Live(tile) = slot else {
			continue;
		};
		let p = layout.bench_pos(player, slot_i, n);
		let p = Vec2::new(p[0], p[1]);
		if pos.distance_squared(p)
			<= (layout.bench_tile_radius + 6.0) * (layout.bench_tile_radius + 6.0)
		{
			return game.bench(player).iter().position(|t| t.id == tile.id);
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
	if cfg.screenshot.is_some() {
		return;
	}
	if play.setup.is_some() {
		complete_setup_if_ready(&mut play, cfg.time_control);
		let human_waiting = match play.setup.as_ref().map(|s| s.stage) {
			Some(SetupStage::Opening(p) | SetupStage::Extend(p)) => cfg.actor(p).is_human(),
			Some(SetupStage::Reveal | SetupStage::Done) | None => false,
		};
		if human_waiting {
			clock.wait = cfg.delay.as_secs_f32();
			return;
		}
		if play.setup.is_none() {
			return;
		}
		clock.wait -= time.delta().as_secs_f32();
		if clock.wait > 0.0 {
			return;
		}
		clock.wait = cfg.delay.as_secs_f32();
		if let Some(session) = play.setup.as_mut() {
			match session.stage {
				SetupStage::Reveal => {
					let _ = session.confirm_human();
				}
				SetupStage::Opening(p) | SetupStage::Extend(p) => {
					session.auto_pick(p);
				}
				SetupStage::Done => {}
			}
		}
		let prompt = play.setup.as_ref().map(|s| s.prompt()).unwrap_or_default();
		play.log = prompt;
		play.warning.clear();
		complete_setup_if_ready(&mut play, cfg.time_control);
		return;
	}
	if play.game.status().is_over() {
		play.bot_thinking = false;
		return;
	}
	let actor = cfg.actor(play.game.current_player());
	if actor.is_human() {
		clock.wait = cfg.delay.as_secs_f32();
		play.bot_thinking = false;
		return;
	}
	if !play.bot_thinking {
		clock.wait -= time.delta().as_secs_f32();
		if clock.wait > 0.0 {
			return;
		}
	}
	play.bot_thinking = true;
	let player = play.game.current_player();
	let think = cfg.think;
	let slice = search_slice();
	let (action, stats) = {
		let PlayState { game, search1, search2, .. } = &mut *play;
		let searcher = match player {
			Player::One => search1,
			Player::Two => search2,
		};
		let action = searcher.think_slice(game, actor, &mut rng.0, SearchLimit::Time(think), slice);
		let stats = searcher.stats();
		(action, stats)
	};
	let Some(action) = action else {
		return;
	};
	play.bot_thinking = false;
	clock.wait = cfg.delay.as_secs_f32();
	apply_action_with_stats(&mut play, action, Some(stats));
}

fn tick_clock(time: Res<Time>, mut play: ResMut<PlayState>) {
	if play.setup.is_some() || play.game.clock().is_none() || play.game.status().is_over() {
		return;
	}
	let ms = (time.delta_secs() * 1000.0).round() as u64;
	if ms == 0 {
		return;
	}
	let player = play.game.current_player();
	if play.game.apply_elapsed(player, ms).ok() == Some(crate::clock::ClockTick::Flagged) {
		play.log = "flag-fall".into();
		play.bot_thinking = false;
		play.fingerprint = 0;
	}
}

fn handle_start_match(
	mut starts: MessageReader<StartMatch>,
	mut cfg: ResMut<ConfigRes>,
	mut play: ResMut<PlayState>,
	mut rng: ResMut<RngRes>,
	mut clock: ResMut<BotClock>,
	mut layout: ResMut<UiLayout>,
	mut screen: ResMut<NextState<AppScreen>>,
) {
	let Some(next) = starts.read().map(|StartMatch { cfg }| cfg.clone()).last() else {
		return;
	};
	cfg.0 = next.clone();
	*play = play_state_from_config(&next);
	play.from_shell = true;
	rng.0 = StdRng::seed_from_u64(next.seed ^ 0xA5A5_A5A5);
	clock.wait = next.delay.as_secs_f32();
	layout.0 = ViewLayout::from_window_with_deck(
		layout.width,
		layout.height,
		play.game.rules().bag_size as usize,
	);
	screen.set(AppScreen::Match);
}

/// After [`handle_game_deep_link`] builds the match, attach it to the
/// server-hosted game when an API is configured. Without one the seeded local
/// stub stays (offline).
fn attach_remote_session(
	mut links: MessageReader<IncomingUrl>,
	mut play: ResMut<PlayState>,
	mut cfg: ResMut<ConfigRes>,
	host: Res<shell::HostRes>,
) {
	let mut last = None;
	for IncomingUrl { uri } in links.read() {
		let link = DeepLink::parse(uri);
		if let Route::Game { id } = &link.route {
			last = Some((id.clone(), link.query.clone()));
		}
	}
	let Some((id, query)) = last else {
		return;
	};
	if host.0.api_url.is_none() || !remote::HTTP_AVAILABLE {
		play.log = format!("Offline game {id}: seeded locally (no API configured).");
		return;
	}
	let (mut seat, mut token) = remote::seat_from_query(&query);
	if token.is_none() {
		if let Some((s, t)) = remote::recall_seat(&id) {
			seat = Some(s);
			token = Some(t);
		}
	} else if let (Some(s), Some(t)) = (seat, token.as_deref()) {
		remote::remember_seat(&id, s, t);
	}
	let mut session = remote::RemoteSession::new(id.clone(), seat, token);
	session.inflight = remote::request_view(&host.0, &id);
	cfg.0.bot1 = crate::ai::Actor::Human;
	cfg.0.bot2 = crate::ai::Actor::Human;
	play.setup = None;
	play.game_log = None;
	play.log_meta = None;
	play.log = format!("Loading game {id} from the server…");
	play.remote = Some(session);
	play.outbox = None;
	play.fingerprint = 0;
}

/// Seat-2 link from a just-created hosted game, carried into the session.
#[derive(Default)]
struct PendingInvite(Option<(String, String)>);

/// Send queued transactions, poll while the other seat moves, and apply
/// server replies.
fn remote_sync(
	time: Res<Time>,
	host: Res<shell::HostRes>,
	mut play: ResMut<PlayState>,
	mut lobby: ResMut<shell::ShellState>,
	mut invite: Local<PendingInvite>,
) {
	for ev in remote::drain_events() {
		match ev {
			remote::RemoteEvent::Created { created } => {
				let p1 = &created.seats[0];
				let p2 = &created.seats[1];
				let link = remote::seat_link(&host.0, &created.game_id, p2.seat, &p2.token);
				share_invite(&link);
				invite.0 = Some((created.game_id.clone(), link));
				crate::platform::push_incoming_url(format!(
					"/game/{}?seat={}&token={}",
					created.game_id, p1.seat, p1.token
				));
			}
			remote::RemoteEvent::View { id, view }
			| remote::RemoteEvent::TxAccepted { id, view } => {
				adopt_remote_view(&mut play, &id, &view);
			}
			remote::RemoteEvent::TxRejected { id, message } => {
				if play.remote.as_ref().is_some_and(|r| r.id == id) {
					play.warning = format!("Server rejected the move: {message}. Reloading.");
					if let Some(r) = play.remote.as_mut() {
						// Force a full reload of the authoritative record.
						r.loaded = false;
						r.inflight = remote::request_view(&host.0, &id);
					}
				}
			}
			remote::RemoteEvent::Failed { id, message } => {
				if id.is_empty() {
					lobby.set_notice(format!("Could not create a hosted game: {message}"));
				} else if let Some(r) = play.remote.as_mut().filter(|r| r.id == id) {
					r.inflight = false;
					play.warning = format!("Server: {message}");
				}
			}
		}
	}
	if let Some((id, link)) = invite.0.as_ref() {
		if let Some(r) = play.remote.as_mut().filter(|r| &r.id == id) {
			r.invite = Some(link.clone());
			invite.0 = None;
		}
	}
	if let Some(tx) = play.outbox.take() {
		if !remote::submit(&host.0, &tx) {
			if let Some(r) = play.remote.as_mut() {
				r.inflight = false;
			}
			play.warning = "No API configured; move kept locally only.".into();
		}
	}
	let Some(r) = play.remote.as_ref() else {
		return;
	};
	if r.wants_poll(&play.game) {
		let id = r.id.clone();
		let wait = r.poll_wait - time.delta_secs();
		let waiting = r.invite.as_ref().map(|l| format!("Waiting for the other seat. Invite: {l}"));
		if let Some(r) = play.remote.as_mut() {
			if wait <= 0.0 {
				r.poll_wait = remote::POLL_SECS;
				r.inflight = remote::request_view(&host.0, &id);
			} else {
				r.poll_wait = wait;
			}
		}
		if let Some(msg) = waiting {
			if play.warning.is_empty() || play.warning.starts_with("Waiting") {
				play.warning = msg;
			}
		}
	}
}

fn adopt_remote_view(play: &mut PlayState, id: &str, view: &apa_kan_view::GameView) {
	let Some(r) = play.remote.as_mut().filter(|r| r.id == id) else {
		return;
	};
	r.inflight = false;
	match r.adopt(view) {
		Ok(Some(game)) => {
			let seat = match r.seat {
				Some(s) => format!("you are seat {s}"),
				None => "spectating".into(),
			};
			let labels = format!("{} vs {}", r.labels[0], r.labels[1]);
			play.game = game;
			play.setup = None;
			play.selection = Selection::None;
			play.bot_thinking = false;
			play.search1.advance(&play.game);
			play.search2.advance(&play.game);
			play.log = format!("Game {id} · {labels} · seq {} · {seat}", view.seq);
			if play.warning.starts_with("Waiting") || play.warning.starts_with("Server") {
				play.warning.clear();
			}
			play.fingerprint = 0;
		}
		Ok(None) => {}
		Err(err) => play.warning = format!("Could not replay game {id}: {err}"),
	}
}

/// Put the seat-2 link where the host can copy it: clipboard + console on
/// web, stdout on desktop. Bevy text is not selectable.
fn share_invite(link: &str) {
	#[cfg(target_arch = "wasm32")]
	{
		web_sys::console::log_1(&format!("Apa Kan invite (seat 2): {link}").into());
		if let Some(w) = web_sys::window() {
			let _ = w.navigator().clipboard().write_text(link);
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		println!("Apa Kan invite (seat 2): {link}");
	}
}

/// A `/game/<id>` link replaces any Divisions match in flight without rating it.
fn clear_division_on_game_link(
	mut links: MessageReader<IncomingUrl>,
	mut division: ResMut<shell::DivisionMatch>,
) {
	if links
		.read()
		.any(|IncomingUrl { uri }| matches!(DeepLink::parse(uri).route, Route::Game { .. }))
	{
		division.pending = None;
	}
}

/// Rate a finished offline Divisions match once, then clear it.
fn record_division_result(
	mut play: ResMut<PlayState>,
	mut division: ResMut<shell::DivisionMatch>,
	mut lobby: ResMut<shell::ShellState>,
) {
	if division.pending.is_none() || play.setup.is_some() {
		return;
	}
	let Some(outcome) = shell::division_outcome(play.game.status()) else {
		return;
	};
	division.pending = None;
	let msg = shell::record_division(&mut lobby, outcome);
	play.log = format!("{} {msg}", play.log).trim().to_string();
	play.fingerprint = 0;
}

fn handle_leave_match(
	mut leaves: MessageReader<LeaveMatch>,
	mut screen: ResMut<NextState<AppScreen>>,
	mut play: ResMut<PlayState>,
	mut division: ResMut<shell::DivisionMatch>,
) {
	if leaves.read().next().is_none() {
		return;
	}
	// Leaving an unfinished Divisions match does not rate it.
	division.pending = None;
	play.bot_thinking = false;
	replace_browser_route(&Route::Home);
	screen.set(AppScreen::Shell);
}

fn handle_game_deep_link(
	mut links: MessageReader<IncomingUrl>,
	mut cfg: ResMut<ConfigRes>,
	mut play: ResMut<PlayState>,
	mut rng: ResMut<RngRes>,
	mut clock: ResMut<BotClock>,
	mut layout: ResMut<UiLayout>,
	mut screen: ResMut<NextState<AppScreen>>,
) {
	let mut id = None;
	for IncomingUrl { uri } in links.read() {
		if let Route::Game { id: game_id } = DeepLink::parse(uri).route {
			id = Some(game_id);
		}
	}
	let Some(id) = id else {
		return;
	};
	let mut next = cfg.0.clone();
	next.match_id = Some(id.clone());
	next.seed = shell::seed_from_match_id(&id);
	next.setup.deck_seed = Some(next.seed);
	next.show_shell = false;
	next.bot1 = crate::ai::Actor::Human;
	next.bot2 = crate::ai::Actor::Greedy;
	cfg.0 = next.clone();
	*play = play_state_from_config(&next);
	play.from_shell = true;
	rng.0 = StdRng::seed_from_u64(next.seed ^ 0xA5A5_A5A5);
	clock.wait = next.delay.as_secs_f32();
	layout.0 = ViewLayout::from_window_with_deck(
		layout.width,
		layout.height,
		play.game.rules().bag_size as usize,
	);
	replace_browser_route(&Route::Game { id });
	screen.set(AppScreen::Match);
}

fn refresh_view(
	mut commands: Commands,
	mut play: ResMut<PlayState>,
	layout: Res<UiLayout>,
	font: Res<UiFont>,
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
	// Legal landings under the current RuleSet (counted pass-through by
	// default; graph hops when `--graph-hops` is on). Occupied pass-through
	// nodes are never highlighted because they are never destinations.
	let dests: Vec<NodeId> = match play.selection {
		Selection::Piece(n) => play.game.legal_destinations(n),
		_ => Vec::new(),
	};
	// Pink rings on legal SWAP partners only. Under NormalOnly that is
	// empty unless the selected tile is a 1-ring normal.
	let swap_partners: Vec<NodeId> = match play.selection {
		Selection::Piece(n) => play.game.legal_swap_partners(n),
		_ => Vec::new(),
	};
	let convert_targets: Vec<NodeId> = play
		.game
		.legal_converts()
		.into_iter()
		.filter_map(|a| match a {
			Action::Convert { node } => Some(node),
			_ => None,
		})
		.collect();

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
			play.game.rules(),
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
		if swap_partners.contains(&id) {
			spawn_ring(
				&mut commands,
				&mut meshes,
				&mut materials,
				p,
				layout.tile_radius + 4.0,
				layout.tile_radius + 7.0,
				rgb(visual::SWAP_RGB),
				visual::Z_HIGHLIGHT,
			);
		}
		if convert_targets.contains(&id) {
			spawn_ring(
				&mut commands,
				&mut meshes,
				&mut materials,
				p,
				layout.tile_radius + 4.0,
				layout.tile_radius + 7.0,
				rgb(visual::CONVERT_RGB),
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

	if play.setup.is_some() {
		draw_setup_view(
			&mut commands,
			&mut meshes,
			&mut materials,
			layout,
			&font,
			play.setup.as_ref().unwrap(),
		);
		return;
	}

	for id in 0..board.node_count() as NodeId {
		if let Some(tile) = play.game.tile_at(id) {
			let claimable =
				play.game.rules().capture_mode.allows_claim() && play.game.is_overwhelmed(id);
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				world_v(layout, board.position(id)),
				tile,
				matches!(play.selection, Selection::Piece(n) if n == id),
				false,
				layout.tile_radius,
			);
			if claimable {
				spawn_label(
					&mut commands,
					&font,
					world_v(layout, board.position(id))
						+ Vec2::Y * (layout.tile_radius * 0.55 + 12.0),
					"CLAIM",
				);
			} else if swap_partners.contains(&id) {
				spawn_label(
					&mut commands,
					&font,
					world_v(layout, board.position(id))
						+ Vec2::Y * (layout.tile_radius * 0.55 + 12.0),
					"SWAP",
				);
			}
		} else if board.is_spawn(id) && matches!(play.selection, Selection::Bench(_)) {
			spawn_label(
				&mut commands,
				&font,
				world_v(layout, board.position(id)) + Vec2::Y * (layout.tile_radius * 0.55 + 12.0),
				"ADD",
			);
		}
	}

	for player in [Player::Two, Player::One] {
		let deck = play.game.deck_slots(player);
		let selected = play.game.current_player() == player;
		let n = deck.len();
		for (i, slot) in deck.iter().enumerate() {
			let p = layout.bench_pos(player, i, n);
			match slot {
				DeckSlot::Empty => spawn_solid_token(
					&mut commands,
					&mut meshes,
					&mut materials,
					Vec2::new(p[0], p[1]),
					player,
					layout.bench_tile_radius,
					0,
					false,
				),
				DeckSlot::Live(tile) => {
					let bench_i = play.game.bench(player).iter().position(|t| t.id == tile.id);
					let highlight = selected
						&& bench_i
							.map(|idx| play.selection == Selection::Bench(idx))
							.unwrap_or(false);
					let just_captured = play.flash.highlights_tile(tile.id, CaptureDest::Bench);
					spawn_tile_at(
						&mut commands,
						&mut meshes,
						&mut materials,
						Vec2::new(p[0], p[1]),
						*tile,
						highlight,
						just_captured,
						layout.bench_tile_radius,
					);
				}
				DeckSlot::Spent(_) => {}
			}
		}
		let tokens = play.game.budget_tokens(player);
		let n = tokens.len();
		for i in 0..n {
			let p = layout.budget_stack_pos(player, i, n);
			let just_captured = match tokens[i] {
				BudgetToken::Captured(tile) => {
					play.flash.highlights_tile(tile.id, CaptureDest::Tokens)
				}
				BudgetToken::Blank => false,
			};
			spawn_solid_token(
				&mut commands,
				&mut meshes,
				&mut materials,
				Vec2::new(p[0], p[1]),
				player,
				layout.bench_tile_radius,
				i,
				just_captured,
			);
		}
	}

	if !play.flash.is_empty() {
		let n = play.flash.tiles.len();
		let r = layout.capture_flash_radius();
		for (i, tile) in play.flash.tiles.iter().enumerate() {
			let p = layout.capture_flash_pos(i, n);
			spawn_tile_at(
				&mut commands,
				&mut meshes,
				&mut materials,
				Vec2::new(p[0], p[1]),
				*tile,
				false,
				false,
				r,
			);
		}
	}
}

fn draw_setup_view(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	layout: &ViewLayout,
	font: &UiFont,
	session: &SetupSession,
) {
	let pool = session.pool_tiles();
	let n = pool.len();
	for (i, tile) in pool.iter().enumerate() {
		let p = layout.setup_grid_pos(i, n);
		let selected = session.selected.contains(&i);
		spawn_tile_at(
			commands,
			meshes,
			materials,
			Vec2::new(p[0], p[1]),
			*tile,
			selected,
			false,
			layout.bench_tile_radius,
		);
	}
	for player in [Player::Two, Player::One] {
		let bench = session.visible_bench(player);
		let n = bench.len();
		for (i, tile) in bench.iter().enumerate() {
			let p = layout.bench_pos(player, i, n.max(1));
			spawn_tile_at(
				commands,
				meshes,
				materials,
				Vec2::new(p[0], p[1]),
				*tile,
				false,
				false,
				layout.bench_tile_radius,
			);
		}
	}
	if !matches!(session.stage, SetupStage::Done) {
		let p = layout.setup_confirm_pos();
		let label = if session.can_confirm() { "CONFIRM" } else { "pick" };
		spawn_label(commands, font, Vec2::new(p[0], p[1]), label);
		if session.can_confirm() {
			spawn_ring(
				commands,
				meshes,
				materials,
				Vec2::new(p[0], p[1]),
				layout.bench_tile_radius + 4.0,
				layout.bench_tile_radius + 8.0,
				rgb(visual::HIGHLIGHT_RGB),
				visual::Z_HIGHLIGHT,
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
	rules: &crate::game::RuleSet,
) {
	let samples = arc.sample(visual::ARC_SAMPLES);
	let pts: Vec<[f64; 2]> = samples
		.iter()
		.map(|p| {
			let w = layout.board_to_world(*p);
			[f64::from(w[0]), f64::from(w[1])]
		})
		.collect();
	let width = layout.arc_width;
	let paint = visual::arc_paint_for_rules(from, to, rules);
	let stripe_off = if visual::arc_touches_center(arc, center) {
		f64::from(layout.stripe_off_center)
	} else {
		f64::from(layout.stripe_off)
	};
	let hitch_r = if visual::arc_touches_center(arc, center) {
		f64::from(layout.hitch_r_center)
	} else {
		f64::from(layout.hitch_r)
	};
	let braid_amp = if visual::arc_touches_center(arc, center) {
		f64::from(layout.braid_amp_center)
	} else {
		f64::from(layout.braid_amp)
	};
	let zag_amp = if visual::arc_touches_center(arc, center) {
		f64::from(layout.zag_amp_center)
	} else {
		f64::from(layout.zag_amp)
	};
	let lanes = paint.lane_offsets(visual::channel_lane_pitch(stripe_off));
	let normal_pts = visual::offset_polyline(&pts, lanes[0].unwrap_or(0.0));
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
				&normal_pts,
				w,
				rgb(visual::tile_rgb(color)),
				visual::Z_ARC,
			);
		}
		ArcBody::Hitch { from, to } => {
			spawn_hitch_diamond(
				commands,
				meshes,
				materials,
				&normal_pts,
				from,
				to,
				[None; 3],
				[None; 3],
				hitch_r,
				stripe_off,
				f64::from(width),
				braid_amp,
				layout.scale,
			);
		}
		ArcBody::Stripes { a, b, zag_a, zag_b } => {
			let strand_w = width * visual::STRIPE_STRAND_WIDTH_SCALE;
			let zag_w = width * visual::ZAG_STRAND_WIDTH_SCALE;
			let teeth = visual::zag_teeth(arc);
			spawn_stripe_strand(
				commands,
				meshes,
				materials,
				&normal_pts,
				stripe_off,
				zag_a,
				a,
				strand_w,
				zag_w,
				zag_amp,
				teeth,
			);
			spawn_stripe_strand(
				commands,
				meshes,
				materials,
				&normal_pts,
				-stripe_off,
				zag_b,
				b,
				strand_w,
				zag_w,
				zag_amp,
				teeth,
			);
		}
		ArcBody::Hidden => {}
	}
	if let Some(lane) = paint.braid {
		let braid_pts = visual::offset_polyline(&pts, lanes[2].unwrap_or(0.0));
		let strand_w = width * visual::BRAID_STRAND_WIDTH_SCALE;
		match lane {
			BraidLane::Weave { colors } => {
				for thread in
					visual::braid_threads(&braid_pts, braid_amp, visual::braid_turns(arc), colors)
				{
					spawn_braid_strand(
						commands,
						meshes,
						materials,
						&thread.pts,
						&thread.phase,
						strand_w,
						rgb(visual::tile_rgb(thread.color)),
						visual::Z_ARC,
						visual::Z_ARC_OVER,
					);
				}
			}
			BraidLane::Failed { from, to } => {
				spawn_hitch_diamond(
					commands,
					meshes,
					materials,
					&braid_pts,
					[None; 3],
					[None; 3],
					from,
					to,
					hitch_r,
					stripe_off,
					f64::from(width),
					braid_amp,
					layout.scale,
				);
			}
		}
	}
	let zag_w = width * visual::ZAG_STRAND_WIDTH_SCALE;
	let teeth = visual::zag_teeth(arc);
	let spacing = visual::zag_lane_spacing(stripe_off);
	let zag_lane = lanes[1].unwrap_or(0.0);
	let inner = visual::zag_lane_inner(paint.body, stripe_off, hitch_r);
	for (i, zag) in paint.zags_from.iter().enumerate() {
		let Some(color) = *zag else {
			continue;
		};
		let side = zag_lane + visual::zag_fan_offset(i, 1.0, inner, spacing);
		let zig = visual::zigzag_alongside(&pts, side, zag_amp, teeth);
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
	for (i, zag) in paint.zags_to.iter().enumerate() {
		let Some(color) = *zag else {
			continue;
		};
		let side = zag_lane + visual::zag_fan_offset(i, -1.0, inner, spacing);
		let zig = visual::zigzag_alongside(&pts, side, zag_amp, teeth);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&zig,
			zag_w,
			rgb(visual::tile_rgb(color)),
			visual::Z_ARC_OVER + 0.02 * (i + visual::MAX_TILE_ZAGS) as f32,
		);
	}
	let mid_n = paint.zag_mid_count();
	let mid_off = visual::zag_center_offsets(mid_n, spacing);
	for (i, zag) in paint.zags_mid.iter().enumerate() {
		let Some(color) = *zag else {
			continue;
		};
		let zig = visual::zigzag_alongside(&pts, zag_lane + mid_off[i], zag_amp, teeth);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&zig,
			zag_w,
			rgb(visual::tile_rgb(color)),
			visual::Z_ARC_OVER + 0.03 * i as f32,
		);
	}
	spawn_bead_lane(
		commands,
		meshes,
		materials,
		&pts,
		&paint,
		lanes[3].unwrap_or(0.0),
		layout.scale,
		layout.arc_width,
	);
}

fn spawn_bead_lane(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	paint: &visual::ArcPaint,
	lane: f64,
	scale: f32,
	arc_width: f32,
) {
	if !paint.has_bead_lane() {
		return;
	}
	let spacing = visual::ARC_BEAD_SPACING_UNITS * f64::from(scale);
	let bead_r = visual::arc_bead_radius(arc_width);
	let pinch = f64::from(visual::bead_pinch(bead_r));
	let mut z = visual::Z_ARC_OVER + 0.04;
	let push = |commands: &mut Commands,
	            meshes: &mut Assets<Mesh>,
	            materials: &mut Assets<ColorMaterial>,
	            color: crate::game::TileColor,
	            off: f64,
	            z: f32| {
		let line = visual::offset_polyline(pts, lane + off);
		let tris = visual::bead_chain_triangles(&line, spacing, f64::from(bead_r), pinch);
		spawn_filled_tris(commands, meshes, materials, &tris, rgb(visual::tile_rgb(color)), z);
	};
	if let Some(color) = paint.bead_match {
		push(commands, meshes, materials, color, 0.0, z);
		z += 0.01;
	}
	let n = paint.bead_from_count() + paint.bead_to_count() + paint.bead_mid_count();
	let fan = visual::zag_center_offsets(n.max(1), spacing.max(1.0));
	let mut i = 0usize;
	for color in paint
		.beads_from
		.iter()
		.chain(paint.beads_to.iter())
		.chain(paint.beads_mid.iter())
	{
		let Some(color) = *color else {
			continue;
		};
		push(commands, meshes, materials, color, fan.get(i).copied().unwrap_or(0.0), z);
		z += 0.01;
		i += 1;
	}
}

fn spawn_hitch_diamond(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	from: [Option<crate::game::TileColor>; visual::MAX_HITCH_COLORS],
	to: [Option<crate::game::TileColor>; visual::MAX_HITCH_COLORS],
	from_braid: [Option<crate::game::TileColor>; 3],
	to_braid: [Option<crate::game::TileColor>; 3],
	hitch_r: f64,
	stripe_off: f64,
	width: f64,
	braid_amp: f64,
	scale: f32,
) {
	let frame = visual::midpoint_frame(pts);
	if frame.index >= pts.len() {
		return;
	}
	let clearance = visual::hitch_clearance(hitch_r);
	let diamond = visual::conflict_diamond(frame.mid, frame.tangent, frame.normal, hitch_r);
	let from_pts = visual::shorten_polyline_end(&pts[..=frame.index], clearance);
	let to_pts = visual::shorten_polyline_start(&pts[frame.index..], clearance);
	let from_path = visual::diamond_from_path(&from_pts, &diamond);
	let to_path = visual::diamond_to_path(&to_pts, &diamond);
	let amp = visual::hitch_braid_amp(braid_amp, hitch_r);
	let braid_w = width as f32 * visual::BRAID_STRAND_WIDTH_SCALE;
	let from_weave = visual::braid_strand_count(from_braid) >= 2;
	let to_weave = visual::braid_strand_count(to_braid) >= 2;
	if from_weave {
		spawn_hitch_braid(
			commands,
			meshes,
			materials,
			&from_path,
			from_braid,
			amp,
			visual::braid_turns_along(&from_path, scale),
			braid_w,
		);
	}
	if to_weave {
		spawn_hitch_braid(
			commands,
			meshes,
			materials,
			&to_path,
			to_braid,
			amp,
			visual::braid_turns_along(&to_path, scale),
			braid_w,
		);
	}
	let mix = from_weave || to_weave;
	spawn_hitch_half(
		commands,
		meshes,
		materials,
		&from_path,
		from,
		stripe_off,
		width,
		1.0,
		mix,
		amp,
		f64::from(braid_w),
	);
	spawn_hitch_half(
		commands,
		meshes,
		materials,
		&to_path,
		to,
		stripe_off,
		width,
		-1.0,
		mix,
		amp,
		f64::from(braid_w),
	);
}

fn spawn_stripe_strand(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	centerline: &[[f64; 2]],
	offset: f64,
	is_zag: bool,
	color: crate::game::TileColor,
	strand_w: f32,
	zag_w: f32,
	zag_amp: f64,
	teeth: f64,
) {
	if is_zag {
		let zig = visual::zigzag_alongside(centerline, offset, zag_amp, teeth);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&zig,
			zag_w,
			rgb(visual::tile_rgb(color)),
			visual::Z_ARC_OVER,
		);
	} else {
		let pts = visual::offset_polyline(centerline, offset);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&pts,
			strand_w,
			rgb(visual::tile_rgb(color)),
			visual::Z_ARC,
		);
	}
}

fn spawn_hitch_half(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	colors: [Option<crate::game::TileColor>; visual::MAX_HITCH_COLORS],
	stripe_off: f64,
	arc_width: f64,
	outside: f64,
	beside_braid: bool,
	braid_amp: f64,
	braid_w: f64,
) {
	let n = visual::packed_color_count(colors);
	if n == 0 || pts.len() < 2 {
		return;
	}
	let strand_w = if beside_braid {
		visual::highway_strand_width(n, arc_width)
	} else {
		visual::diamond_strand_width(arc_width)
	};
	let offsets = if beside_braid {
		let lane = visual::hitch_leftover_lane(stripe_off, braid_amp, braid_w, strand_w);
		visual::diamond_leftover_offsets(n, lane, outside)
	} else {
		visual::diamond_bundle_offsets(n, stripe_off, outside)
	};
	let strand_w = strand_w as f32;
	let mut k = 0usize;
	for color in colors {
		let Some(c) = color else {
			continue;
		};
		let line = visual::offset_polyline(pts, offsets[k]);
		spawn_stroke(
			commands,
			meshes,
			materials,
			&line,
			strand_w,
			rgb(visual::tile_rgb(c)),
			visual::Z_ARC,
		);
		k += 1;
	}
}

fn spawn_hitch_braid(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	colors: [Option<crate::game::TileColor>; 3],
	amp: f64,
	turns: f64,
	strand_w: f32,
) {
	if visual::braid_strand_count(colors) < 2 || pts.len() < 2 {
		return;
	}
	for thread in visual::braid_threads(pts, amp, turns, colors) {
		spawn_braid_strand(
			commands,
			meshes,
			materials,
			&thread.pts,
			&thread.phase,
			strand_w,
			rgb(visual::tile_rgb(thread.color)),
			visual::Z_ARC,
			visual::Z_ARC_OVER,
		);
	}
}

fn spawn_braid_strand(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	phase: &[f64],
	width: f32,
	color: Color,
	z_back: f32,
	z_front: f32,
) {
	visual::for_braid_segments(pts, phase, |slice, front| {
		let z = if front { z_front } else { z_back };
		spawn_stroke(commands, meshes, materials, slice, width, color, z);
	});
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

fn spawn_stroke_capped(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f64; 2]],
	width: f32,
	color: Color,
	z: f32,
) {
	spawn_stroke(commands, meshes, materials, pts, width, color, z);
	let cap = (width * 0.5).max(0.35);
	for p in [pts.first(), pts.last()].into_iter().flatten() {
		commands.spawn((
			DynamicView,
			Mesh2d(meshes.add(Circle::new(cap))),
			MeshMaterial2d(materials.add(ColorMaterial::from(color))),
			Transform::from_translation(Vec2::new(p[0] as f32, p[1] as f32).extend(z)),
		));
	}
}

fn spawn_filled_poly(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pts: &[[f32; 2]],
	color: Color,
	z: f32,
) {
	if pts.len() < 3 {
		return;
	}
	spawn_filled_tris(commands, meshes, materials, &visual::polygon_triangles(pts), color, z);
}

fn spawn_filled_tris(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	tris: &[[[f32; 2]; 3]],
	color: Color,
	z: f32,
) {
	if tris.is_empty() {
		return;
	}
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(tris_mesh(tris))),
		MeshMaterial2d(materials.add(ColorMaterial::from(color))),
		Transform::from_xyz(0.0, 0.0, z),
	));
}

fn tris_mesh(tris: &[[[f32; 2]; 3]]) -> Mesh {
	let mut positions: Vec<[f32; 3]> = Vec::with_capacity(tris.len() * 3);
	let mut normals: Vec<[f32; 3]> = Vec::with_capacity(tris.len() * 3);
	let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(tris.len() * 3);
	let mut indices: Vec<u32> = Vec::with_capacity(tris.len() * 3);
	for tri in tris {
		let i0 = positions.len() as u32;
		for p in tri {
			positions.push([p[0], p[1], 0.0]);
			normals.push([0.0, 0.0, 1.0]);
			uvs.push([0.0, 0.0]);
		}
		indices.extend_from_slice(&[i0, i0 + 1, i0 + 2]);
	}
	Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
		.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
		.with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
		.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
		.with_inserted_indices(Indices::U32(indices))
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

fn spawn_solid_token(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	player: Player,
	radius: f32,
	stack_index: usize,
	just_captured: bool,
) {
	let fill_z = visual::token_stack_fill_z(stack_index);
	let rim_z = visual::token_stack_rim_z(stack_index);
	let fill = rgb(visual::owner_fill_rgb(player));
	// Color::srgb is alpha 1, so From<Color> already picks Opaque. Named
	// here so a later tint cannot silently flip the stack back to Blend.
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(radius))),
		MeshMaterial2d(materials.add(ColorMaterial::from(fill))),
		Transform::from_translation(pos.extend(fill_z)),
	));
	let rim = (radius * 0.10).max(1.4);
	spawn_ring(
		commands,
		meshes,
		materials,
		pos,
		(radius - rim).max(1.0),
		radius,
		rgb(visual::BLANK_TOKEN_RIM_RGB),
		rim_z,
	);
	if just_captured {
		spawn_capture_glow(commands, meshes, materials, pos, radius);
	}
}

fn spawn_capture_glow(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	radius: f32,
) {
	let scale = (radius / visual::PREV_TILE_RADIUS_PX).max(0.4);
	spawn_ring(
		commands,
		meshes,
		materials,
		pos,
		radius + 1.6 * scale,
		radius + 3.2 * scale,
		rgb(visual::CAPTURE_GLOW_INNER_RGB),
		visual::Z_HIGHLIGHT,
	);
	spawn_ring(
		commands,
		meshes,
		materials,
		pos,
		radius + 3.4 * scale,
		radius + 5.6 * scale,
		rgb(visual::CAPTURE_GLOW_RGB),
		visual::Z_HIGHLIGHT + 0.01,
	);
}

fn spawn_tile_at(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	pos: Vec2,
	tile: Tile,
	highlight: bool,
	just_captured: bool,
	radius: f32,
) {
	let fill = rgb(visual::owner_fill_rgb(tile.owner));
	let primary = rgb(visual::tile_rgb(tile.primary()));
	let z = visual::Z_TILE;
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(radius))),
		MeshMaterial2d(materials.add(ColorMaterial::from(fill))),
		Transform::from_translation(pos.extend(z)),
	));
	let draw = visual::TileRingDraw::for_radius(radius);
	let ring_half = draw.stroke * 0.5;
	let marks = visual::capture_mark_count(tile);
	let down = visual::CAPTURE_DOWN_BEVY;
	let notches = visual::tile_notches(tile, down);
	let strokes = tile.ring_strokes();
	for k in 0..strokes {
		if tile.stroke_is_braid(k) {
			continue;
		}
		let r = draw.ring_radius(radius, k);
		if r > 6.0 * draw.scale {
			let color = tile.stroke_color(k);
			let z = visual::Z_TILE_RING + f32::from(k) * 0.02;
			let center = [f64::from(pos.x), f64::from(pos.y)];
			if tile.stroke_is_zag(k) {
				for zig in visual::zigzag_notched(
					center,
					f64::from(r - ring_half * 0.5),
					f64::from(draw.zag_amp),
					draw.zag_teeth,
					&notches,
				) {
					spawn_stroke_capped(
						commands,
						meshes,
						materials,
						&zig,
						draw.zag_stroke.max(0.45),
						rgb(visual::tile_rgb(color)),
						z,
					);
				}
			} else if tile.stroke_is_bead(k) {
				let bead_r = f64::from(draw.bead_r.max(0.7));
				let pinch = f64::from(visual::bead_pinch(bead_r as f32));
				let rr = f64::from(r - ring_half * 0.5);
				let color = rgb(visual::tile_rgb(color));
				let tris = visual::bead_chain_notched(
					center,
					rr,
					draw.bead_count,
					bead_r,
					pinch,
					visual::TILE_BEAD_SAMPLES,
					&notches,
				);
				spawn_filled_tris(commands, meshes, materials, &tris, color, z);
				let cap = (pinch as f32).max(0.4);
				for rim in
					visual::tile_ring_notched(center, rr, &notches, visual::TILE_BEAD_SAMPLES)
				{
					for p in [rim.first(), rim.last()].into_iter().flatten() {
						commands.spawn((
							DynamicView,
							Mesh2d(meshes.add(Circle::new(cap))),
							MeshMaterial2d(materials.add(ColorMaterial::from(color))),
							Transform::from_translation(
								Vec2::new(p[0] as f32, p[1] as f32).extend(z),
							),
						));
					}
				}
			} else {
				for rim in visual::tile_ring_notched(
					center,
					f64::from(r - ring_half * 0.5),
					&notches,
					visual::TILE_RING_ARC_SAMPLES,
				) {
					spawn_stroke_capped(
						commands,
						meshes,
						materials,
						&rim,
						draw.stroke.max(1.2),
						rgb(visual::tile_rgb(color)),
						z,
					);
				}
			}
		}
	}
	if tile.braid_ring_count() >= 2 {
		let r = f64::from(draw.ring_radius(radius, 0) - ring_half * 0.5);
		if r > 6.0 * f64::from(draw.scale) {
			let center = [f64::from(pos.x), f64::from(pos.y)];
			let amp = f64::from(draw.braid_amp);
			let w = draw.braid_stroke.max(0.28);
			for rim in visual::tile_ring_notched(center, r, &notches, draw.braid_samples) {
				let sweep = visual::polyline_angular_sweep(&rim, center);
				let turns = visual::tile_braid_turns(r) * sweep / (2.0 * std::f64::consts::PI);
				for thread in visual::braid_threads(&rim, amp, turns, tile.braid_colors()) {
					let color = rgb(visual::tile_rgb(thread.color));
					spawn_braid_strand(
						commands,
						meshes,
						materials,
						&thread.pts,
						&thread.phase,
						w,
						color,
						visual::Z_TILE_RING,
						visual::Z_TILE_RING + 0.08,
					);
					let cap = (w * 0.5).max(0.28);
					for p in [thread.pts.first(), thread.pts.last()].into_iter().flatten() {
						commands.spawn((
							DynamicView,
							Mesh2d(meshes.add(Circle::new(cap))),
							MeshMaterial2d(materials.add(ColorMaterial::from(color))),
							Transform::from_translation(
								Vec2::new(p[0] as f32, p[1] as f32)
									.extend(visual::Z_TILE_RING + 0.08),
							),
						));
					}
				}
			}
		}
	}
	for i in 0..marks {
		let triangle = matches!(tile.capture, CaptureMark::Triangle);
		let poly = draw.capture_mark_poly(
			[pos.x, pos.y],
			radius,
			i,
			marks,
			visual::CAPTURE_DOWN_BEVY,
			triangle,
		);
		spawn_filled_poly(commands, meshes, materials, &poly, primary, visual::Z_TILE_DOT);
	}
	let up = [-down[0], -down[1]];
	let glyph = draw.emblem_glyph(tile.emblem, [pos.x, pos.y], radius, up);
	for fill in glyph.fills {
		match fill {
			visual::EmblemFill::Tris(tris) => {
				spawn_filled_tris(commands, meshes, materials, &tris, primary, visual::Z_TILE_DOT);
			}
			visual::EmblemFill::Poly(poly) => {
				spawn_filled_poly(commands, meshes, materials, &poly, primary, visual::Z_TILE_DOT);
			}
		}
	}
	for stroke in glyph.strokes {
		let pts: Vec<[f64; 2]> =
			stroke.pts.iter().map(|p| [f64::from(p[0]), f64::from(p[1])]).collect();
		spawn_stroke_capped(
			commands,
			meshes,
			materials,
			&pts,
			stroke.width.max(0.7),
			primary,
			visual::Z_TILE_DOT,
		);
	}
	if highlight {
		spawn_ring(
			commands,
			meshes,
			materials,
			pos,
			radius + 2.0 * draw.scale,
			radius + 5.0 * draw.scale,
			rgb(visual::HIGHLIGHT_RGB),
			visual::Z_HIGHLIGHT,
		);
	} else if just_captured {
		spawn_capture_glow(commands, meshes, materials, pos, radius);
	}
}

fn spawn_label(commands: &mut Commands, font: &UiFont, pos: Vec2, text: &str) {
	commands.spawn((
		DynamicView,
		Text2d::new(text),
		ui_text_font(font, 12.0),
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
	mut help: Query<
		&mut Text,
		(With<HelpHud>, Without<StatusHud>, Without<TurnHud>, Without<LogHud>, Without<WarnHud>),
	>,
) {
	let g = &play.game;
	let a1 = cfg.bot1.name();
	let a2 = cfg.bot2.name();
	if let Some(mut t) = status.iter_mut().next() {
		*t = Text::new(visual::format_title(a1, a2));
	}
	if let Some(mut t) = turn.iter_mut().next() {
		*t = Text::new(if let Some(session) = play.setup.as_ref() {
			session.prompt()
		} else {
			let mut line = visual::format_turn_line(g, cfg.actor(g.current_player()).name());
			if let Some(ms) = g.remaining_ms(g.current_player()) {
				line.push_str("  clock ");
				line.push_str(&fmt_clock(ms));
			}
			if play.bot_thinking {
				line.push_str("  thinking");
			}
			if let Some(id) = cfg.match_id.as_ref() {
				line.push_str("  game ");
				line.push_str(id);
			}
			line
		});
	}
	if let Some(mut t) = log.iter_mut().next() {
		*t = Text::new(if play.setup.is_some() {
			play.log.clone()
		} else {
			visual::format_log_line(&play.log, &play.last_removals)
		});
	}
	if let Some(mut t) = warn.iter_mut().next() {
		*t = Text::new(play.warning.clone());
	}
	if let Some(mut t) = help.iter_mut().next() {
		*t = Text::new(if play.setup.is_some() {
			visual::SETUP_HELP_TEXT
		} else {
			visual::HELP_TEXT
		});
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

fn fmt_clock(ms: u64) -> String {
	let secs = ms / 1000;
	format!("{}:{:02}", secs / 60, secs % 60)
}

fn view_fingerprint(play: &PlayState, layout: &ViewLayout) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut h = DefaultHasher::new();
	play.game.turn_count().hash(&mut h);
	play.game.current_player().hash(&mut h);
	play.game.extra_convert_active().hash(&mut h);
	play.game.extra_swap_active().hash(&mut h);
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
		play.game.budget(p).hash(&mut h);
		for token in play.game.budget_tokens(p) {
			match token {
				BudgetToken::Blank => 0u8.hash(&mut h),
				BudgetToken::Captured(t) => {
					1u8.hash(&mut h);
					t.id.hash(&mut h);
				}
			}
		}
	}
	if let Some(session) = &play.setup {
		1u8.hash(&mut h);
		session.stage.hash(&mut h);
		session.selected.hash(&mut h);
		session.pick7.hash(&mut h);
		session.extend.hash(&mut h);
	} else {
		0u8.hash(&mut h);
	}
	play.flash.mover.hash(&mut h);
	play.flash.dest.hash(&mut h);
	play.flash.tiles.len().hash(&mut h);
	for t in &play.flash.tiles {
		t.id.hash(&mut h);
	}
	h.finish()
}
