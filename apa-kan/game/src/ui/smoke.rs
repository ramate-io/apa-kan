//! Browser smoke-test hook (web only, opt-in with `?smoke` in the page URL).
//!
//! The canvas has no DOM for a headless browser to query, so when the page is
//! loaded with `?smoke` this publishes a small JSON snapshot on
//! `#bevy-canvas[data-apakan-smoke]` a few times a second:
//!
//! - `screen`: `"shell"` or `"match"`
//! - `buttons`: every UI button with its label and centre in CSS pixels
//! - `match`: turn, status, warning, and `clicks`, the canvas points (CSS
//!   pixels) that make one legal move for the local human, or `[]` when it is
//!   not their turn
//!
//! The smoke driver (`packaging/web/smoke`) clicks those points with real
//! mouse events, so input, picking and rules run exactly as for a player. It
//! never changes game state itself. Without `?smoke` the system does nothing.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use super::{hit_bench, world_v, ConfigRes, PlayState, UiLayout};
use crate::game::{Action, DeckSlot};
use crate::shell::AppScreen;

pub(super) fn plugin(app: &mut App) {
	app.add_systems(Last, publish_snapshot.run_if(enabled));
}

fn enabled() -> bool {
	#[cfg(target_arch = "wasm32")]
	{
		static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
		*ON.get_or_init(|| {
			web_sys::window()
				.and_then(|w| w.location().search().ok())
				.is_some_and(|s| s.contains("smoke"))
		})
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		false
	}
}

#[allow(clippy::too_many_arguments)]
fn publish_snapshot(
	mut frames: Local<u32>,
	screen: Res<State<AppScreen>>,
	buttons: Query<(&Interaction, &ComputedNode, &UiGlobalTransform, &Children)>,
	texts: Query<&Text>,
	play: Res<PlayState>,
	cfg: Res<ConfigRes>,
	layout: Res<UiLayout>,
	cameras: Query<(&Camera, &GlobalTransform)>,
) {
	*frames += 1;
	if *frames % 10 != 0 {
		return;
	}
	let mut out = serde_json::Map::new();
	let screen_name = match screen.get() {
		AppScreen::Shell => "shell",
		AppScreen::Match => "match",
	};
	out.insert("screen".into(), screen_name.into());
	out.insert("frame".into(), (*frames).into());
	let mut list = Vec::new();
	for (_, node, xf, children) in &buttons {
		let label: String = children
			.iter()
			.filter_map(|c| texts.get(c).ok())
			.map(|t| t.0.clone())
			.collect::<Vec<_>>()
			.join(" ");
		let s = node.inverse_scale_factor();
		let c = xf.translation * s;
		let size = node.size() * s;
		if size.x <= 0.0 || size.y <= 0.0 {
			continue;
		}
		list.push(
			serde_json::json!({ "label": label, "x": c.x, "y": c.y, "w": size.x, "h": size.y }),
		);
	}
	out.insert("buttons".into(), list.into());
	if *screen.get() == AppScreen::Match {
		out.insert("match".into(), match_snapshot(&play, &cfg, &layout, &cameras));
	}
	write(&serde_json::Value::Object(out).to_string());
}

fn match_snapshot(
	play: &PlayState,
	cfg: &ConfigRes,
	layout: &UiLayout,
	cameras: &Query<(&Camera, &GlobalTransform)>,
) -> serde_json::Value {
	let status = play.game.status();
	let over = status.is_over();
	let points: Vec<Vec2> = if over { Vec::new() } else { next_clicks(play, cfg, layout) };
	let clicks: Vec<serde_json::Value> = match cameras.iter().next() {
		Some((camera, cam_tf)) => points
			.iter()
			.filter_map(|p| camera.world_to_viewport(cam_tf, p.extend(0.0)).ok())
			.map(|v| serde_json::json!([v.x, v.y]))
			.collect(),
		None => Vec::new(),
	};
	serde_json::json!({
		"setup": play.setup.is_some(),
		"turn": play.game.turn_count(),
		"player": play.game.current_player().label(),
		"over": over,
		"status": format!("{status:?}"),
		"log": play.log,
		"warning": play.warning,
		"selection": format!("{:?}", play.selection),
		"clicks": clicks,
	})
}

/// World points that make one legal action for the local human right now.
fn next_clicks(play: &PlayState, cfg: &ConfigRes, layout: &UiLayout) -> Vec<Vec2> {
	if let Some(session) = play.setup.as_ref() {
		let human = match session.actor_player() {
			Some(p) => cfg.actor(p).is_human(),
			None => true,
		};
		if !human {
			return Vec::new();
		}
		let confirm = || {
			let p = layout.setup_confirm_pos();
			vec![Vec2::new(p[0], p[1])]
		};
		if session.can_confirm() {
			return confirm();
		}
		let len = session.pool_tiles().len();
		for i in 0..len {
			if session.selected.contains(&i) {
				continue;
			}
			let mut probe = session.clone();
			if probe.toggle(i).is_ok() {
				let p = layout.setup_grid_pos(i, len);
				return vec![Vec2::new(p[0], p[1])];
			}
		}
		return Vec::new();
	}
	let current = play.game.current_player();
	if !cfg.actor(current).is_human() {
		return Vec::new();
	}
	if play.selection != super::Selection::None {
		// Clear a half-made selection first (clicking empty space resets it).
		return Vec::new();
	}
	let node_pos = |id| world_v(layout, play.game.board().position(id));
	for action in play.game.legal_actions() {
		match action {
			Action::Move { from, to } => return vec![node_pos(from), node_pos(to)],
			Action::Add { bench_index, node } => {
				let Some(tile) = play.game.bench(current).get(bench_index).copied() else {
					continue;
				};
				let deck = play.game.deck_slots(current);
				let n = deck.len();
				let Some(slot) =
					deck.iter().position(|s| matches!(s, DeckSlot::Live(t) if t.id == tile.id))
				else {
					continue;
				};
				let p = layout.bench_pos(current, slot, n);
				let bench = Vec2::new(p[0], p[1]);
				if hit_bench(&play.game, layout, current, bench) != Some(bench_index) {
					continue;
				}
				return vec![bench, node_pos(node)];
			}
			_ => {}
		}
	}
	Vec::new()
}

fn write(json: &str) {
	#[cfg(target_arch = "wasm32")]
	{
		if let Some(el) = web_sys::window()
			.and_then(|w| w.document())
			.and_then(|d| d.get_element_by_id("bevy-canvas"))
		{
			let _ = el.set_attribute("data-apakan-smoke", json);
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = json;
	}
}
