//! Bevy home, Head-to-Head lobby, and stub pages. No Dioxus chrome.
//!
//! Buttons follow issue #50. Tile/deck options that are not in core yet are
//! labelled Coming Soon rather than invented.

use bevy::prelude::*;

use crate::ai::Actor;
use crate::clock::TimeControl;
use crate::config::GameConfig;
use crate::divisions::{Division, Ladder, Outcome};
use crate::host::HostConfig;
use crate::platform::{open_url, replace_browser_route, IncomingUrl};
use crate::route::{DeepLink, Route};
use crate::ui::{ui_text_font, ui_title_font, UiFont};

/// Resolved deploy host (public URL, API URL, external links). See [`crate::host`].
#[derive(Resource, Clone, Debug, Default)]
pub struct HostRes(pub HostConfig);

/// The offline Divisions match in flight, if any. Set by Play, cleared when the
/// result is recorded or the player leaves.
#[derive(Resource, Clone, Debug, Default)]
pub struct DivisionMatch {
	pub pending: Option<Division>,
}

const BG: Color = Color::srgb(0.957, 0.933, 0.886); // #f4eee2
const BG_ALT: Color = Color::srgb(0.929, 0.898, 0.831); // #ede5d4
const PAPER: Color = Color::srgb(0.984, 0.969, 0.925); // #fbf7ec
const INK: Color = Color::srgb(0.102, 0.102, 0.102); // #1a1a1a
const MUTED: Color = Color::srgb(0.365, 0.341, 0.290); // #5d574a
const PURPLE: Color = Color::srgb(0.431, 0.345, 0.514); // #6e5883
const GREEN: Color = Color::srgb(0.239, 0.400, 0.322); // #3d6652
const ORANGE: Color = Color::srgb(0.639, 0.361, 0.227); // #a35c3a

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppScreen {
	#[default]
	Shell,
	Match,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShellPage {
	#[default]
	Home,
	HeadToHead,
	Puzzles,
	Market,
	Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum OpponentKind {
	#[default]
	Local,
	Bot,
	Online,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum TileKind {
	#[default]
	Iye,
	Breakwalls,
	Fences,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum DeckKind {
	#[default]
	PartHand,
	Blind,
	Randoms,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum RewardKind {
	#[default]
	Casual,
	Divisions,
	Wagers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum ClockKind {
	Unclocked,
	#[default]
	FiveMin,
	TwoPlusOne,
}

#[derive(Resource, Clone)]
pub struct ShellState {
	page: ShellPage,
	opponent: OpponentKind,
	bot: Actor,
	tile: TileKind,
	deck: DeckKind,
	reward: RewardKind,
	clock: ClockKind,
	notice: String,
	ladder: Ladder,
}

impl Default for ShellState {
	fn default() -> Self {
		Self {
			page: ShellPage::Home,
			opponent: OpponentKind::Bot,
			bot: Actor::Greedy,
			tile: TileKind::Iye,
			deck: DeckKind::PartHand,
			reward: RewardKind::Casual,
			clock: ClockKind::FiveMin,
			notice: String::new(),
			ladder: Ladder::default(),
		}
	}
}

#[derive(Message, Clone)]
pub struct StartMatch {
	pub cfg: GameConfig,
}

#[derive(Message, Clone, Copy)]
pub struct LeaveMatch;

#[derive(Component)]
struct ShellRoot;

#[derive(Component, Clone, Copy)]
enum ShellBtn {
	Page(ShellPage),
	Opponent(OpponentKind),
	Bot(Actor),
	Tile(TileKind),
	Deck(DeckKind),
	Reward(RewardKind),
	Clock(ClockKind),
	Play,
	OpenMarket,
	OpenLegal,
	OpenPay,
	Account(AccountAction),
}

/// Settings → account buttons ([`crate::keyguard`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccountAction {
	Protect,
	Unlock,
	RestorePasskey,
	ShowPhrase,
	RestorePhrase,
}

pub fn plugin(app: &mut App) {
	app.insert_resource(ShellState { ladder: Ladder::load(), ..ShellState::default() })
		.init_resource::<HostRes>()
		.init_resource::<DivisionMatch>()
		.add_message::<StartMatch>()
		.add_message::<LeaveMatch>()
		.add_systems(OnEnter(AppScreen::Shell), spawn_root)
		.add_systems(OnExit(AppScreen::Shell), despawn_root)
		.add_systems(
			Update,
			(rebuild_if_needed, handle_clicks, apply_deep_links).run_if(in_state(AppScreen::Shell)),
		)
		.add_systems(Update, apply_keyguard_notices);
}

fn spawn_root(
	mut commands: Commands,
	shell: Res<ShellState>,
	host: Res<HostRes>,
	font: Res<UiFont>,
) {
	build_page(&mut commands, &shell, &host.0, &font);
}

fn despawn_root(mut commands: Commands, roots: Query<Entity, With<ShellRoot>>) {
	for e in &roots {
		commands.entity(e).despawn();
	}
}

fn rebuild_if_needed(
	mut commands: Commands,
	shell: Res<ShellState>,
	host: Res<HostRes>,
	font: Res<UiFont>,
	roots: Query<Entity, With<ShellRoot>>,
	mut fingerprint: Local<u64>,
) {
	let fp = shell_fingerprint(&shell);
	if *fingerprint == fp && roots.iter().next().is_some() {
		return;
	}
	*fingerprint = fp;
	for e in &roots {
		commands.entity(e).despawn();
	}
	build_page(&mut commands, &shell, &host.0, &font);
}

fn shell_fingerprint(shell: &ShellState) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut h = std::collections::hash_map::DefaultHasher::new();
	(shell.page as u8).hash(&mut h);
	(shell.opponent as u8).hash(&mut h);
	shell.bot.name().hash(&mut h);
	(shell.tile as u8).hash(&mut h);
	(shell.deck as u8).hash(&mut h);
	(shell.reward as u8).hash(&mut h);
	(shell.clock as u8).hash(&mut h);
	shell.notice.hash(&mut h);
	shell.ladder.elo.hash(&mut h);
	shell.ladder.games().hash(&mut h);
	crate::keyguard::revision().hash(&mut h);
	h.finish()
}

fn root_node() -> Node {
	Node {
		width: Val::Percent(100.0),
		height: Val::Percent(100.0),
		flex_direction: FlexDirection::Column,
		align_items: AlignItems::Center,
		justify_content: JustifyContent::FlexStart,
		padding: UiRect::all(Val::Px(28.0)),
		row_gap: Val::Px(12.0),
		overflow: Overflow::scroll_y(),
		..default()
	}
}

fn build_page(commands: &mut Commands, shell: &ShellState, host: &HostConfig, font: &UiFont) {
	let root = commands.spawn((ShellRoot, root_node(), BackgroundColor(BG))).id();
	match shell.page {
		ShellPage::Home => fill_home(commands, root, shell, font),
		ShellPage::HeadToHead => fill_h2h(commands, root, shell, font),
		ShellPage::Puzzles => fill_stub(
			commands,
			root,
			font,
			"Puzzles",
			"Coming Soon. Chapter rewards are a later grant path; there is no puzzle runtime in this phase.",
			&[],
		),
		ShellPage::Market => fill_stub(
			commands,
			root,
			font,
			"Market",
			"Listings, sleeves, and purchases live outside the match view. Opens a new tab — no in-app browser.",
			&[
				(ShellBtn::OpenMarket, "Open market in a new tab"),
				(ShellBtn::OpenPay, "Open payment in a new tab"),
			],
		),
		ShellPage::Settings => {
			let mut extra = account_buttons();
			extra.push((ShellBtn::OpenLegal, "Open license in a new tab"));
			fill_stub(
				commands,
				root,
				font,
				"Settings",
				&format!(
					"{}\n\n{}\nDivisions: {}\nThe source-available license opens in a new tab.",
					crate::keyguard::status_line(),
					host.summary(),
					shell.ladder.summary()
				),
				&extra,
			);
			let notice = spawn_notice(commands, font, &shell.notice);
			commands.entity(root).insert_children(1, &[notice]);
		}
	}
}

/// Account buttons for the current identity state.
fn account_buttons() -> Vec<(ShellBtn, &'static str)> {
	use crate::identity::{self, Protection};
	let passkeys = crate::keyguard::passkey_supported();
	let mut out = Vec::new();
	if identity::is_locked() {
		out.push((ShellBtn::Account(AccountAction::Unlock), "Unlock with passkey"));
	} else {
		if passkeys && identity::protection() == Protection::Plain {
			out.push((ShellBtn::Account(AccountAction::Protect), "Protect with passkey"));
		}
		out.push((ShellBtn::Account(AccountAction::ShowPhrase), "Show recovery phrase"));
	}
	if passkeys {
		out.push((ShellBtn::Account(AccountAction::RestorePasskey), "Restore with passkey"));
	}
	if cfg!(target_arch = "wasm32") {
		out.push((ShellBtn::Account(AccountAction::RestorePhrase), "Restore from recovery phrase"));
	}
	out
}

fn apply_keyguard_notices(mut shell: ResMut<ShellState>) {
	if let Some(msg) = crate::keyguard::drain_notices().pop() {
		shell.notice = msg;
	}
}

fn fill_home(commands: &mut Commands, root: Entity, shell: &ShellState, font: &UiFont) {
	let mut children = Vec::new();
	children.push(spawn_title(
		commands,
		font,
		"Apa Kan",
		"A turn-based game of colors and adjacency.",
	));
	if !shell.notice.is_empty() {
		children.push(spawn_notice(commands, font, &shell.notice));
	}
	children.push(spawn_full_btn(
		commands,
		font,
		"Head-to-Head",
		ShellBtn::Page(ShellPage::HeadToHead),
		PURPLE,
		true,
	));
	children.push(spawn_full_btn(
		commands,
		font,
		"Puzzles — Coming Soon",
		ShellBtn::Page(ShellPage::Puzzles),
		BG_ALT,
		false,
	));
	children.push(spawn_full_btn(
		commands,
		font,
		"Market",
		ShellBtn::Page(ShellPage::Market),
		GREEN,
		true,
	));
	children.push(spawn_full_btn(
		commands,
		font,
		"Settings",
		ShellBtn::Page(ShellPage::Settings),
		ORANGE,
		true,
	));
	commands.entity(root).add_children(&children);
}

fn fill_h2h(commands: &mut Commands, root: Entity, shell: &ShellState, font: &UiFont) {
	let mut children = Vec::new();
	children.push(spawn_title(
		commands,
		font,
		"Head-to-Head",
		"Pick axes, then play locally. Online and economy options stay visible but flagged Coming Soon.",
	));
	push_axis(
		commands,
		font,
		&mut children,
		"Tile option",
		&[
			("Iye", ShellBtn::Tile(TileKind::Iye), shell.tile == TileKind::Iye),
			("Breakwalls — soon", ShellBtn::Tile(TileKind::Breakwalls), false),
			("Fences — soon", ShellBtn::Tile(TileKind::Fences), false),
		],
	);
	push_axis(
		commands,
		font,
		&mut children,
		"Deck option",
		&[
			("Part-hand", ShellBtn::Deck(DeckKind::PartHand), shell.deck == DeckKind::PartHand),
			("Blind — soon", ShellBtn::Deck(DeckKind::Blind), false),
			("Randoms — soon", ShellBtn::Deck(DeckKind::Randoms), false),
		],
	);
	push_axis(
		commands,
		font,
		&mut children,
		"Reward",
		&[
			("Casual", ShellBtn::Reward(RewardKind::Casual), shell.reward == RewardKind::Casual),
			(
				"Divisions (offline)",
				ShellBtn::Reward(RewardKind::Divisions),
				shell.reward == RewardKind::Divisions,
			),
			("Wagers — soon", ShellBtn::Reward(RewardKind::Wagers), false),
		],
	);
	if shell.reward == RewardKind::Divisions {
		children.push(spawn_caption(commands, font, "Division"));
		children.push(spawn_notice_colored(commands, font, &shell.ladder.summary(), INK));
	} else {
		push_opponent_axes(commands, font, &mut children, shell);
	}
	push_axis(
		commands,
		font,
		&mut children,
		"Time control",
		&[
			(
				"Unclocked",
				ShellBtn::Clock(ClockKind::Unclocked),
				shell.clock == ClockKind::Unclocked,
			),
			("5-minute", ShellBtn::Clock(ClockKind::FiveMin), shell.clock == ClockKind::FiveMin),
			("2 + 1", ShellBtn::Clock(ClockKind::TwoPlusOne), shell.clock == ClockKind::TwoPlusOne),
		],
	);
	children.push(spawn_notice(commands, font, &shell.notice));
	children.push(spawn_full_btn(commands, font, "Play", ShellBtn::Play, GREEN, true));
	children.push(spawn_full_btn(
		commands,
		font,
		"Back",
		ShellBtn::Page(ShellPage::Home),
		BG_ALT,
		false,
	));
	commands.entity(root).add_children(&children);
}

fn push_opponent_axes(
	commands: &mut Commands,
	font: &UiFont,
	out: &mut Vec<Entity>,
	shell: &ShellState,
) {
	push_axis(
		commands,
		font,
		out,
		"Opponent",
		&[
			(
				"Local",
				ShellBtn::Opponent(OpponentKind::Local),
				shell.opponent == OpponentKind::Local,
			),
			("Bot", ShellBtn::Opponent(OpponentKind::Bot), shell.opponent == OpponentKind::Bot),
			(
				"Online (invite link)",
				ShellBtn::Opponent(OpponentKind::Online),
				shell.opponent == OpponentKind::Online,
			),
		],
	);
	push_axis(
		commands,
		font,
		out,
		"Bot",
		&[
			("greedy", ShellBtn::Bot(Actor::Greedy), shell.bot == Actor::Greedy),
			("minimax", ShellBtn::Bot(Actor::Minimax), shell.bot == Actor::Minimax),
			("mcts", ShellBtn::Bot(Actor::Mcts), shell.bot == Actor::Mcts),
		],
	);
}

fn fill_stub(
	commands: &mut Commands,
	root: Entity,
	font: &UiFont,
	title: &str,
	body: &str,
	extra: &[(ShellBtn, &str)],
) {
	let mut children = Vec::new();
	children.push(spawn_title(commands, font, title, body));
	for (btn, label) in extra {
		children.push(spawn_full_btn(commands, font, label, *btn, PURPLE, true));
	}
	children.push(spawn_full_btn(
		commands,
		font,
		"Back",
		ShellBtn::Page(ShellPage::Home),
		BG_ALT,
		false,
	));
	commands.entity(root).add_children(&children);
}

fn push_axis(
	commands: &mut Commands,
	font: &UiFont,
	out: &mut Vec<Entity>,
	caption: &str,
	items: &[(&str, ShellBtn, bool)],
) {
	out.push(spawn_caption(commands, font, caption));
	out.push(
		commands
			.spawn(Node {
				width: Val::Percent(100.0),
				max_width: Val::Px(640.0),
				flex_direction: FlexDirection::Row,
				column_gap: Val::Px(8.0),
				flex_wrap: FlexWrap::Wrap,
				..default()
			})
			.with_children(|parent| {
				for (label, action, on) in items {
					spawn_chip(parent, font, label, *action, *on);
				}
			})
			.id(),
	);
}

fn spawn_title(commands: &mut Commands, font: &UiFont, title: &str, sub: &str) -> Entity {
	let title_font = ui_title_font(font, 42.0);
	let sub_font = ui_text_font(font, 16.0);
	commands
		.spawn(Node {
			width: Val::Percent(100.0),
			max_width: Val::Px(640.0),
			flex_direction: FlexDirection::Column,
			row_gap: Val::Px(6.0),
			margin: UiRect::bottom(Val::Px(8.0)),
			..default()
		})
		.with_children(|parent| {
			parent.spawn((Text::new(title.to_string()), title_font, TextColor(INK)));
			parent.spawn((Text::new(sub.to_string()), sub_font, TextColor(MUTED)));
		})
		.id()
}

fn spawn_caption(commands: &mut Commands, font: &UiFont, text: &str) -> Entity {
	commands
		.spawn((
			Text::new(text.to_string()),
			ui_text_font(font, 14.0),
			TextColor(MUTED),
			Node {
				width: Val::Percent(100.0),
				max_width: Val::Px(640.0),
				margin: UiRect::top(Val::Px(8.0)),
				..default()
			},
		))
		.id()
}

fn spawn_notice(commands: &mut Commands, font: &UiFont, text: &str) -> Entity {
	spawn_notice_colored(commands, font, text, if text.is_empty() { MUTED } else { ORANGE })
}

fn spawn_notice_colored(
	commands: &mut Commands,
	font: &UiFont,
	text: &str,
	color: Color,
) -> Entity {
	commands
		.spawn((
			Text::new(text.to_string()),
			ui_text_font(font, 15.0),
			TextColor(color),
			Node {
				width: Val::Percent(100.0),
				max_width: Val::Px(640.0),
				min_height: Val::Px(22.0),
				..default()
			},
		))
		.id()
}

fn spawn_full_btn(
	commands: &mut Commands,
	font: &UiFont,
	label: &str,
	action: ShellBtn,
	fill: Color,
	light_text: bool,
) -> Entity {
	let color = if light_text { PAPER } else { INK };
	let text_font = ui_title_font(font, 20.0);
	commands
		.spawn((
			Button,
			action,
			Node {
				width: Val::Percent(100.0),
				max_width: Val::Px(640.0),
				min_height: Val::Px(52.0),
				padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
				justify_content: JustifyContent::Center,
				align_items: AlignItems::Center,
				border_radius: BorderRadius::all(Val::Px(4.0)),
				..default()
			},
			BackgroundColor(fill),
		))
		.with_children(|parent| {
			parent.spawn((Text::new(label.to_string()), text_font, TextColor(color)));
		})
		.id()
}

fn spawn_chip(
	parent: &mut ChildSpawnerCommands,
	font: &UiFont,
	label: &str,
	action: ShellBtn,
	on: bool,
) {
	parent
		.spawn((
			Button,
			action,
			Node {
				flex_grow: 1.0,
				min_height: Val::Px(40.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
				justify_content: JustifyContent::Center,
				align_items: AlignItems::Center,
				border_radius: BorderRadius::all(Val::Px(4.0)),
				..default()
			},
			BackgroundColor(if on { PURPLE } else { BG_ALT }),
		))
		.with_children(|parent| {
			parent.spawn((
				Text::new(label.to_string()),
				ui_text_font(font, 15.0),
				TextColor(if on { PAPER } else { INK }),
			));
		});
}

fn handle_clicks(
	mut shell: ResMut<ShellState>,
	mut start: MessageWriter<StartMatch>,
	base: Res<crate::ui::ConfigRes>,
	host: Res<HostRes>,
	mut division: ResMut<DivisionMatch>,
	mut interaction: Query<(&Interaction, &ShellBtn), Changed<Interaction>>,
) {
	for (interaction, btn) in &mut interaction {
		if *interaction != Interaction::Pressed {
			continue;
		}
		match *btn {
			ShellBtn::Page(page) => {
				shell.page = page;
				shell.notice.clear();
				replace_browser_route(&page_route(page));
			}
			ShellBtn::Opponent(OpponentKind::Online) => {
				if online_available(&host.0) {
					shell.opponent = OpponentKind::Online;
					shell.notice.clear();
				} else {
					shell.notice = OFFLINE_NOTICE.into();
				}
			}
			ShellBtn::Opponent(kind) => {
				shell.opponent = kind;
				shell.notice.clear();
			}
			ShellBtn::Bot(actor) => {
				shell.bot = actor;
				if shell.opponent != OpponentKind::Bot {
					shell.opponent = OpponentKind::Bot;
				}
				shell.notice.clear();
			}
			ShellBtn::Tile(TileKind::Iye) => {
				shell.tile = TileKind::Iye;
				shell.notice.clear();
			}
			ShellBtn::Tile(_) => {
				shell.notice =
					"Breakwalls and Fences are core transforms in a later milestone.".into();
			}
			ShellBtn::Deck(DeckKind::PartHand) => {
				shell.deck = DeckKind::PartHand;
				shell.notice.clear();
			}
			ShellBtn::Deck(_) => {
				shell.notice = "Blind and Randoms deck procedures are not in this build.".into();
			}
			ShellBtn::Reward(kind @ (RewardKind::Casual | RewardKind::Divisions)) => {
				shell.reward = kind;
				shell.notice.clear();
			}
			ShellBtn::Reward(RewardKind::Wagers) => {
				shell.notice = "Wagers wait on ratings and the market.".into();
			}
			ShellBtn::Clock(kind) => {
				shell.clock = kind;
				shell.notice.clear();
			}
			ShellBtn::Account(action) => {
				let msg = match action {
					AccountAction::Protect => crate::keyguard::protect(&host.0),
					AccountAction::Unlock => crate::keyguard::unlock(&host.0),
					AccountAction::RestorePasskey => crate::keyguard::restore_with_passkey(&host.0),
					AccountAction::ShowPhrase => crate::keyguard::show_phrase(),
					AccountAction::RestorePhrase => crate::keyguard::restore_from_phrase(&host.0),
				};
				if !msg.is_empty() {
					shell.notice = msg;
				}
			}
			ShellBtn::Play
				if shell.opponent == OpponentKind::Online
					&& shell.reward == RewardKind::Casual
					&& crate::identity::is_locked() =>
			{
				shell.notice = "Account locked: Settings → Unlock with passkey.".into();
			}
			ShellBtn::Play
				if shell.opponent == OpponentKind::Online && shell.reward == RewardKind::Casual =>
			{
				if crate::remote::request_create(&host.0) {
					division.pending = None;
					shell.notice =
						"Creating a hosted game… the invite link is copied when it is ready."
							.into();
				} else {
					shell.notice = OFFLINE_NOTICE.into();
				}
			}
			ShellBtn::Play => match start_cfg(&shell, &base.0) {
				Ok((cfg, div)) => {
					division.pending = div;
					replace_browser_route(&Route::HeadToHead);
					start.write(StartMatch { cfg });
				}
				Err(msg) => shell.notice = msg,
			},
			ShellBtn::OpenMarket => {
				let _ = open_url(&host.0.market_url);
			}
			ShellBtn::OpenLegal => {
				let _ = open_url(&host.0.legal_url);
			}
			ShellBtn::OpenPay => {
				let _ = open_url(&host.0.payment_url);
			}
		}
	}
}

const OFFLINE_NOTICE: &str =
	"Online needs a server: set APAKAN_API_URL (or the apakan:api-url meta tag).";

fn online_available(host: &HostConfig) -> bool {
	host.api_url.is_some() && crate::remote::HTTP_AVAILABLE
}

impl ShellState {
	/// Show a message on the lobby (used by server replies).
	pub fn set_notice(&mut self, msg: impl Into<String>) {
		self.notice = msg.into();
	}
}

fn page_route(page: ShellPage) -> Route {
	match page {
		ShellPage::Home => Route::Home,
		ShellPage::HeadToHead => Route::HeadToHead,
		ShellPage::Puzzles => Route::Puzzles,
		ShellPage::Market => Route::Market,
		ShellPage::Settings => Route::Settings,
	}
}

/// Match config for the lobby selection, plus the division being played when
/// Reward is Divisions.
fn start_cfg(
	shell: &ShellState,
	base: &GameConfig,
) -> Result<(GameConfig, Option<Division>), String> {
	if shell.tile != TileKind::Iye {
		return Err("Only Iye is playable in this build.".into());
	}
	if shell.deck != DeckKind::PartHand {
		return Err("Only Part-hand is playable in this build.".into());
	}
	if shell.reward == RewardKind::Wagers {
		return Err("Wagers wait on ratings and the market.".into());
	}
	let division = (shell.reward == RewardKind::Divisions).then(|| shell.ladder.division());
	if division.is_none() && shell.opponent == OpponentKind::Online {
		return Err(OFFLINE_NOTICE.into());
	}
	let mut cfg = base.clone();
	cfg.show_shell = false;
	cfg.bot1 = Actor::Human;
	cfg.bot2 = match (division, shell.opponent) {
		(Some(d), _) => d.bot,
		(None, OpponentKind::Local | OpponentKind::Online) => Actor::Human,
		(None, OpponentKind::Bot) => shell.bot,
	};
	if let Some(d) = division {
		cfg.think = d.think();
		cfg.seed = rand::random();
		cfg.setup.deck_seed = Some(cfg.seed);
		cfg.match_id = None;
	}
	cfg.time_control = match shell.clock {
		ClockKind::Unclocked => None,
		ClockKind::FiveMin => Some(TimeControl::FiveMin),
		ClockKind::TwoPlusOne => Some(TimeControl::TwoPlusOne),
	};
	Ok((cfg, division))
}

/// Outcome for the human seat (P1) once the game is over.
pub fn division_outcome(status: crate::game::GameStatus) -> Option<Outcome> {
	use crate::game::{GameStatus, Player};
	match status {
		GameStatus::InProgress => None,
		GameStatus::Won { winner: Player::One, .. } => Some(Outcome::Win),
		GameStatus::Won { .. } => Some(Outcome::Loss),
		GameStatus::Draw { .. } => Some(Outcome::Draw),
	}
}

/// Record a finished Divisions match on the local ladder and save it.
/// Returns the HUD/lobby message.
pub fn record_division(shell: &mut ShellState, outcome: Outcome) -> String {
	let update = shell.ladder.record(outcome);
	let mut msg = update.message();
	if let Err(err) = shell.ladder.save() {
		msg.push_str(&format!(" (not saved: {err})"));
	}
	shell.notice = msg.clone();
	msg
}

pub fn apply_link_to_shell(shell: &mut ShellState, link: &DeepLink) {
	match &link.route {
		Route::Home => shell.page = ShellPage::Home,
		Route::HeadToHead => {
			shell.page = ShellPage::HeadToHead;
			if let Some(v) = link.query.get("opponent") {
				match v.as_str() {
					"local" => shell.opponent = OpponentKind::Local,
					"bot" => shell.opponent = OpponentKind::Bot,
					"online" => {
						shell.notice = "Online matchmaking is not in this web bring-up.".into();
					}
					_ => {}
				}
			}
			if let Some(v) = link.query.get("bot") {
				if let Ok(actor) = Actor::parse(v) {
					if !actor.is_human() {
						shell.bot = actor;
						shell.opponent = OpponentKind::Bot;
					}
				}
			}
			if let Some(v) = link.query.get("reward") {
				match v.to_ascii_lowercase().as_str() {
					"casual" => shell.reward = RewardKind::Casual,
					"divisions" | "division" => shell.reward = RewardKind::Divisions,
					_ => {}
				}
			}
			if let Some(v) = link.query.get("time") {
				match TimeControl::parse(v) {
					Ok(TimeControl::FiveMin) => shell.clock = ClockKind::FiveMin,
					Ok(TimeControl::TwoPlusOne) => shell.clock = ClockKind::TwoPlusOne,
					Err(_) if v.eq_ignore_ascii_case("off") => {
						shell.clock = ClockKind::Unclocked;
					}
					Err(_) => {}
				}
			}
		}
		Route::Puzzles => shell.page = ShellPage::Puzzles,
		Route::Market => shell.page = ShellPage::Market,
		Route::Settings => shell.page = ShellPage::Settings,
		Route::Game { .. } => {}
	}
}

fn apply_deep_links(mut shell: ResMut<ShellState>, mut links: MessageReader<IncomingUrl>) {
	for IncomingUrl { uri } in links.read() {
		let link = DeepLink::parse(uri);
		if matches!(link.route, Route::Game { .. }) {
			continue;
		}
		apply_link_to_shell(&mut shell, &link);
	}
}

/// Seed a local match from `/game/<id>` until the server hosts real ids.
pub fn seed_from_match_id(id: &str) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut h = std::collections::hash_map::DefaultHasher::new();
	id.hash(&mut h);
	let n = h.finish();
	if n == 0 {
		1
	} else {
		n
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;
	use crate::game::{EndReason, GameStatus, Player};

	#[test]
	fn divisions_play_uses_the_division_bot_and_budget() {
		let shell = ShellState {
			reward: RewardKind::Divisions,
			opponent: OpponentKind::Local,
			..ShellState::default()
		};
		let (cfg, div) = start_cfg(&shell, &GameConfig::default()).unwrap();
		let div = div.unwrap();
		assert_eq!(div, shell.ladder.division());
		assert!(cfg.bot1.is_human());
		assert_eq!(cfg.bot2, div.bot);
		assert_eq!(cfg.think, div.think());
	}

	#[test]
	fn casual_play_has_no_division() {
		let shell = ShellState::default();
		let (cfg, div) = start_cfg(&shell, &GameConfig::default()).unwrap();
		assert!(div.is_none());
		assert_eq!(cfg.bot2, shell.bot);
		let wagers = ShellState { reward: RewardKind::Wagers, ..ShellState::default() };
		assert!(start_cfg(&wagers, &GameConfig::default()).is_err());
	}

	#[test]
	fn outcome_is_from_the_human_seat() {
		assert_eq!(division_outcome(GameStatus::InProgress), None);
		assert_eq!(
			division_outcome(GameStatus::Won {
				winner: Player::One,
				reason: EndReason::DoublePass
			}),
			Some(Outcome::Win)
		);
		assert_eq!(
			division_outcome(GameStatus::Won {
				winner: Player::Two,
				reason: EndReason::DoublePass
			}),
			Some(Outcome::Loss)
		);
		assert_eq!(
			division_outcome(GameStatus::Draw { reason: EndReason::DoublePass }),
			Some(Outcome::Draw)
		);
	}

	#[test]
	fn reward_deep_link() {
		let mut shell = ShellState::default();
		apply_link_to_shell(&mut shell, &DeepLink::parse("#/h2h?reward=divisions"));
		assert_eq!(shell.page, ShellPage::HeadToHead);
		assert_eq!(shell.reward, RewardKind::Divisions);
	}
}
