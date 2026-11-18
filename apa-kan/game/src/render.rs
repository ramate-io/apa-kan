//! Headless PNG renderer for the geometric board, tiles, and both benches.

use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::board::NodeId;
use crate::game::{BudgetToken, CaptureDest, DeckSlot, Game, Player, Tile};
use crate::visual::{self, ArcBody, BraidLane, CaptureFlash, ViewLayout};

pub fn write_png(game: &Game, path: impl AsRef<Path>, caption: &str) -> Result<(), String> {
	write_png_sized(game, path, caption, 1280, 960)
}

pub fn write_png_sized(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
) -> Result<(), String> {
	write_png_view(game, path, caption, width, height, None)
}

pub fn write_png_view(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
	selected: Option<NodeId>,
) -> Result<(), String> {
	write_png_view_flash(game, path, caption, width, height, selected, None)
}

pub fn write_png_view_flash(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
	selected: Option<NodeId>,
	flash: Option<&CaptureFlash>,
) -> Result<(), String> {
	write_png_view_preview(game, path, caption, width, height, selected, flash, &[])
}

/// Preview-only board PNG with character medallions in tile centers.
/// Generation and the live painter do not call this.
pub fn write_png_view_preview(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
	selected: Option<NodeId>,
	flash: Option<&CaptureFlash>,
	characters: &[(NodeId, &RgbaImage)],
) -> Result<(), String> {
	let layout = ViewLayout::from_window_with_deck(
		width as f32,
		height as f32,
		game.rules().bag_size as usize,
	);
	let bg = visual::rgb8(visual::BG_RGB);
	let mut img = RgbaImage::from_pixel(width, height, rgba(bg[0], bg[1], bg[2], 255));
	draw_arcs(&mut img, game, &layout);
	draw_nodes(&mut img, game, &layout);
	draw_tiles_preview(&mut img, game, &layout, characters);
	draw_marks(&mut img, game, &layout, selected);
	draw_benches(&mut img, game, &layout, flash);
	draw_flash(&mut img, &layout, flash);
	draw_hud(&mut img, game, &layout, caption);
	img.save(path.as_ref()).map_err(|e| e.to_string())
}

fn draw_arcs(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	let mut back: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	let mut front: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	for arc in board.arcs() {
		let samples = arc.sample(visual::ARC_SAMPLES);
		let pts: Vec<[f64; 2]> = samples.iter().map(|p| image_pt(layout, *p)).collect();
		let width = f64::from(layout.arc_width);
		let paint = visual::arc_paint_with(
			game.tile_at(arc.from),
			game.tile_at(arc.to),
			game.rules().zag_mismatch,
		);
		let stripe_off = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.stripe_off_center)
		} else {
			f64::from(layout.stripe_off)
		};
		let hitch_r = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.hitch_r_center)
		} else {
			f64::from(layout.hitch_r)
		};
		let braid_amp = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.braid_amp_center)
		} else {
			f64::from(layout.braid_amp)
		};
		let zag_amp = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.zag_amp_center)
		} else {
			f64::from(layout.zag_amp)
		};
		let lanes = paint.lane_offsets(visual::channel_lane_pitch(stripe_off));
		let normal_pts = visual::offset_polyline(&pts, lanes[0].unwrap_or(0.0));
		match paint.body {
			ArcBody::Neutral => {
				back.push((pts.clone(), rgb_of(visual::NEUTRAL_ARC_RGB), width));
			}
			ArcBody::Solid { color, thick } => {
				let w = if thick {
					width * f64::from(visual::ARC_SAME_COLOR_WIDTH_SCALE)
				} else {
					width
				};
				back.push((normal_pts.clone(), rgb_of(visual::tile_rgb(color)), w));
			}
			ArcBody::Hitch { from, to } => {
				push_hitch_diamond(
					&normal_pts,
					from,
					to,
					[None; 3],
					[None; 3],
					hitch_r,
					stripe_off,
					width,
					braid_amp,
					layout.scale,
					&mut back,
					&mut front,
				);
			}
			ArcBody::Stripes { a, b, zag_a, zag_b } => {
				let strand_w = width * f64::from(visual::STRIPE_STRAND_WIDTH_SCALE);
				let zag_w = width * f64::from(visual::ZAG_STRAND_WIDTH_SCALE);
				let teeth = visual::zag_teeth(arc);
				push_stripe(
					&normal_pts,
					stripe_off,
					zag_a,
					rgb_of(visual::tile_rgb(a)),
					strand_w,
					zag_w,
					zag_amp,
					teeth,
					&mut back,
					&mut front,
				);
				push_stripe(
					&normal_pts,
					-stripe_off,
					zag_b,
					rgb_of(visual::tile_rgb(b)),
					strand_w,
					zag_w,
					zag_amp,
					teeth,
					&mut back,
					&mut front,
				);
			}
			ArcBody::Hidden => {}
		}
		if let Some(lane) = paint.braid {
			let braid_pts = visual::offset_polyline(&pts, lanes[2].unwrap_or(0.0));
			let strand_w = width * f64::from(visual::BRAID_STRAND_WIDTH_SCALE);
			match lane {
				BraidLane::Weave { colors } => {
					for thread in visual::braid_threads(
						&braid_pts,
						braid_amp,
						visual::braid_turns(arc),
						colors,
					) {
						split_braid(
							&thread.pts,
							&thread.phase,
							rgb_of(visual::tile_rgb(thread.color)),
							strand_w,
							&mut back,
							&mut front,
						);
					}
				}
				BraidLane::Failed { from, to } => {
					push_hitch_diamond(
						&braid_pts,
						[None; 3],
						[None; 3],
						from,
						to,
						hitch_r,
						stripe_off,
						width,
						braid_amp,
						layout.scale,
						&mut back,
						&mut front,
					);
				}
			}
		}
		let zag_w = width * f64::from(visual::ZAG_STRAND_WIDTH_SCALE);
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
			front.push((zig, rgb_of(visual::tile_rgb(color)), zag_w));
		}
		for (i, zag) in paint.zags_to.iter().enumerate() {
			let Some(color) = *zag else {
				continue;
			};
			let side = zag_lane + visual::zag_fan_offset(i, -1.0, inner, spacing);
			let zig = visual::zigzag_alongside(&pts, side, zag_amp, teeth);
			front.push((zig, rgb_of(visual::tile_rgb(color)), zag_w));
		}
		let mid_n = paint.zag_mid_count();
		let mid_off = visual::zag_center_offsets(mid_n, spacing);
		for (i, zag) in paint.zags_mid.iter().enumerate() {
			let Some(color) = *zag else {
				continue;
			};
			let zig = visual::zigzag_alongside(&pts, zag_lane + mid_off[i], zag_amp, teeth);
			front.push((zig, rgb_of(visual::tile_rgb(color)), zag_w));
		}
	}
	for (pts, color, width) in back.into_iter().chain(front) {
		stroke_polyline(img, &pts, color, width);
	}
	for arc in board.arcs() {
		let samples = arc.sample(visual::ARC_SAMPLES);
		let pts: Vec<[f64; 2]> = samples.iter().map(|p| image_pt(layout, *p)).collect();
		let paint = visual::arc_paint_with(
			game.tile_at(arc.from),
			game.tile_at(arc.to),
			game.rules().zag_mismatch,
		);
		if !paint.has_bead_lane() {
			continue;
		}
		let stripe_off = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.stripe_off_center)
		} else {
			f64::from(layout.stripe_off)
		};
		let lanes = paint.lane_offsets(visual::channel_lane_pitch(stripe_off));
		let bead_lane = lanes[3].unwrap_or(0.0);
		let bead_spacing = visual::ARC_BEAD_SPACING_UNITS * f64::from(layout.scale);
		let bead_r = f64::from(visual::arc_bead_radius(layout.arc_width));
		let pinch = f64::from(visual::bead_pinch(bead_r as f32));
		if let Some(color) = paint.bead_match {
			let line = visual::offset_polyline(&pts, bead_lane);
			let fill = rgb_of(visual::tile_rgb(color));
			fill_tris(img, &visual::bead_chain_triangles(&line, bead_spacing, bead_r, pinch), fill);
		}
		let n = paint.bead_from_count() + paint.bead_to_count() + paint.bead_mid_count();
		let fan = visual::zag_center_offsets(n.max(1), bead_spacing.max(1.0));
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
			let line =
				visual::offset_polyline(&pts, bead_lane + fan.get(i).copied().unwrap_or(0.0));
			let fill = rgb_of(visual::tile_rgb(color));
			fill_tris(img, &visual::bead_chain_triangles(&line, bead_spacing, bead_r, pinch), fill);
			i += 1;
		}
	}
}

fn split_braid(
	pts: &[[f64; 2]],
	phase: &[f64],
	color: Rgba<u8>,
	width: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	visual::for_braid_segments(pts, phase, |slice, is_front| {
		let dest = if is_front { &mut *front } else { &mut *back };
		dest.push((slice.to_vec(), color, width));
	});
}

fn push_stripe(
	centerline: &[[f64; 2]],
	offset: f64,
	is_zag: bool,
	color: Rgba<u8>,
	strand_w: f64,
	zag_w: f64,
	zag_amp: f64,
	teeth: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	if is_zag {
		let zig = visual::zigzag_alongside(centerline, offset, zag_amp, teeth);
		front.push((zig, color, zag_w));
	} else {
		back.push((visual::offset_polyline(centerline, offset), color, strand_w));
	}
}

fn push_hitch_half(
	pts: &[[f64; 2]],
	colors: [Option<crate::game::TileColor>; visual::MAX_HITCH_COLORS],
	stripe_off: f64,
	arc_width: f64,
	outside: f64,
	beside_braid: bool,
	braid_amp: f64,
	braid_w: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
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
	let mut k = 0usize;
	for color in colors {
		let Some(c) = color else {
			continue;
		};
		back.push((
			visual::offset_polyline(pts, offsets[k]),
			rgb_of(visual::tile_rgb(c)),
			strand_w,
		));
		k += 1;
	}
}

fn push_hitch_diamond(
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
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
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
	let braid_w = width * f64::from(visual::BRAID_STRAND_WIDTH_SCALE);
	let from_weave = visual::braid_strand_count(from_braid) >= 2;
	let to_weave = visual::braid_strand_count(to_braid) >= 2;
	if from_weave {
		push_hitch_braid(
			&from_path,
			from_braid,
			amp,
			visual::braid_turns_along(&from_path, scale),
			braid_w,
			back,
			front,
		);
	}
	if to_weave {
		push_hitch_braid(
			&to_path,
			to_braid,
			amp,
			visual::braid_turns_along(&to_path, scale),
			braid_w,
			back,
			front,
		);
	}
	let mix = from_weave || to_weave;
	push_hitch_half(&from_path, from, stripe_off, width, 1.0, mix, amp, braid_w, back);
	push_hitch_half(&to_path, to, stripe_off, width, -1.0, mix, amp, braid_w, back);
}

fn push_hitch_braid(
	pts: &[[f64; 2]],
	colors: [Option<crate::game::TileColor>; 3],
	amp: f64,
	turns: f64,
	strand_w: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	if visual::braid_strand_count(colors) < 2 || pts.len() < 2 {
		return;
	}
	for thread in visual::braid_threads(pts, amp, turns, colors) {
		split_braid(
			&thread.pts,
			&thread.phase,
			rgb_of(visual::tile_rgb(thread.color)),
			strand_w,
			back,
			front,
		);
	}
}

fn draw_nodes(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	for id in 0..board.node_count() as u8 {
		if game.tile_at(id).is_some() {
			continue;
		}
		let p = image_pt(layout, board.position(id));
		if board.is_spawn(id) {
			fill_circle(img, tup(p), f64::from(layout.spawn_radius), rgb_of(visual::SPAWN_RGB));
		} else if id == board.center() {
			fill_circle(
				img,
				tup(p),
				f64::from(layout.node_radius + 1.5),
				rgb_of(visual::CENTER_RGB),
			);
		} else {
			fill_circle(img, tup(p), f64::from(layout.node_radius), rgb_of(visual::NODE_RGB));
		}
	}
}

fn draw_tiles_preview(
	img: &mut RgbaImage,
	game: &Game,
	layout: &ViewLayout,
	characters: &[(NodeId, &RgbaImage)],
) {
	let board = game.board();
	for id in 0..board.node_count() as u8 {
		if let Some(tile) = game.tile_at(id) {
			let p = image_pt(layout, board.position(id));
			let art = characters.iter().find(|(n, _)| *n == id).map(|(_, a)| *a);
			draw_tile_preview(img, tup(p), f64::from(layout.tile_radius), tile, art);
		}
	}
}

fn draw_marks(
	img: &mut RgbaImage,
	game: &Game,
	layout: &ViewLayout,
	selected: Option<crate::board::NodeId>,
) {
	let Some(from) = selected else {
		return;
	};
	let board = game.board();
	// Destinations follow RuleSet movement (pass-through default, hops optional).
	for id in game.legal_destinations(from) {
		let p = image_pt(layout, board.position(id));
		stroke_circle(
			img,
			tup(p),
			f64::from(layout.tile_radius + 5.0),
			rgb_of(visual::DEST_RGB),
			3.2,
		);
	}
	let p = image_pt(layout, board.position(from));
	stroke_circle(
		img,
		tup(p),
		f64::from(layout.tile_radius + 7.0),
		rgb_of(visual::HIGHLIGHT_RGB),
		3.6,
	);
}

fn draw_benches(
	img: &mut RgbaImage,
	game: &Game,
	layout: &ViewLayout,
	flash: Option<&CaptureFlash>,
) {
	for player in [Player::Two, Player::One] {
		let deck = game.deck_slots(player);
		let n = deck.len();
		for (i, slot) in deck.iter().enumerate() {
			let p = layout.bench_pos(player, i, n);
			let q = layout.world_to_image(p);
			let c = (f64::from(q[0]), f64::from(q[1]));
			let r = f64::from(layout.bench_tile_radius);
			match slot {
				DeckSlot::Empty => draw_solid_token(img, c, r, player),
				DeckSlot::Live(tile) => {
					draw_tile(img, c, r, *tile);
					if flash.is_some_and(|f| f.highlights_tile(tile.id, CaptureDest::Bench)) {
						draw_capture_glow(img, c, r);
					}
				}
				DeckSlot::Spent(_) => {}
			}
		}
		let tokens = game.budget_tokens(player);
		let n = tokens.len();
		// Painter's algorithm matches Bevy `token_stack_*_z`: index 0 is the
		// back of the pile; each later disc's opaque fill covers the previous
		// rim so stacked edges show, not rims shining through the front face.
		for i in 0..n {
			let p = layout.budget_stack_pos(player, i, n);
			let q = layout.world_to_image(p);
			let c = (f64::from(q[0]), f64::from(q[1]));
			let r = f64::from(layout.bench_tile_radius);
			draw_solid_token(img, c, r, player);
			if let BudgetToken::Captured(tile) = tokens[i] {
				if flash.is_some_and(|f| f.highlights_tile(tile.id, CaptureDest::Tokens)) {
					draw_capture_glow(img, c, r);
				}
			}
		}
	}
}

fn draw_flash(img: &mut RgbaImage, layout: &ViewLayout, flash: Option<&CaptureFlash>) {
	let Some(flash) = flash else {
		return;
	};
	if flash.is_empty() {
		return;
	}
	let n = flash.tiles.len();
	let r = f64::from(layout.capture_flash_radius());
	for (i, tile) in flash.tiles.iter().enumerate() {
		let p = layout.capture_flash_pos(i, n);
		let q = layout.world_to_image(p);
		draw_tile(img, (f64::from(q[0]), f64::from(q[1])), r, *tile);
	}
}

fn draw_capture_glow(img: &mut RgbaImage, c: (f64, f64), r: f64) {
	let scale = (r / f64::from(visual::PREV_TILE_RADIUS_PX)).max(0.4);
	stroke_circle(img, c, r + 2.4 * scale, rgb_of(visual::CAPTURE_GLOW_INNER_RGB), 1.6 * scale);
	stroke_circle(img, c, r + 4.6 * scale, rgb_of(visual::CAPTURE_GLOW_RGB), 2.2 * scale);
}

fn draw_hud(img: &mut RgbaImage, game: &Game, layout: &ViewLayout, caption: &str) {
	let hi = rgba(230, 230, 220, 255);
	let mid = rgba(200, 204, 190, 255);
	let lo = rgba(140, 146, 156, 255);
	draw_text(img, 16, 12, "APA KAN  |  8-PETAL ROSETTE", hi);
	draw_text(img, 16, 30, caption, mid);
	let turn = visual::format_turn_line(game, "human");
	draw_text(img, 16, 48, &turn, mid);
	draw_text(img, 16, (layout.height - 22.0) as u32, visual::HELP_TEXT, lo);
}

fn draw_solid_token(img: &mut RgbaImage, c: (f64, f64), r: f64, player: Player) {
	fill_circle(img, c, r, rgb_of(visual::owner_fill_rgb(player)));
	stroke_circle(img, c, r, rgb_of(visual::BLANK_TOKEN_RIM_RGB), (r * 0.10).max(1.4));
}

fn draw_tile(img: &mut RgbaImage, c: (f64, f64), r: f64, tile: Tile) {
	draw_tile_preview(img, c, r, tile, None);
}

fn draw_tile_preview(
	img: &mut RgbaImage,
	c: (f64, f64),
	r: f64,
	tile: Tile,
	character: Option<&RgbaImage>,
) {
	fill_circle(img, c, r, rgb_of(visual::owner_fill_rgb(tile.owner)));
	if let Some(art) = character {
		blit_character_medallion(img, c, r, tile, art);
	}
	let primary = rgb_of(visual::tile_rgb(tile.primary()));
	let draw = visual::TileRingDraw::for_radius(r as f32);
	let marks = visual::capture_mark_count(tile);
	let down = visual::CAPTURE_DOWN_IMAGE;
	let notches = visual::tile_notches(tile, down);
	let strokes = tile.ring_strokes();
	for k in 0..strokes {
		if tile.stroke_is_braid(k) {
			continue;
		}
		let rr = f64::from(draw.ring_radius(r as f32, k));
		if rr > 4.0 * f64::from(draw.scale) {
			let color = tile.stroke_color(k);
			let width =
				f64::from(if tile.stroke_is_zag(k) { draw.zag_stroke } else { draw.stroke });
			if tile.stroke_is_zag(k) {
				for zig in visual::zigzag_notched(
					[c.0, c.1],
					rr,
					f64::from(draw.zag_amp),
					draw.zag_teeth,
					&notches,
				) {
					stroke_polyline(img, &zig, rgb_of(visual::tile_rgb(color)), width);
				}
			} else if tile.stroke_is_bead(k) {
				let bead_r = f64::from(draw.bead_r);
				let pinch = f64::from(visual::bead_pinch(draw.bead_r));
				let fill = rgb_of(visual::tile_rgb(color));
				fill_tris(
					img,
					&visual::bead_chain_notched(
						[c.0, c.1],
						rr,
						draw.bead_count,
						bead_r,
						pinch,
						visual::TILE_BEAD_SAMPLES,
						&notches,
					),
					fill,
				);
				for rim in
					visual::tile_ring_notched([c.0, c.1], rr, &notches, visual::TILE_BEAD_SAMPLES)
				{
					for p in [rim.first(), rim.last()].into_iter().flatten() {
						fill_circle(img, (p[0], p[1]), pinch, fill);
					}
				}
			} else {
				for rim in visual::tile_ring_notched(
					[c.0, c.1],
					rr,
					&notches,
					visual::TILE_RING_ARC_SAMPLES,
				) {
					stroke_polyline(img, &rim, rgb_of(visual::tile_rgb(color)), width);
				}
			}
		}
	}
	if tile.braid_ring_count() >= 2 {
		let rr = f64::from(draw.ring_radius(r as f32, 0));
		if rr > 4.0 * f64::from(draw.scale) {
			let amp = f64::from(draw.braid_amp);
			let w = f64::from(draw.braid_stroke);
			for rim in visual::tile_ring_notched([c.0, c.1], rr, &notches, draw.braid_samples) {
				let sweep = visual::polyline_angular_sweep(&rim, [c.0, c.1]);
				let turns = visual::tile_braid_turns(rr) * sweep / (2.0 * std::f64::consts::PI);
				let mut back: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
				let mut front: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
				for thread in visual::braid_threads(&rim, amp, turns, tile.braid_colors()) {
					split_braid(
						&thread.pts,
						&thread.phase,
						rgb_of(visual::tile_rgb(thread.color)),
						w,
						&mut back,
						&mut front,
					);
				}
				for (pts, color, width) in back.into_iter().chain(front) {
					stroke_polyline(img, &pts, color, width);
				}
			}
		}
	}
	for i in 0..marks {
		let triangle = matches!(tile.capture, crate::game::CaptureMark::Triangle);
		let poly = draw.capture_mark_poly(
			[c.0 as f32, c.1 as f32],
			r as f32,
			i,
			marks,
			visual::CAPTURE_DOWN_IMAGE,
			triangle,
		);
		fill_polygon(img, &poly, primary);
	}
	let glyph =
		draw.emblem_glyph(tile.emblem, [c.0 as f32, c.1 as f32], r as f32, [-down[0], -down[1]]);
	for fill in glyph.fills {
		match fill {
			visual::EmblemFill::Tris(tris) => fill_tris(img, &tris, primary),
			visual::EmblemFill::Poly(poly) => fill_polygon(img, &poly, primary),
		}
	}
	for stroke in glyph.strokes {
		let pts: Vec<[f64; 2]> =
			stroke.pts.iter().map(|p| [f64::from(p[0]), f64::from(p[1])]).collect();
		stroke_polyline(img, &pts, primary, f64::from(stroke.width).max(1.4));
	}
}

fn image_pt(layout: &ViewLayout, p: [f64; 2]) -> [f64; 2] {
	let q = layout.board_to_image(p);
	[f64::from(q[0]), f64::from(q[1])]
}

fn tup(p: [f64; 2]) -> (f64, f64) {
	(p[0], p[1])
}

fn rgb_of(rgb: [f32; 3]) -> Rgba<u8> {
	let c = visual::rgb8(rgb);
	rgba(c[0], c[1], c[2], 255)
}

fn stroke_polyline(img: &mut RgbaImage, pts: &[[f64; 2]], color: Rgba<u8>, width: f64) {
	for w in pts.windows(2) {
		draw_line(img, (w[0][0], w[0][1]), (w[1][0], w[1][1]), color, width);
	}
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba<u8> {
	Rgba([r, g, b, a])
}

fn blend(dst: &mut Rgba<u8>, src: Rgba<u8>, a: f64) {
	if a <= 0.0 {
		return;
	}
	let a = a.min(1.0);
	let ia = 1.0 - a;
	for i in 0..3 {
		dst.0[i] = (f64::from(dst.0[i]) * ia + f64::from(src.0[i]) * a) as u8;
	}
}

fn put(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>, a: f64) {
	if x < 0 || y < 0 {
		return;
	}
	let (w, h) = (img.width() as i32, img.height() as i32);
	if x >= w || y >= h {
		return;
	}
	blend(&mut img.get_pixel_mut(x as u32, y as u32), color, a);
}

fn fill_polygon(img: &mut RgbaImage, pts: &[[f32; 2]], color: Rgba<u8>) {
	fill_tris(img, &visual::polygon_triangles(pts), color);
}

fn fill_tris(img: &mut RgbaImage, tris: &[[[f32; 2]; 3]], color: Rgba<u8>) {
	for tri in tris {
		fill_triangle(
			img,
			(f64::from(tri[0][0]), f64::from(tri[0][1])),
			(f64::from(tri[1][0]), f64::from(tri[1][1])),
			(f64::from(tri[2][0]), f64::from(tri[2][1])),
			color,
		);
	}
}

fn fill_triangle(
	img: &mut RgbaImage,
	a: (f64, f64),
	b: (f64, f64),
	c: (f64, f64),
	color: Rgba<u8>,
) {
	let minx = a.0.min(b.0).min(c.0).floor() as i32 - 1;
	let maxx = a.0.max(b.0).max(c.0).ceil() as i32 + 1;
	let miny = a.1.min(b.1).min(c.1).floor() as i32 - 1;
	let maxy = a.1.max(b.1).max(c.1).ceil() as i32 + 1;
	let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
	if area.abs() < 1e-6 {
		return;
	}
	for y in miny..=maxy {
		for x in minx..=maxx {
			let px = f64::from(x) + 0.5;
			let py = f64::from(y) + 0.5;
			let w0 = ((b.0 - px) * (c.1 - py) - (c.0 - px) * (b.1 - py)) / area;
			let w1 = ((c.0 - px) * (a.1 - py) - (a.0 - px) * (c.1 - py)) / area;
			let w2 = 1.0 - w0 - w1;
			if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
				put(img, x, y, color, 1.0);
				continue;
			}
			let d = (-w0).max(-w1).max(-w2);
			let alpha = (0.55 - d * 8.0).clamp(0.0, 1.0);
			if alpha > 0.0 {
				put(img, x, y, color, alpha);
			}
		}
	}
}

/// Preview-only: circular-clip `art` into the tile center, scaled to
/// [`visual::TileRingDraw::character_fit_radius`]. Aspect ratio is preserved
/// by mapping the source's inscribed circle onto that disc.
fn blit_character_medallion(
	img: &mut RgbaImage,
	c: (f64, f64),
	tile_r: f64,
	tile: Tile,
	art: &RgbaImage,
) {
	let draw = visual::TileRingDraw::for_radius(tile_r as f32);
	let fit = f64::from(draw.character_fit_radius(tile, tile_r as f32));
	if fit < 2.0 {
		return;
	}
	let sw = f64::from(art.width());
	let sh = f64::from(art.height());
	let src_r = sw.min(sh) * 0.5;
	if src_r < 1.0 {
		return;
	}
	let minx = (c.0 - fit - 1.0).floor() as i32;
	let maxx = (c.0 + fit + 1.0).ceil() as i32;
	let miny = (c.1 - fit - 1.0).floor() as i32;
	let maxy = (c.1 + fit + 1.0).ceil() as i32;
	for y in miny..=maxy {
		for x in minx..=maxx {
			let px = f64::from(x) + 0.5;
			let py = f64::from(y) + 0.5;
			let dx = px - c.0;
			let dy = py - c.1;
			let dist = dx.hypot(dy);
			let dest_edge = (fit - dist + 0.55).clamp(0.0, 1.0);
			if dest_edge <= 0.0 {
				continue;
			}
			let sx = sw * 0.5 + dx * src_r / fit;
			let sy = sh * 0.5 + dy * src_r / fit;
			let sdx = sx - sw * 0.5;
			let sdy = sy - sh * 0.5;
			let src_edge = (src_r - sdx.hypot(sdy) + 0.55).clamp(0.0, 1.0);
			if src_edge <= 0.0 {
				continue;
			}
			let sample = sample_bilinear(art, sx, sy);
			let a = dest_edge * src_edge * f64::from(sample.0[3]) / 255.0;
			put(img, x, y, sample, a);
		}
	}
}

fn sample_bilinear(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
	let w = img.width();
	let h = img.height();
	if w == 0 || h == 0 {
		return Rgba([0, 0, 0, 0]);
	}
	if w == 1 && h == 1 {
		return *img.get_pixel(0, 0);
	}
	let x = x.clamp(0.0, f64::from(w.saturating_sub(1)));
	let y = y.clamp(0.0, f64::from(h.saturating_sub(1)));
	let x0 = x.floor() as u32;
	let y0 = y.floor() as u32;
	let x1 = (x0 + 1).min(w - 1);
	let y1 = (y0 + 1).min(h - 1);
	let fx = x - f64::from(x0);
	let fy = y - f64::from(y0);
	let p00 = img.get_pixel(x0, y0).0;
	let p10 = img.get_pixel(x1, y0).0;
	let p01 = img.get_pixel(x0, y1).0;
	let p11 = img.get_pixel(x1, y1).0;
	let mut out = [0u8; 4];
	for i in 0..4 {
		let v0 = f64::from(p00[i]) * (1.0 - fx) + f64::from(p10[i]) * fx;
		let v1 = f64::from(p01[i]) * (1.0 - fx) + f64::from(p11[i]) * fx;
		out[i] = (v0 * (1.0 - fy) + v1 * fy).round().clamp(0.0, 255.0) as u8;
	}
	Rgba(out)
}

fn fill_circle(img: &mut RgbaImage, c: (f64, f64), r: f64, color: Rgba<u8>) {
	let r2 = r + 0.5;
	let minx = (c.0 - r2 - 1.0).floor() as i32;
	let maxx = (c.0 + r2 + 1.0).ceil() as i32;
	let miny = (c.1 - r2 - 1.0).floor() as i32;
	let maxy = (c.1 + r2 + 1.0).ceil() as i32;
	for y in miny..=maxy {
		for x in minx..=maxx {
			let dx = f64::from(x) + 0.5 - c.0;
			let dy = f64::from(y) + 0.5 - c.1;
			let d = (dx * dx + dy * dy).sqrt();
			let a = (r2 - d + 0.5).clamp(0.0, 1.0);
			put(img, x, y, color, a);
		}
	}
}

fn stroke_circle(img: &mut RgbaImage, c: (f64, f64), r: f64, color: Rgba<u8>, width: f64) {
	let outer = r + width / 2.0;
	let inner = (r - width / 2.0).max(0.0);
	let minx = (c.0 - outer - 1.0).floor() as i32;
	let maxx = (c.0 + outer + 1.0).ceil() as i32;
	let miny = (c.1 - outer - 1.0).floor() as i32;
	let maxy = (c.1 + outer + 1.0).ceil() as i32;
	for y in miny..=maxy {
		for x in minx..=maxx {
			let dx = f64::from(x) + 0.5 - c.0;
			let dy = f64::from(y) + 0.5 - c.1;
			let d = (dx * dx + dy * dy).sqrt();
			let a = (outer + 0.5 - d).clamp(0.0, 1.0) * (d - inner + 0.5).clamp(0.0, 1.0);
			put(img, x, y, color, a);
		}
	}
}

fn draw_line(img: &mut RgbaImage, a: (f64, f64), b: (f64, f64), color: Rgba<u8>, width: f64) {
	let dx = b.0 - a.0;
	let dy = b.1 - a.1;
	let len = (dx * dx + dy * dy).sqrt().max(1.0);
	let n = (len.ceil() as i32).max(1);
	for i in 0..=n {
		let t = f64::from(i) / f64::from(n);
		fill_circle(img, (a.0 + dx * t, a.1 + dy * t), width / 2.0, color);
	}
}

fn draw_text(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
	let mut cx = x as i32;
	let cy = y as i32;
	for ch in text.chars() {
		let g = glyph(ch);
		for (row, bits) in g.iter().enumerate() {
			for col in 0..5 {
				if bits & (1 << (4 - col)) != 0 {
					for oy in 0..2 {
						for ox in 0..2 {
							put(
								img,
								cx + col as i32 * 2 + ox,
								cy + row as i32 * 2 + oy,
								color,
								1.0,
							);
						}
					}
				}
			}
		}
		cx += 12;
	}
}

fn glyph(c: char) -> [u8; 7] {
	match c.to_ascii_uppercase() {
		'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
		'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
		'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
		'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
		'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
		'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
		'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
		'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
		'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
		'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
		'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
		'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
		'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
		'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
		'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
		'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
		'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
		'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
		'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
		'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
		'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
		'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
		'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
		'0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
		'1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
		'2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
		'3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
		'4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
		'5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
		'6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
		'7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
		'8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
		'9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
		'-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
		'.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100],
		',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
		':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
		'=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
		'+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
		'/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
		'|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
		'(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
		')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
		'\'' => [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
		' ' | _ => [0; 7],
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{CaptureMark, Emblem, Game, Player, RingType, RuleSet, TileColor};

	fn emblem_closeup_tiles(game: &mut Game) -> Vec<(crate::game::Tile, &'static str)> {
		let rings_moon = [
			(TileColor::Green, RingType::Normal),
			(TileColor::Blue, RingType::Normal),
			(TileColor::Orange, RingType::Zag),
		];
		let rings_zs = [
			(TileColor::Orange, RingType::Zag),
			(TileColor::Blue, RingType::Zag),
			(TileColor::Green, RingType::Normal),
		];
		let rings_block = [
			(TileColor::Blue, RingType::Normal),
			(TileColor::Green, RingType::Normal),
			(TileColor::Orange, RingType::Normal),
		];
		let rings_rose = [
			(TileColor::Green, RingType::Braid),
			(TileColor::Blue, RingType::Braid),
			(TileColor::Orange, RingType::Normal),
		];
		vec![
			(
				game.alloc_typed_rings(Player::One, &rings_moon, 1, 3)
					.with_capture(CaptureMark::Dot)
					.with_emblem(Emblem::CrescentMoon),
				"emblem_crescent_dot.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_moon, 1, 3)
					.with_capture(CaptureMark::Triangle)
					.with_emblem(Emblem::CrescentMoon),
				"emblem_crescent_triangle.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_zs, 1, 3)
					.with_capture(CaptureMark::Dot)
					.with_emblem(Emblem::TwoZagZs),
				"emblem_zs_dot.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_zs, 1, 3)
					.with_capture(CaptureMark::Triangle)
					.with_emblem(Emblem::TwoZagZs),
				"emblem_zs_triangle.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_block, 1, 3)
					.with_capture(CaptureMark::Dot)
					.with_emblem(Emblem::Blocker),
				"emblem_blocker_dot.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_block, 1, 3)
					.with_capture(CaptureMark::Triangle)
					.with_emblem(Emblem::Blocker),
				"emblem_blocker_triangle.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_rose, 1, 3)
					.with_capture(CaptureMark::Dot)
					.with_emblem(Emblem::Rosette),
				"emblem_rosette_dot.png",
			),
			(
				game.alloc_typed_rings(Player::One, &rings_rose, 1, 3)
					.with_capture(CaptureMark::Triangle)
					.with_emblem(Emblem::Rosette),
				"emblem_rosette_triangle.png",
			),
		]
	}

	#[test]
	fn write_emblem_closeups() {
		let mut game = Game::empty(RuleSet::default());
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots");
		std::fs::create_dir_all(&dir).unwrap();
		for (tile, name) in emblem_closeup_tiles(&mut game) {
			write_tile_closeup(tile, dir.join(name), 880, 330.0);
		}
	}

	#[test]
	fn write_character_tile_previews() {
		use crate::characters::CharacterLibrary;
		use std::path::PathBuf;

		let lib = CharacterLibrary::load().unwrap();
		assert_eq!(lib.len(), 14);
		let mut game = Game::empty(RuleSet::default());
		let specs = character_preview_specs(&mut game);
		assert_eq!(specs.len(), 14);

		let shot_dir =
			std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots/characters");
		std::fs::create_dir_all(&shot_dir).unwrap();
		let art_dir = PathBuf::from("/opt/cursor/artifacts");
		let _ = std::fs::create_dir_all(&art_dir);

		let mut arts: Vec<RgbaImage> = Vec::with_capacity(specs.len());
		for spec in &specs {
			let ch = lib.get(spec.id).unwrap_or_else(|| panic!("missing character {}", spec.id));
			arts.push(load_preview_art(&lib.asset_path(ch)));
		}

		for (i, spec) in specs.iter().enumerate() {
			let name = format!("character_{:02}_{}.png", i + 1, spec.id.replace('-', "_"));
			write_tile_closeup_with_character(
				spec.tile,
				&arts[i],
				shot_dir.join(&name),
				880,
				330.0,
			);
			if art_dir.is_dir() {
				let _ = std::fs::copy(shot_dir.join(&name), art_dir.join(&name));
			}
		}

		let sheet = write_character_contact_sheet(&specs, &arts);
		sheet.save(shot_dir.join("character_contact_sheet.png")).unwrap();
		if art_dir.is_dir() {
			sheet.save(art_dir.join("character_contact_sheet.png")).unwrap();
		}

		let (neighbors, overlays) = character_neighbor_board(&specs, &arts);
		write_character_board_crop(
			&neighbors,
			&overlays,
			shot_dir.join("character_board_neighbors.png"),
			"preview: two neighboring tiles with character medallions",
			36.0,
		);
		if art_dir.is_dir() {
			let _ = std::fs::copy(
				shot_dir.join("character_board_neighbors.png"),
				art_dir.join("character_board_neighbors.png"),
			);
		}

		let (cluster, cluster_overlays) = character_cluster_board(&specs, &arts);
		write_png_view_preview(
			&cluster,
			shot_dir.join("character_board_cluster.png"),
			"preview: character medallions in tile centers (not generation)",
			1280,
			800,
			None,
			None,
			&cluster_overlays,
		)
		.unwrap();
		if art_dir.is_dir() {
			let _ = std::fs::copy(
				shot_dir.join("character_board_cluster.png"),
				art_dir.join("character_board_cluster.png"),
			);
		}

		assert!(shot_dir.join("character_contact_sheet.png").is_file());
		assert!(shot_dir.join("character_board_neighbors.png").is_file());
	}

	#[test]
	fn opening_deck_paints_every_unplaced_tile() {
		use crate::game::Player;
		use crate::visual::{self, ViewLayout};
		let game = Game::new(RuleSet::default(), 1);
		let slots = game.deck_slots(Player::One);
		assert_eq!(slots.len(), 11);
		assert_eq!(slots.iter().filter(|s| s.is_live()).count(), 11);
		assert_eq!(slots.iter().filter(|s| s.shows_blank_face()).count(), 0);
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-opening-deck.png");
		write_png_sized(&game, &path, "opening deck", w, h).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout = ViewLayout::from_window_with_deck(w as f32, h as f32, 11);
		let p = layout.world_to_image(layout.bench_pos(Player::One, 10, 11));
		let fill = visual::rgb8(visual::owner_fill_rgb(Player::One));
		let tile = slots[10].tile().expect("last opening slot is a live tile");
		let draw = visual::TileRingDraw::for_radius(layout.bench_tile_radius);
		let mark = draw.capture_mark_pos(
			p,
			layout.bench_tile_radius,
			0,
			visual::capture_mark_count(tile),
			visual::CAPTURE_DOWN_IMAGE,
		);
		let px = img.get_pixel(mark[0] as u32, mark[1] as u32);
		assert!(
			!channel_near(px[0], fill[0], 18)
				|| !channel_near(px[1], fill[1], 18)
				|| !channel_near(px[2], fill[2], 18),
			"last deck slot should show a capture mark in the ring notch, not a blank face, got {px:?}"
		);
	}

	#[test]
	fn writes_a_png() {
		let game = Game::new(RuleSet::default(), 1);
		let path = std::env::temp_dir().join("apa-kan-render-test.png");
		write_png(&game, &path, "test").unwrap();
		assert!(path.is_file());
		let bytes = std::fs::read(&path).unwrap();
		assert!(bytes.len() > 100);
		assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
	}

	#[test]
	fn writes_midgame_at_window_sizes() {
		let game = crate::visual::demo_midgame();
		for (w, h) in [(1024_u32, 728_u32), (1280, 800), (1920, 1080)] {
			let path = std::env::temp_dir().join(format!("apa-kan-render-{w}x{h}.png"));
			write_png_sized(&game, &path, "mid-game visual fixture", w, h).unwrap();
			let bytes = std::fs::read(&path).unwrap();
			assert!(bytes.len() > 100);
			assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
		}
	}

	#[test]
	fn budget_stack_is_left_and_empty_slots_are_opaque_blank() {
		use crate::game::Player;
		use crate::visual::{self, ViewLayout};
		let game = visual::demo_midgame();
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-budget-stack-left.png");
		write_png_sized(&game, &path, "budget stack left", w, h).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout =
			ViewLayout::from_window_with_deck(w as f32, h as f32, game.rules().bag_size as usize);
		for player in [Player::One, Player::Two] {
			let tokens = game.budget_tokens(player);
			let usable = game.budget(player);
			let n = tokens.len();
			let deck_n = game.deck_slots(player).len();
			assert_eq!(deck_n, game.bench(player).len());
			assert!(deck_n < game.rules().bag_size as usize);
			let bench0 = layout.world_to_image(layout.bench_pos(player, 0, deck_n));
			let stack0 = layout.world_to_image(layout.budget_stack_pos(player, 0, n));
			assert!(
				stack0[0] < bench0[0],
				"{player:?}: stack x {} should be left of bench {}",
				stack0[0],
				bench0[0]
			);
			assert!(stack0[0] < w as f32 * 0.45, "{player:?}: stack not on the left side");
			let i = n - 1;
			assert!(visual::budget_slot_is_blank_face(&tokens[i], i, usable));
			let p = layout.world_to_image(layout.budget_stack_pos(player, i, n));
			let px = img.get_pixel(p[0] as u32, p[1] as u32);
			let fill = visual::rgb8(visual::owner_fill_rgb(player));
			assert!(
				channel_near(px[0], fill[0], 18)
					&& channel_near(px[1], fill[1], 18)
					&& channel_near(px[2], fill[2], 18),
				"{player:?}: empty-slot center should be opaque owner fill {fill:?}, got {px:?}"
			);
			let r = layout.bench_tile_radius * 0.35;
			let ring_px = img.get_pixel((p[0] + r) as u32, p[1] as u32);
			assert!(
				channel_near(ring_px[0], fill[0], 40)
					&& channel_near(ring_px[1], fill[1], 40)
					&& channel_near(ring_px[2], fill[2], 40),
				"{player:?}: empty slot must not show rings/dots, got {ring_px:?} vs fill {fill:?}"
			);
		}
	}

	#[test]
	fn budget_stack_front_disc_occludes_rear_rims() {
		use crate::game::Player;
		use crate::visual::{self, ViewLayout};
		let game = Game::new(RuleSet::default(), 1);
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-budget-stack-occlude.png");
		write_png_sized(&game, &path, "opening token stacks", w, h).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout =
			ViewLayout::from_window_with_deck(w as f32, h as f32, game.rules().bag_size as usize);
		let rim = visual::rgb8(visual::BLANK_TOKEN_RIM_RGB);
		for player in [Player::One, Player::Two] {
			let n = game.budget_tokens(player).len();
			assert!(n >= 2, "{player:?}: need a stack to test occlusion");
			let fill = visual::rgb8(visual::owner_fill_rgb(player));
			let back = layout.world_to_image(layout.budget_stack_pos(player, n - 2, n));
			let front = layout.world_to_image(layout.budget_stack_pos(player, n - 1, n));
			let dx = front[0] - back[0];
			let dy = front[1] - back[1];
			let dist = (dx * dx + dy * dy).sqrt();
			assert!(dist > 1.0, "{player:?}: stacked discs should be offset");
			let r = layout.bench_tile_radius;
			let t = r / dist;
			let on_back_rim = [back[0] + dx * t, back[1] + dy * t];
			let px = img.get_pixel(on_back_rim[0] as u32, on_back_rim[1] as u32);
			assert!(
				channel_near(px[0], fill[0], 24)
					&& channel_near(px[1], fill[1], 24)
					&& channel_near(px[2], fill[2], 24),
				"{player:?}: front disc must hide the rear rim at {on_back_rim:?}; want fill {fill:?}, not rim {rim:?}, got {px:?}"
			);
			let front_c = img.get_pixel(front[0] as u32, front[1] as u32);
			assert!(
				channel_near(front_c[0], fill[0], 18)
					&& channel_near(front_c[1], fill[1], 18)
					&& channel_near(front_c[2], fill[2], 18),
				"{player:?}: front face should be opaque owner fill {fill:?}, got {front_c:?}"
			);
		}
	}

	#[test]
	fn usable_captured_budget_token_is_a_solid_disc() {
		use crate::game::{BudgetToken, Player, TileColor};
		use crate::visual::{self, ViewLayout};
		let mut game = Game::empty(RuleSet::default());
		let bench_tile = game.alloc_tile(Player::One, TileColor::Blue, 1, 1);
		game.set_bench(Player::One, vec![bench_tile]);
		let cap = game.alloc_tile(Player::Two, TileColor::Orange, 2, 1);
		game.set_budget_tokens(Player::One, vec![BudgetToken::Captured(cap), BudgetToken::Blank]);
		assert_eq!(game.budget(Player::One), 1);
		assert!(!visual::budget_slot_is_blank_face(&game.budget_tokens(Player::One)[0], 0, 1));
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-budget-captured.png");
		write_png_sized(&game, &path, "usable captured token", w, h).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout =
			ViewLayout::from_window_with_deck(w as f32, h as f32, game.rules().bag_size as usize);
		let p = layout.world_to_image(layout.budget_stack_pos(Player::One, 0, 2));
		let px = img.get_pixel(p[0] as u32, p[1] as u32);
		let fill = visual::rgb8(visual::owner_fill_rgb(Player::One));
		assert!(
			channel_near(px[0], fill[0], 18)
				&& channel_near(px[1], fill[1], 18)
				&& channel_near(px[2], fill[2], 18),
			"usable captured token center should be a solid owner fill {fill:?}, got {px:?}"
		);
		let r = layout.bench_tile_radius * 0.35;
		let ring_px = img.get_pixel((p[0] + r) as u32, p[1] as u32);
		assert!(
			channel_near(ring_px[0], fill[0], 40)
				&& channel_near(ring_px[1], fill[1], 40)
				&& channel_near(ring_px[2], fill[2], 40),
			"captured token must not show tile rings, got {ring_px:?} vs fill {fill:?}"
		);
	}

	fn channel_near(got: u8, want: u8, tol: u8) -> bool {
		got.abs_diff(want) <= tol
	}

	#[test]
	fn capture_marks_leave_the_tile_center_empty() {
		use crate::game::Player;
		use crate::visual::{self, ViewLayout};
		let game = crate::visual::demo_zag_tile();
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-edge-marks.png");
		write_png_sized(&game, &path, "edge marks", w, h).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout =
			ViewLayout::from_window_with_deck(w as f32, h as f32, game.rules().bag_size as usize);
		let (id, tile) = game.tiles().next().expect("zag tile");
		let p = layout.board_to_image(game.board().position(id));
		let px = img.get_pixel(p[0] as u32, p[1] as u32);
		let fill = visual::rgb8(visual::owner_fill_rgb(Player::One));
		assert!(
			channel_near(px[0], fill[0], 28)
				&& channel_near(px[1], fill[1], 28)
				&& channel_near(px[2], fill[2], 28),
			"tile center must stay empty for artwork, fill {fill:?} got {px:?}"
		);
		let draw = visual::TileRingDraw::for_radius(layout.tile_radius);
		let mark = draw.capture_mark_pos(
			p,
			layout.tile_radius,
			0,
			visual::capture_mark_count(tile),
			visual::CAPTURE_DOWN_IMAGE,
		);
		assert!(
			mark[1] > p[1] + layout.tile_radius * 0.4,
			"PNG mark sits toward the bottom of the tile"
		);
		assert!(
			mark[1] < p[1] + layout.tile_radius,
			"PNG mark must stay inside the tile, not hang below it"
		);
	}

	#[test]
	fn bead_pinches_stay_inked_at_board_scale() {
		use crate::game::TileColor;
		use crate::visual::{self, ViewLayout};
		let layout = ViewLayout::from_window(1280.0, 800.0);
		let peak = f64::from(visual::arc_bead_radius(layout.arc_width));
		let pinch = f64::from(visual::bead_pinch(visual::arc_bead_radius(layout.arc_width)));
		let period = visual::ARC_BEAD_SPACING_UNITS * f64::from(layout.scale);
		let y = 24.5;
		let pts: Vec<[f64; 2]> = (0..=220).map(|i| [f64::from(i), y]).collect();
		let bg = visual::rgb8(visual::BG_RGB);
		let mut img = image::RgbaImage::from_pixel(240, 48, rgba(bg[0], bg[1], bg[2], 255));
		fill_tris(
			&mut img,
			&visual::bead_chain_triangles(&pts, period, peak, pinch),
			rgb_of(visual::tile_rgb(TileColor::Orange)),
		);
		for k in 1..=8 {
			let x = (period * f64::from(k)).round() as u32;
			let px = img.get_pixel(x, 24);
			assert!(
				px[0] > 80,
				"pinch {k} at x={x} must stay connected at board scale, pixel={px:?} peak={peak:.2} pinch={pinch:.2}"
			);
		}
	}

	#[test]
	fn last_capture_flash_paints_between_the_stacks() {
		use crate::game::CaptureDest;
		use crate::visual::{self, ViewLayout};
		let (game, flash) = visual::demo_midgame_capture_flash(CaptureDest::Tokens);
		assert!(!flash.is_empty());
		let w = 1280u32;
		let h = 800u32;
		let path = std::env::temp_dir().join("apa-kan-capture-flash.png");
		write_png_view_flash(&game, &path, "flash", w, h, None, Some(&flash)).unwrap();
		let img = image::open(&path).unwrap().to_rgba8();
		let layout =
			ViewLayout::from_window_with_deck(w as f32, h as f32, game.rules().bag_size as usize);
		let p = layout.world_to_image(layout.capture_flash_pos(0, flash.tiles.len()));
		let px = img.get_pixel(p[0] as u32, p[1] as u32);
		let bg = visual::rgb8(visual::BG_RGB);
		assert!(
			!channel_near(px[0], bg[0], 12)
				|| !channel_near(px[1], bg[1], 12)
				|| !channel_near(px[2], bg[2], 12),
			"freeze-frame tile should not be the background, got {px:?}"
		);
		let glow = visual::rgb8(visual::CAPTURE_GLOW_RGB);
		let tokens = game.budget_tokens(crate::game::Player::One);
		let n = tokens.len();
		let mut found_glow = false;
		for i in 0..n {
			if let crate::game::BudgetToken::Captured(tile) = tokens[i] {
				if flash.contains_id(tile.id) {
					let q = layout.world_to_image(layout.budget_stack_pos(
						crate::game::Player::One,
						i,
						n,
					));
					let r = layout.bench_tile_radius;
					for extra in [3.0, 4.0, 5.0, 6.0, 7.0] {
						let gx = (q[0] + r + extra).clamp(0.0, w as f32 - 1.0) as u32;
						let gy = q[1].clamp(0.0, h as f32 - 1.0) as u32;
						let gpx = img.get_pixel(gx, gy);
						if channel_near(gpx[0], glow[0], 48)
							&& channel_near(gpx[1], glow[1], 60)
							&& channel_near(gpx[2], glow[2], 60)
						{
							found_glow = true;
						}
					}
				}
			}
		}
		assert!(found_glow, "triangle-capture landing should glow on the token stack");
	}

	#[test]
	fn dump_hitch_and_zag_artifacts() {
		let dir = std::path::Path::new("/opt/cursor/artifacts");
		if !dir.is_dir() {
			return;
		}
		use crate::game::{
			CaptureDest, CaptureMark, Player, RingType, RuleSet, TileColor, ZagMismatchCount,
		};
		use crate::visual::{
			demo_center_inner_pair, demo_hop_turn, demo_left_cluster, demo_midgame,
			demo_midgame_capture_flash, demo_pair, demo_pair_with, demo_typed_pair,
			demo_typed_tile, demo_zag_color_exclusion_board, demo_zag_tile,
		};
		let board = demo_midgame();
		write_png_sized(
			&board,
			dir.join("zag_board_1280x800.png"),
			"diamonds + realised-state edges",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&board,
			dir.join("capture_budget_stack_1280x800.png"),
			"left stacks, unplaced-only decks, solid token discs",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&board,
			dir.join("capture_budget_stack_390x844.png"),
			"left stacks and 11-tile decks on a mobile portrait window",
			390,
			844,
		)
		.unwrap();
		write_png_sized(
			&Game::new(RuleSet::default(), 1),
			dir.join("opening_deck_1280x800.png"),
			"opening: 11 live deck tiles, spawn-token stacks on the left",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&Game::new(RuleSet::default(), 1),
			dir.join("bigger_tiles_opening.png"),
			"1.1x tiles, solid token discs, full 11-tile opening deck",
			1280,
			800,
		)
		.unwrap();
		{
			let mut after = Game::new(RuleSet::default(), 1);
			let add = after.legal_adds().into_iter().next().expect("opening ADD");
			after.apply(add).unwrap();
			write_png_sized(
				&after,
				dir.join("deck_after_add_placed_removed.png"),
				"placed tile leaves the deck row",
				1280,
				800,
			)
			.unwrap();
		}
		write_png_sized(
			&board,
			dir.join("thicker_rim_braid_zag.png"),
			"1.3x thicker tile-rim braids and zags; solid token discs",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&board,
			dir.join("solid_token_discs.png"),
			"token stacks as solid filled discs",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&board,
			dir.join("diamond_board_1280x800.png"),
			"conflict diamonds, full board",
			1280,
			800,
		)
		.unwrap();
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, false), (TileColor::Green, false)],
				2,
				&[(TileColor::Orange, false), (TileColor::Teal, false)],
				2,
			),
			dir.join("diamond_blue_green_vs_orange_teal.png"),
			"A blue+green vs B orange+teal diamond",
			36.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Blue, false)], 2, &[(TileColor::Orange, false)], 2),
			dir.join("diamond_blue_vs_orange.png"),
			"blue vs orange diamond",
			36.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Orange, RingType::Normal), (TileColor::Green, RingType::Normal)],
				1,
			),
			dir.join("braid_orange_green_satisfied.png"),
			"orange+green braid vs both colors: old conflict weave",
			40.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
				1,
				&[(TileColor::Orange, RingType::Normal), (TileColor::Orange, RingType::Normal)],
				1,
			),
			dir.join("braid_orange_orange_satisfied.png"),
			"orange+orange braid: two orange threads",
			40.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Braid),
				],
				1,
				&[
					(TileColor::Orange, RingType::Normal),
					(TileColor::Green, RingType::Normal),
					(TileColor::Blue, RingType::Normal),
				],
				1,
			),
			dir.join("braid_three_threads_satisfied.png"),
			"3-braid: three woven threads",
			40.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Blue, RingType::Normal)],
				1,
			),
			dir.join("braid_failed_hitch.png"),
			"failed braid lane plus leftover-normal hitch on its own lane",
			40.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Normal),
				],
				1,
				&[(TileColor::Teal, RingType::Normal)],
				1,
			),
			dir.join("braid_failed_hitch_with_leftover.png"),
			"failed braid lane beside leftover blue vs teal hitch",
			40.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
			),
			dir.join("braid_one_ended.png"),
			"unconnected edge still shows the would-be braid weave",
			28.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, false), (TileColor::Orange, true)],
				2,
				&[(TileColor::Green, false)],
				2,
			),
			dir.join("diamond_next_to_zags.png"),
			"diamond next to a non-hitting orange zag",
			48.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Orange, false), (TileColor::Green, true)],
				2,
				&[(TileColor::Orange, false), (TileColor::Green, false)],
				2,
			),
			dir.join("zag_only_vs_orange_holder.png"),
			"green zag vs orange holder: zag only",
			48.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, true)],
				3,
				&[(TileColor::Orange, false), (TileColor::Green, false)],
				2,
			),
			dir.join("grey_no_effect_edge.png"),
			"blue zag vs orange+green: grey, no effect",
			48.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Blue, true)], 3, &[(TileColor::Blue, true)], 3),
			dir.join("extra_weight_strokes_one_ring_zag.png"),
			"one-ring zags: one stroke each (weight = ring count)",
			36.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Orange, false)], 3, &[(TileColor::Green, false)], 1),
			dir.join("extra_weight_strokes_one_ring_normal.png"),
			"one-ring orange vs green: one stroke each (weight = ring count)",
			36.0,
		);
		write_cropped(
			&demo_zag_tile(),
			dir.join("one_ended_potential_zags.png"),
			"one-ended potential zags",
			28.0,
		);
		write_png_sized(
			&demo_left_cluster(),
			dir.join("left_cluster_realized.png"),
			"left cluster, realized-state edges",
			1280,
			800,
		)
		.unwrap();
		let (hop, from) = demo_hop_turn();
		write_png_view(
			&hop,
			dir.join("hop_multidir_highlights.png"),
			"d=1 hop turns at center",
			1280,
			800,
			Some(from),
		)
		.unwrap();
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
				3,
				&[(TileColor::Orange, true), (TileColor::Orange, true), (TileColor::Orange, true)],
				3,
			),
			dir.join("zag_thickness_arc.png"),
			"thicker zag strokes",
			48.0,
		);
		write_cropped(
			&demo_zag_tile(),
			dir.join("zag_thickness_tile.png"),
			"thicker tile zags",
			28.0,
		);
		write_png_sized(
			&board,
			dir.join("zag_edge_board_1280x800.png"),
			"undirected zag mismatches on the center line",
			1280,
			800,
		)
		.unwrap();
		write_cropped(
			&demo_pair(&[(TileColor::Orange, true)], 1, &[(TileColor::Orange, true)], 1),
			dir.join("orange_zag_vs_orange_zag_sum.png"),
			"SUM: orange zag vs orange zag = 2 center squiggles",
			48.0,
		);
		let mut max = RuleSet::default();
		max.zag_mismatch = ZagMismatchCount::Max;
		let three_blue =
			[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)];
		write_cropped(
			&demo_pair(&three_blue, 3, &three_blue, 3),
			dir.join("three_blue_zags_vs_three_blue_zags_sum.png"),
			"SUM: 3 blue zags vs 3 blue zags = 6 center squiggles",
			48.0,
		);
		write_cropped(
			&demo_pair_with(max, &three_blue, 3, &three_blue, 3),
			dir.join("three_blue_zags_vs_three_blue_zags_max.png"),
			"MAX: 3 blue zags vs 3 blue zags = 3 center squiggles",
			48.0,
		);
		let two_orange = [(TileColor::Orange, true), (TileColor::Orange, true)];
		let one_orange = [(TileColor::Orange, true)];
		write_cropped(
			&demo_pair(&two_orange, 2, &one_orange, 1),
			dir.join("two_orange_zags_vs_one_orange_zag_sum.png"),
			"SUM: 2 orange zags vs 1 orange zag = 3 center squiggles",
			48.0,
		);
		write_cropped(
			&demo_pair_with(max, &two_orange, 2, &one_orange, 1),
			dir.join("two_orange_zags_vs_one_orange_zag_max.png"),
			"MAX: 2 orange zags vs 1 orange zag = 2 center squiggles",
			48.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Orange, true)], 1, &[(TileColor::Orange, false)], 1),
			dir.join("one_sided_zag_hit_center.png"),
			"one-sided orange zag hit on the center line",
			48.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, false), (TileColor::Orange, true)],
				2,
				&[(TileColor::Green, false), (TileColor::Orange, false)],
				2,
			),
			dir.join("zag_and_normal_mismatch_same_edge.png"),
			"zag hit plus hitch: orange zag is not a match for the orange normal",
			48.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Orange, false)], 1, &[(TileColor::Orange, false)], 1),
			dir.join("lane_normal_match.png"),
			"normal match lane: highway in the matched color",
			36.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Blue, false)], 1, &[(TileColor::Orange, false)], 1),
			dir.join("lane_normal_mismatch.png"),
			"normal mismatch lane: diamond of unmatched colors",
			36.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Orange, true)], 1, &[(TileColor::Orange, true)], 1),
			dir.join("lane_zag.png"),
			"zag lane: each triggered zag between the tiles",
			48.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Orange, true)], 1, &[(TileColor::Orange, false)], 1),
			dir.join("lane_zag_plus_normal.png"),
			"zag hit plus hitch: same-color zag is not a match for the normal",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Orange, RingType::Normal),
				],
				1,
				&[(TileColor::Orange, RingType::Normal), (TileColor::Green, RingType::Normal)],
				1,
			),
			dir.join("lane_braid_plus_normal.png"),
			"fulfilled braid stacks with the leftover orange normal match",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Zag),
				],
				1,
				&[
					(TileColor::Orange, RingType::Normal),
					(TileColor::Green, RingType::Normal),
					(TileColor::Blue, RingType::Normal),
				],
				1,
			),
			dir.join("lane_all_three.png"),
			"all three lanes: normal match, zag, fulfilled braid",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
				1,
			),
			dir.join("lane_braid_failed.png"),
			"failed braid: each side weaves its own colors into a midpoint diamond",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Blue, RingType::Normal)],
				1,
			),
			dir.join("lane_braid_failed_one_side.png"),
			"one-sided failed braid: only that braid reaches the diamond",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
				1,
			),
			dir.join("lane_braid_failed_both.png"),
			"both-sided failed braids: orange+green meets blue+teal at the diamond",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Zag),
				],
				1,
				&[(TileColor::Blue, RingType::Normal), (TileColor::Teal, RingType::Normal)],
				1,
			),
			dir.join("lane_braid_failed_plus_lanes.png"),
			"failed braid plus hitch and zag lanes",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[
					(TileColor::Orange, RingType::Bead),
					(TileColor::Orange, RingType::Bead),
					(TileColor::Orange, RingType::Bead),
				],
				1,
				&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
				1,
			),
			dir.join("lane_bead_bomb_vs_braid.png"),
			"bead lane: unmatched orange bead-chain bombs a non-matching braid",
			48.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Bead), (TileColor::Orange, RingType::Bead)],
				2,
				&[(TileColor::Orange, RingType::Normal)],
				1,
			),
			dir.join("lane_bead_match.png"),
			"bead match: two connected bead chains plus the orange normal highway",
			48.0,
		);
		write_png_sized(
			&demo_center_inner_pair(
				&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				1,
				&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
				1,
			),
			dir.join("center_petal_both_failed_braids.png"),
			"both inner↔center petals paint the failed-braid lane",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&demo_center_inner_pair(
				&[(TileColor::Orange, RingType::Normal)],
				1,
				&[(TileColor::Orange, RingType::Normal)],
				1,
			),
			dir.join("center_petal_both_normal_match.png"),
			"both inner↔center petals paint the match highway",
			1280,
			800,
		)
		.unwrap();
		let (flash_game, token_flash) = demo_midgame_capture_flash(CaptureDest::Tokens);
		write_png_view_flash(
			&flash_game,
			dir.join("capture_flash_tokens.png"),
			"freeze-frame of last captures between the token stacks",
			1280,
			800,
			None,
			Some(&token_flash),
		)
		.unwrap();
		let (flash_game, bench_flash) = demo_midgame_capture_flash(CaptureDest::Bench);
		write_png_view_flash(
			&flash_game,
			dir.join("capture_flash_bench.png"),
			"dot-capture landing glow on the mover bench",
			1280,
			800,
			None,
			Some(&bench_flash),
		)
		.unwrap();
		write_cropped(
			&demo_zag_tile(),
			dir.join("edge_capture_marks.png"),
			"dot/triangle marks in the bottom ring notch",
			28.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[(TileColor::Orange, RingType::Bead), (TileColor::Blue, RingType::Bead)],
				2,
			),
			dir.join("bead_ring_tile.png"),
			"two-bead tile: connected swelling/pinching chains",
			28.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[(TileColor::Orange, RingType::Bead), (TileColor::Blue, RingType::Bead)],
				2,
			),
			dir.join("two_bead_tile.png"),
			"two concentric bead rings, connected ribbon style",
			28.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[
					(TileColor::Blue, RingType::Bead),
					(TileColor::Orange, RingType::Bead),
					(TileColor::Teal, RingType::Bead),
				],
				3,
			),
			dir.join("three_bead_tile.png"),
			"three concentric bead rings, connected ribbon style",
			28.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[(TileColor::Blue, RingType::Bead), (TileColor::Blue, RingType::Bead)],
				2,
			),
			dir.join("double_blue_bead_tile.png"),
			"two concentric same-color bead rings stay distinguishable",
			28.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Blue, RingType::Bead), (TileColor::Blue, RingType::Bead)],
				2,
				&[
					(TileColor::Teal, RingType::Normal),
					(TileColor::Orange, RingType::Normal),
					(TileColor::Blue, RingType::Zag),
				],
				3,
			),
			dir.join("two_blue_beads_vs_teal_orange_blue_zag.png"),
			"2 blue beads vs teal+orange+blue zag: unmatched beads + hitch + zag",
			28.0,
		);
		write_cropped(
			&demo_typed_tile(
				&[
					(TileColor::Orange, RingType::Bead),
					(TileColor::Blue, RingType::Bead),
					(TileColor::Green, RingType::Normal),
				],
				3,
			),
			dir.join("bead_and_plate_tile.png"),
			"two connected bead chains with a bottom ring notch",
			28.0,
		);
		{
			let mut g = crate::game::Game::empty(RuleSet::default());
			let dot = g
				.alloc_typed_rings(
					Player::One,
					&[
						(TileColor::Orange, RingType::Bead),
						(TileColor::Blue, RingType::Bead),
						(TileColor::Green, RingType::Normal),
					],
					1,
					3,
				)
				.with_capture(CaptureMark::Dot);
			let tri = g
				.alloc_typed_rings(
					Player::One,
					&[
						(TileColor::Orange, RingType::Braid),
						(TileColor::Green, RingType::Braid),
						(TileColor::Blue, RingType::Zag),
					],
					1,
					3,
				)
				.with_capture(CaptureMark::Triangle);
			write_tile_closeup(dot, dir.join("closeup_notch_dot.png"), 880, 330.0);
			write_tile_closeup(tri, dir.join("closeup_notch_triangle.png"), 880, 330.0);
			for (tile, name) in emblem_closeup_tiles(&mut g) {
				write_tile_closeup(tile, dir.join(name), 880, 330.0);
			}
			write_cropped(
				&demo_typed_pair(
					&[(TileColor::Orange, RingType::Bead), (TileColor::Blue, RingType::Bead)],
					2,
					&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
					1,
				),
				dir.join("board_ring_notch.png"),
				"normal-zoom board crop: C-notch rings, no plate",
				36.0,
			);
			write_arc_bead_closeup(
				&demo_typed_pair(
					&[(TileColor::Orange, RingType::Bead), (TileColor::Blue, RingType::Bead)],
					2,
					&[(TileColor::Orange, RingType::Normal)],
					1,
				),
				dir.join("closeup_bead_arc.png"),
				dir.join("bead_arc_normal_zoom.png"),
			);
		}
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Zag)],
				1,
				&[(TileColor::Orange, RingType::Normal)],
				1,
			),
			dir.join("zag_vs_same_color_normal.png"),
			"orange zag vs orange normal: hitch + zag hit, no match credit",
			28.0,
		);
		write_cropped(
			&demo_typed_pair(
				&[(TileColor::Orange, RingType::Zag)],
				1,
				&[(TileColor::Orange, RingType::Bead), (TileColor::Orange, RingType::Bead)],
				2,
			),
			dir.join("zag_vs_same_color_bead.png"),
			"orange zag vs two orange beads: unmatched beads + zag hit, no match",
			28.0,
		);
		write_png_sized(
			&demo_zag_color_exclusion_board(),
			dir.join("zag_color_exclusion_board.png"),
			"board: zag vs same-color normal and bead; zag-on-zag still hits",
			1280,
			800,
		)
		.unwrap();
	}

	struct CharacterPreview {
		id: &'static str,
		label: &'static str,
		tile: crate::game::Tile,
	}

	fn character_preview_specs(game: &mut Game) -> Vec<CharacterPreview> {
		use crate::game::{Player, RingType, TileColor};
		let mut t = |owner, rings: &[(TileColor, RingType)], dots, capture, emblem| {
			game.alloc_typed_rings(owner, rings, rings.len() as u8, dots)
				.with_capture(capture)
				.with_emblem(emblem)
		};
		vec![
			CharacterPreview {
				id: "blue-diamond-lattice",
				label: "01 lattice",
				tile: t(
					Player::One,
					&[
						(TileColor::Green, RingType::Normal),
						(TileColor::Blue, RingType::Normal),
						(TileColor::Orange, RingType::Normal),
					],
					2,
					CaptureMark::Dot,
					Emblem::None,
				),
			},
			CharacterPreview {
				id: "green-pink-diamond-heart",
				label: "02 heart",
				tile: t(
					Player::Two,
					&[(TileColor::Green, RingType::Normal)],
					1,
					CaptureMark::Triangle,
					Emblem::CrescentMoon,
				),
			},
			CharacterPreview {
				id: "crossed-needles",
				label: "03 needles",
				tile: t(
					Player::One,
					&[
						(TileColor::Orange, RingType::Zag),
						(TileColor::Blue, RingType::Zag),
						(TileColor::Green, RingType::Normal),
					],
					3,
					CaptureMark::Dot,
					Emblem::TwoZagZs,
				),
			},
			CharacterPreview {
				id: "spirograph-diamond",
				label: "04 spiro",
				tile: t(
					Player::Two,
					&[(TileColor::Teal, RingType::Normal), (TileColor::Orange, RingType::Normal)],
					2,
					CaptureMark::Triangle,
					Emblem::None,
				),
			},
			CharacterPreview {
				id: "bird-of-paradise-pot",
				label: "05 bird",
				tile: t(
					Player::One,
					&[
						(TileColor::Blue, RingType::Normal),
						(TileColor::Green, RingType::Normal),
						(TileColor::Orange, RingType::Normal),
					],
					1,
					CaptureMark::Dot,
					Emblem::Blocker,
				),
			},
			CharacterPreview {
				id: "twin-star-comets",
				label: "06 comets",
				tile: t(
					Player::Two,
					&[
						(TileColor::Green, RingType::Braid),
						(TileColor::Blue, RingType::Braid),
						(TileColor::Orange, RingType::Normal),
					],
					2,
					CaptureMark::Triangle,
					Emblem::Rosette,
				),
			},
			CharacterPreview {
				id: "star-medallion",
				label: "07 star",
				tile: t(
					Player::One,
					&[
						(TileColor::Orange, RingType::Bead),
						(TileColor::Blue, RingType::Bead),
						(TileColor::Green, RingType::Normal),
					],
					3,
					CaptureMark::Dot,
					Emblem::None,
				),
			},
			CharacterPreview {
				id: "teal-four-point-star",
				label: "08 teal star",
				tile: t(
					Player::Two,
					&[(TileColor::Teal, RingType::Normal)],
					1,
					CaptureMark::Triangle,
					Emblem::CrescentMoon,
				),
			},
			CharacterPreview {
				id: "black-beaked-face",
				label: "09 beak",
				tile: t(
					Player::One,
					&[
						(TileColor::Maroon, RingType::Normal),
						(TileColor::Teal, RingType::Normal),
						(TileColor::Green, RingType::Normal),
					],
					2,
					CaptureMark::Dot,
					Emblem::None,
				),
			},
			CharacterPreview {
				id: "pink-claw-marks",
				label: "10 claws",
				tile: t(
					Player::Two,
					&[(TileColor::LavenderPink, RingType::Zag), (TileColor::Orange, RingType::Zag)],
					2,
					CaptureMark::Triangle,
					Emblem::TwoZagZs,
				),
			},
			CharacterPreview {
				id: "three-rosettes",
				label: "11 rosettes",
				tile: t(
					Player::One,
					&[
						(TileColor::Green, RingType::Braid),
						(TileColor::Blue, RingType::Braid),
						(TileColor::Orange, RingType::Braid),
					],
					1,
					CaptureMark::Dot,
					Emblem::Rosette,
				),
			},
			CharacterPreview {
				id: "petal-diamond-cluster",
				label: "12 petals",
				tile: t(
					Player::Two,
					&[(TileColor::Orange, RingType::Normal)],
					3,
					CaptureMark::Triangle,
					Emblem::Blocker,
				),
			},
			CharacterPreview {
				id: "lavender-square-spirograph",
				label: "13 lavender",
				tile: t(
					Player::One,
					&[
						(TileColor::LavenderPink, RingType::Normal),
						(TileColor::Teal, RingType::Normal),
					],
					1,
					CaptureMark::Dot,
					Emblem::None,
				),
			},
			CharacterPreview {
				id: "green-hatched-bowl",
				label: "14 bowl",
				tile: t(
					Player::Two,
					&[
						(TileColor::Green, RingType::Bead),
						(TileColor::Blue, RingType::Normal),
						(TileColor::Orange, RingType::Normal),
					],
					2,
					CaptureMark::Triangle,
					Emblem::None,
				),
			},
		]
	}

	fn load_preview_art(path: &std::path::Path) -> RgbaImage {
		let img = image::open(path).unwrap().to_rgba8();
		let max_dim = 768u32;
		let dim = img.width().max(img.height());
		if dim <= max_dim {
			return img;
		}
		let scale = max_dim as f32 / dim as f32;
		let w = (img.width() as f32 * scale).round().max(1.0) as u32;
		let h = (img.height() as f32 * scale).round().max(1.0) as u32;
		image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
	}

	fn write_tile_closeup_with_character(
		tile: crate::game::Tile,
		art: &RgbaImage,
		path: impl AsRef<std::path::Path>,
		size: u32,
		radius: f64,
	) {
		let bg = visual::rgb8(visual::BG_RGB);
		let mut img = image::RgbaImage::from_pixel(size, size, rgba(bg[0], bg[1], bg[2], 255));
		let c = (f64::from(size) * 0.5, f64::from(size) * 0.5);
		draw_tile_preview(&mut img, c, radius, tile, Some(art));
		img.save(path.as_ref()).unwrap();
	}

	fn write_character_contact_sheet(specs: &[CharacterPreview], arts: &[RgbaImage]) -> RgbaImage {
		let cols = 5u32;
		let rows = 3u32;
		let cell = 340u32;
		let label_h = 28u32;
		let pad = 16u32;
		let tile_r = 132.0;
		let w = pad + cols * (cell + pad);
		let h = pad + rows * (cell + label_h + pad);
		let bg = visual::rgb8(visual::BG_RGB);
		let mut img = image::RgbaImage::from_pixel(w, h, rgba(bg[0], bg[1], bg[2], 255));
		let label = rgba(230, 230, 220, 255);
		for (i, spec) in specs.iter().enumerate() {
			let r = (i as u32) / cols;
			let c = (i as u32) % cols;
			let x = pad + c * (cell + pad);
			let y = pad + r * (cell + label_h + pad);
			let cx = f64::from(x) + f64::from(cell) * 0.5;
			let cy = f64::from(y) + f64::from(cell) * 0.5;
			draw_tile_preview(&mut img, (cx, cy), tile_r, spec.tile, Some(&arts[i]));
			draw_text(&mut img, x + 8, y + cell + 6, spec.label, label);
		}
		img
	}

	fn empty_inner_pair(game: &Game) -> (NodeId, NodeId) {
		let board = game.board();
		let center = board.center();
		for id in 0..board.node_count() as NodeId {
			if game.tile_at(id).is_some() || board.is_spawn(id) || id == center {
				continue;
			}
			for &nb in board.neighbors(id) {
				if game.tile_at(nb).is_none() && !board.is_spawn(nb) && nb != center {
					return (id, nb);
				}
			}
		}
		panic!("expected an empty inner-ring neighbor pair");
	}

	fn character_neighbor_board<'a>(
		specs: &[CharacterPreview],
		arts: &'a [RgbaImage],
	) -> (Game, Vec<(NodeId, &'a RgbaImage)>) {
		let mut game = Game::empty(RuleSet::default());
		let (a, b) = empty_inner_pair(&game);
		game.place_tile(a, specs[0].tile);
		game.place_tile(b, specs[4].tile);
		(game, vec![(a, &arts[0]), (b, &arts[4])])
	}

	fn character_cluster_board<'a>(
		specs: &[CharacterPreview],
		arts: &'a [RgbaImage],
	) -> (Game, Vec<(NodeId, &'a RgbaImage)>) {
		let mut game = Game::empty(RuleSet::default());
		let board = game.board().clone();
		let center = board.center();
		game.place_tile(center, specs[8].tile);
		let mut overlays = vec![(center, &arts[8])];
		let picks = [1usize, 2, 6, 9, 11, 13];
		let mut pi = 0;
		for id in 0..board.node_count() as NodeId {
			if id == center || board.is_spawn(id) || game.tile_at(id).is_some() {
				continue;
			}
			if pi >= picks.len() {
				break;
			}
			let spec_i = picks[pi];
			game.place_tile(id, specs[spec_i].tile);
			overlays.push((id, &arts[spec_i]));
			pi += 1;
		}
		(game, overlays)
	}

	fn write_character_board_crop(
		game: &Game,
		characters: &[(NodeId, &RgbaImage)],
		path: impl AsRef<std::path::Path>,
		caption: &str,
		pad: f32,
	) {
		let w = 1280u32;
		let h = 800u32;
		let tmp = std::env::temp_dir().join("apa-kan-character-crop-src.png");
		write_png_view_preview(game, &tmp, caption, w, h, None, None, characters).unwrap();
		let img = image::open(&tmp).unwrap().to_rgba8();
		let layout = crate::visual::ViewLayout::from_window(w as f32, h as f32);
		let (x, y, cw, ch) = occupied_crop(game, &layout, pad, w, h);
		image::imageops::crop_imm(&img, x, y, cw, ch)
			.to_image()
			.save(path.as_ref())
			.unwrap();
	}

	fn write_tile_closeup(
		tile: crate::game::Tile,
		path: impl AsRef<std::path::Path>,
		size: u32,
		radius: f64,
	) {
		let bg = visual::rgb8(visual::BG_RGB);
		let mut img = image::RgbaImage::from_pixel(size, size, rgba(bg[0], bg[1], bg[2], 255));
		let c = (f64::from(size) * 0.5, f64::from(size) * 0.5);
		draw_tile(&mut img, c, radius, tile);
		img.save(path.as_ref()).unwrap();
	}

	fn write_arc_bead_closeup(
		game: &Game,
		closeup: impl AsRef<std::path::Path>,
		normal: impl AsRef<std::path::Path>,
	) {
		use crate::visual::ViewLayout;
		let layout = ViewLayout::from_window(1280.0, 800.0);
		let board = game.board();
		let mut line: Option<Vec<[f64; 2]>> = None;
		let mut color = crate::game::TileColor::Orange;
		for arc in board.arcs() {
			let paint = visual::arc_paint_with(
				game.tile_at(arc.from),
				game.tile_at(arc.to),
				game.rules().zag_mismatch,
			);
			if !paint.has_bead_lane() {
				continue;
			}
			let samples = arc.sample(visual::ARC_SAMPLES);
			let pts: Vec<[f64; 2]> = samples.iter().map(|p| image_pt(&layout, *p)).collect();
			let stripe_off = if visual::arc_touches_center(arc, board.center()) {
				f64::from(layout.stripe_off_center)
			} else {
				f64::from(layout.stripe_off)
			};
			let lanes = paint.lane_offsets(visual::channel_lane_pitch(stripe_off));
			let bead_lane = lanes[3].unwrap_or(0.0);
			line = Some(visual::offset_polyline(&pts, bead_lane));
			if let Some(c) = paint.bead_match {
				color = c;
			} else if let Some(c) = paint.beads_from.iter().copied().flatten().next() {
				color = c;
			}
			break;
		}
		let pts = line.expect("bead arc for close-up");
		let minx = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
		let maxx = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
		let miny = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
		let maxy = pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
		let bw = (maxx - minx).max(1.0);
		let bh = (maxy - miny).max(1.0);
		let mag = (920.0 / bw).min(380.0 / bh);
		let pad = 36.0;
		let w = (bw * mag + 2.0 * pad).ceil() as u32;
		let h = (bh * mag + 2.0 * pad).ceil() as u32;
		let mapped: Vec<[f64; 2]> = pts
			.iter()
			.map(|p| [(p[0] - minx) * mag + pad, (p[1] - miny) * mag + pad])
			.collect();
		let peak = f64::from(visual::arc_bead_radius(layout.arc_width)) * mag;
		let pinch = f64::from(visual::bead_pinch(visual::arc_bead_radius(layout.arc_width))) * mag;
		let spacing = visual::ARC_BEAD_SPACING_UNITS * f64::from(layout.scale) * mag;
		let bg = visual::rgb8(visual::BG_RGB);
		let mut img =
			image::RgbaImage::from_pixel(w.max(8), h.max(8), rgba(bg[0], bg[1], bg[2], 255));
		fill_tris(
			&mut img,
			&visual::bead_chain_triangles(&mapped, spacing, peak, pinch),
			rgb_of(visual::tile_rgb(color)),
		);
		img.save(closeup.as_ref()).unwrap();
		write_png_sized(
			game,
			&std::env::temp_dir().join("apa-kan-bead-arc-src.png"),
			"bead arc",
			1280,
			800,
		)
		.unwrap();
		let src = image::open(std::env::temp_dir().join("apa-kan-bead-arc-src.png"))
			.unwrap()
			.to_rgba8();
		let pad = 18.0;
		let nx = (minx - pad).max(0.0).floor() as u32;
		let ny = (miny - pad).max(0.0).floor() as u32;
		let nw =
			((maxx + pad).min(f64::from(src.width() - 1)) - f64::from(nx)).ceil().max(8.0) as u32;
		let nh =
			((maxy + pad).min(f64::from(src.height() - 1)) - f64::from(ny)).ceil().max(8.0) as u32;
		image::imageops::crop_imm(
			&src,
			nx,
			ny,
			nw.min(src.width() - nx),
			nh.min(src.height() - ny),
		)
		.to_image()
		.save(normal.as_ref())
		.unwrap();
	}

	fn write_cropped(game: &Game, path: impl AsRef<std::path::Path>, caption: &str, pad: f32) {
		let w = 1280u32;
		let h = 800u32;
		let tmp = std::env::temp_dir().join("apa-kan-crop-src.png");
		write_png_sized(game, &tmp, caption, w, h).unwrap();
		let img = image::open(&tmp).unwrap().to_rgba8();
		let layout = crate::visual::ViewLayout::from_window(w as f32, h as f32);
		let (x, y, cw, ch) = occupied_crop(game, &layout, pad, w, h);
		image::imageops::crop_imm(&img, x, y, cw, ch)
			.to_image()
			.save(path.as_ref())
			.unwrap();
	}

	fn occupied_crop(
		game: &Game,
		layout: &crate::visual::ViewLayout,
		pad: f32,
		width: u32,
		height: u32,
	) -> (u32, u32, u32, u32) {
		let board = game.board();
		let mut minx = f32::MAX;
		let mut miny = f32::MAX;
		let mut maxx = f32::MIN;
		let mut maxy = f32::MIN;
		let bump = |x: f32,
		            y: f32,
		            r: f32,
		            minx: &mut f32,
		            miny: &mut f32,
		            maxx: &mut f32,
		            maxy: &mut f32| {
			*minx = minx.min(x - r);
			*miny = miny.min(y - r);
			*maxx = maxx.max(x + r);
			*maxy = maxy.max(y + r);
		};
		for id in 0..board.node_count() as u8 {
			if game.tile_at(id).is_none() {
				continue;
			}
			let p = layout.board_to_image(board.position(id));
			bump(p[0], p[1], layout.tile_radius + pad, &mut minx, &mut miny, &mut maxx, &mut maxy);
		}
		for arc in board.arcs() {
			if game.tile_at(arc.from).is_none() && game.tile_at(arc.to).is_none() {
				continue;
			}
			for p in arc.sample(24) {
				let q = layout.board_to_image(p);
				bump(q[0], q[1], pad, &mut minx, &mut miny, &mut maxx, &mut maxy);
			}
		}
		if !minx.is_finite() {
			return (0, 0, width, height);
		}
		let x = minx.max(0.0).floor() as u32;
		let y = miny.max(0.0).floor() as u32;
		let x2 = maxx.min(width as f32).ceil() as u32;
		let y2 = maxy.min(height as f32).ceil() as u32;
		(x, y, x2.saturating_sub(x).max(1), y2.saturating_sub(y).max(1))
	}
}
