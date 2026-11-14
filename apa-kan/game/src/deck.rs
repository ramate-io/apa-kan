//! Deck-building: 27-tile collections, 7+4 public bench, ring-type cap, picks.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::ai::{evaluate, SearchLimit};
use crate::game::{sample_tile, Game, Player, RingType, RuleSet, Tile, BAG_SIZE, MAX_COLOR_RINGS};

/// Maximum collection size (historical 27 unique 3-color kinds).
pub const DECK_SIZE_MAX: usize = BAG_SIZE;
/// Default collection size. CLI `--deck-size` / `--bag-size`.
pub const DECK_SIZE_DEFAULT: usize = BAG_SIZE;
/// First simultaneous pick from the collection. Revealed to the opponent.
pub const BENCH_OPENING: usize = 7;
/// Second pick from reserves, made with knowledge of the opponent's 7.
pub const BENCH_EXTEND: usize = 4;
/// Public in-game bench. Replaces the old 11-tile public deck.
pub const BENCH_SIZE: usize = BENCH_OPENING + BENCH_EXTEND;
/// Max rings of each **non-normal** type (zag, braid, bead) on the
/// full 11-tile bench. Counted by **ring slots** (confirmed rule, not an
/// assumption): a 3-braid uses 3 of the 6. Normal rings are uncapped.
/// Enforced at the 7-pick and the +4 extension.
pub const RING_TYPE_CAP: u8 = 6;

/// How the 7-pick and +4 extension are chosen. Never uniform-random.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PickStrategy {
	/// Rank tiles, take a legal greedy set, then hill-climb under the work budget.
	#[default]
	Heuristic,
	/// [`Self::Heuristic`] plus short seeded rollouts on the top candidates.
	Search,
	/// Rank tiles and take the first legal set. No hill-climb, no rollouts.
	Greedy,
}

impl PickStrategy {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"heuristic" | "heur" => Ok(Self::Heuristic),
			"search" | "rollout" | "rollouts" => Ok(Self::Search),
			"greedy" => Ok(Self::Greedy),
			other => Err(format!("unknown --pick '{other}' (expected heuristic|search|greedy)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Heuristic => "heuristic",
			Self::Search => "search",
			Self::Greedy => "greedy",
		}
	}
}

/// Match-level deck generation and picking (not in-game RuleSet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetupConfig {
	/// RNG seed for collection sampling. `None` uses the game seed.
	pub deck_seed: Option<u64>,
	/// 0 = mirrored copy when `identical_bags`; 100 = every mirrored tile is
	/// independently resampled. Ignored when bags are already independent.
	pub deck_noise: u8,
	pub pick: PickStrategy,
	/// Caps hill-climb steps (`nodes`) and rollout count (`iters`).
	pub limit: SearchLimit,
}

impl Default for SetupConfig {
	fn default() -> Self {
		Self {
			deck_seed: None,
			deck_noise: 0,
			pick: PickStrategy::Heuristic,
			limit: SearchLimit::calibrated(),
		}
	}
}

impl SetupConfig {
	pub fn for_seed(seed: u64) -> Self {
		Self { deck_seed: Some(seed), ..Self::default() }
	}

	pub fn deck_seed(self, game_seed: u64) -> u64 {
		self.deck_seed.unwrap_or(game_seed)
	}
}

/// Reconstructible draft: full collections, the revealed 7s, and the +4s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupRecord {
	pub decks: [Vec<Tile>; 2],
	pub pick7: [Vec<Tile>; 2],
	pub extend: [Vec<Tile>; 2],
	pub strategy: PickStrategy,
	pub deck_seed: u64,
	pub deck_noise: u8,
}

impl SetupRecord {
	pub fn bench(&self, player: Player) -> Vec<Tile> {
		let mut out = self.pick7[player.index()].clone();
		out.extend_from_slice(&self.extend[player.index()]);
		out
	}

	pub fn reserves(&self, player: Player) -> Vec<Tile> {
		let used = self.bench(player);
		self.decks[player.index()]
			.iter()
			.copied()
			.filter(|t| !used.iter().any(|u| u.id == t.id))
			.collect()
	}
}

/// Opening 7 and +4 sizes for a collection of `deck_len` tiles.
pub fn pick_plan(deck_len: usize) -> (usize, usize) {
	let target = BENCH_SIZE.min(deck_len);
	let opening = BENCH_OPENING.min(target);
	(opening, target.saturating_sub(opening))
}

/// How many live ring slots of `typ` are on `tiles`. Braids count by slot
/// toward the cap (confirmed: a 3-braid is 3 of 6). Normal is counted too
/// but is not capped.
pub fn type_ring_count(tiles: &[Tile], typ: RingType) -> u8 {
	tiles
		.iter()
		.map(|t| {
			(0..t.n_rings.min(MAX_COLOR_RINGS))
				.filter(|&i| t.ring_type[i as usize] == typ)
				.count() as u8
		})
		.fold(0u8, |a, b| a.saturating_add(b))
}

pub fn zag_ring_count(tiles: &[Tile]) -> u8 {
	type_ring_count(tiles, RingType::Zag)
}

pub fn braid_ring_count(tiles: &[Tile]) -> u8 {
	type_ring_count(tiles, RingType::Braid)
}

pub fn bead_ring_count(tiles: &[Tile]) -> u8 {
	type_ring_count(tiles, RingType::Bead)
}

/// Non-normal types capped on the 11-tile bench: zag, braid, and bead.
/// Normal rings are never capped.
pub fn capped_ring_types() -> &'static [RingType] {
	&[RingType::Zag, RingType::Braid, RingType::Bead]
}

pub fn respects_type_cap(tiles: &[Tile]) -> bool {
	capped_overflow(tiles).is_none()
}

/// First capped type that exceeds [`RING_TYPE_CAP`], if any.
pub fn capped_overflow(tiles: &[Tile]) -> Option<(RingType, u8)> {
	for &typ in capped_ring_types() {
		let n = type_ring_count(tiles, typ);
		if n > RING_TYPE_CAP {
			return Some((typ, n));
		}
	}
	None
}

pub fn would_respect_cap(current: &[Tile], extra: Tile) -> bool {
	let mut v = Vec::with_capacity(current.len() + 1);
	v.extend_from_slice(current);
	v.push(extra);
	respects_type_cap(&v)
}

/// True if some `k`-subset of `tiles` stays under the per-type cap.
pub fn has_legal_subset(tiles: &[Tile], k: usize) -> bool {
	find_legal_subset(tiles, k).is_some()
}

/// Indices of a `k`-subset under the cap, preferring fewer special rings.
pub fn find_legal_subset(tiles: &[Tile], k: usize) -> Option<Vec<usize>> {
	let n = tiles.len();
	if k == 0 {
		return Some(Vec::new());
	}
	if k > n {
		return None;
	}
	if k == n {
		return respects_type_cap(tiles).then(|| (0..n).collect());
	}
	let cap = RING_TYPE_CAP as usize;
	let cap1 = cap + 1;
	let mut parent: Vec<Option<(usize, usize, usize, usize, usize)>> =
		vec![None; (n + 1) * (k + 1) * cap1 * cap1 * cap1];
	let idx = |i: usize, taken: usize, z: usize, b: usize, e: usize| {
		((((i * (k + 1) + taken) * cap1 + z) * cap1 + b) * cap1) + e
	};
	let mut reach = vec![false; parent.len()];
	reach[idx(0, 0, 0, 0, 0)] = true;
	for i in 0..n {
		let z_add = tiles[i].zag_ring_count() as usize;
		let b_add = tiles[i].braid_ring_count() as usize;
		let e_add = tiles[i].bead_ring_count() as usize;
		for taken in (0..=k.min(i)).rev() {
			for z in 0..=cap {
				for b in 0..=cap {
					for e in 0..=cap {
						if !reach[idx(i, taken, z, b, e)] {
							continue;
						}
						let skip = idx(i + 1, taken, z, b, e);
						if !reach[skip] {
							reach[skip] = true;
							parent[skip] = Some((i, taken, z, b, e));
						}
						if taken < k && z + z_add <= cap && b + b_add <= cap && e + e_add <= cap {
							let take = idx(i + 1, taken + 1, z + z_add, b + b_add, e + e_add);
							if !reach[take] {
								reach[take] = true;
								parent[take] = Some((i, taken, z, b, e));
							}
						}
					}
				}
			}
		}
	}
	let mut end = None;
	'find: for z in 0..=cap {
		for b in 0..=cap {
			for e in 0..=cap {
				if reach[idx(n, k, z, b, e)] {
					end = Some((k, z, b, e));
					break 'find;
				}
			}
		}
	}
	let (mut taken, mut z, mut b, mut e) = end?;
	let mut i = n;
	let mut out = Vec::with_capacity(k);
	while i > 0 {
		let Some((pi, pt, pz, pb, pe)) = parent[idx(i, taken, z, b, e)] else {
			break;
		};
		if pt < taken {
			out.push(pi);
		}
		i -= 1;
		taken = pt;
		z = pz;
		b = pb;
		e = pe;
	}
	out.reverse();
	(out.len() == k).then_some(out)
}

/// Sample a viable `size`-tile collection from the ring/color/zag/braid/bead process.
pub fn generate_collection(
	owner: Player,
	size: usize,
	rules: &RuleSet,
	next_id: &mut u32,
	rng: &mut impl Rng,
) -> Vec<Tile> {
	let size = size.clamp(1, DECK_SIZE_MAX);
	let (_, target) = pick_plan(size);
	let want = BENCH_SIZE.min(size).max(target);
	for _ in 0..128 {
		let mut deck = Vec::with_capacity(size);
		for _ in 0..size {
			deck.push(sample_tile(
				owner,
				rules.max_rings,
				rules.zag_numer,
				rules.braid_numer,
				rules.bead_numer,
				rules.range_numer,
				next_id,
				rng,
			));
		}
		if has_legal_subset(&deck, want) {
			return deck;
		}
	}
	repair_collection(owner, size, rules, next_id, rng, want)
}

fn repair_collection(
	owner: Player,
	size: usize,
	rules: &RuleSet,
	next_id: &mut u32,
	rng: &mut impl Rng,
	want: usize,
) -> Vec<Tile> {
	let mut deck = Vec::with_capacity(size);
	for _ in 0..size {
		deck.push(sample_tile(
			owner,
			rules.max_rings,
			rules.zag_numer,
			rules.braid_numer,
			rules.bead_numer,
			rules.range_numer,
			next_id,
			rng,
		));
	}
	let mut guard = 0u32;
	while !has_legal_subset(&deck, want) && guard < 64 {
		guard += 1;
		let Some(i) = deck
			.iter()
			.enumerate()
			.max_by_key(|(_, t)| {
				(t.zag_ring_count() + t.braid_ring_count() + t.bead_ring_count(), t.id)
			})
			.map(|(i, _)| i)
		else {
			break;
		};
		for slot in deck[i].ring_type.iter_mut() {
			if !slot.is_normal() {
				*slot = RingType::Normal;
				break;
			}
		}
	}
	deck
}

fn mirror_tile(src: Tile, owner: Player, next_id: &mut u32) -> Tile {
	let id = *next_id;
	*next_id += 1;
	Tile { id, owner, ..src }
}

/// Apply `--deck-noise`: independently resample that many percent of tiles.
pub fn apply_deck_noise(
	base: &[Tile],
	owner: Player,
	noise: u8,
	rules: &RuleSet,
	next_id: &mut u32,
	rng: &mut impl Rng,
) -> Vec<Tile> {
	let noise = noise.min(100);
	if noise == 0 {
		return base.iter().copied().map(|t| mirror_tile(t, owner, next_id)).collect();
	}
	let mut out = Vec::with_capacity(base.len());
	for &src in base {
		if rng.gen_range(0..100u8) < noise {
			out.push(sample_tile(
				owner,
				rules.max_rings,
				rules.zag_numer,
				rules.braid_numer,
				rules.bead_numer,
				rules.range_numer,
				next_id,
				rng,
			));
		} else {
			out.push(mirror_tile(src, owner, next_id));
		}
	}
	let want = BENCH_SIZE.min(out.len());
	if !has_legal_subset(&out, want) {
		if let Some(idx) = find_legal_subset(base, want) {
			for i in idx {
				if i < out.len() {
					out[i] = mirror_tile(base[i], owner, next_id);
				}
			}
		}
	}
	out
}

pub fn generate_both_collections(
	rules: &RuleSet,
	setup: &SetupConfig,
	game_seed: u64,
	next_id: &mut u32,
) -> [Vec<Tile>; 2] {
	let size = (rules.deck_size as usize).clamp(1, DECK_SIZE_MAX);
	let mut rng = StdRng::seed_from_u64(setup.deck_seed(game_seed) ^ 0xDEC0);
	let p1 = generate_collection(Player::One, size, rules, next_id, &mut rng);
	let p2 = if rules.identical_bags {
		apply_deck_noise(&p1, Player::Two, setup.deck_noise, rules, next_id, &mut rng)
	} else {
		generate_collection(Player::Two, size, rules, next_id, &mut rng)
	};
	[p1, p2]
}

fn kind_key(t: Tile) -> (u8, u8, [u8; 3], [u8; 3], [bool; 3]) {
	(t.n_rings, t.dots, t.rings.map(|c| c as u8), t.ring_type.map(|r| r as u8), t.ring_range)
}

fn tile_score(tile: Tile) -> i32 {
	let mut s = 24 * i32::from(tile.weight) + 8 * i32::from(tile.dots);
	s += 6 * i32::from(tile.normal_ring_count());
	s += 4 * i32::from(tile.zag_ring_count());
	s += 5 * i32::from(tile.braid_ring_count());
	s += 6 * i32::from(tile.bead_ring_count());
	s += 5 * i32::from(tile.range_ring_count());
	for &c in tile.ring_colors() {
		s += match c as u8 {
			5 => 18,
			3 | 4 => 8,
			_ => 2,
		};
	}
	s
}

fn set_score(tiles: &[Tile], opponent: &[Tile]) -> i32 {
	let mut s: i32 = tiles.iter().map(|t| tile_score(*t)).sum();
	let mut colors = [false; 6];
	let mut dots = [false; 4];
	for t in tiles {
		dots[t.dots.clamp(1, 3) as usize] = true;
		for &c in t.ring_colors() {
			colors[c as usize] = true;
		}
	}
	s += 18 * colors.iter().filter(|c| **c).count() as i32;
	s += 10 * dots.iter().skip(1).filter(|d| **d).count() as i32;
	let zag = i32::from(zag_ring_count(tiles));
	let braid = i32::from(braid_ring_count(tiles));
	let bead = i32::from(bead_ring_count(tiles));
	let cap = i32::from(RING_TYPE_CAP);
	s += 8 * (cap - zag).max(0);
	s += 8 * (cap - braid).max(0);
	s += 8 * (cap - bead).max(0);
	if !opponent.is_empty() {
		for &mine in tiles {
			for &opp in opponent {
				s += 12 * mine.zag_hits_on(opp);
				s += 10 * opp.zag_hits_on(mine);
				s += 11 * i32::from(mine.bead_unmatched_on(opp));
				s += 9 * i32::from(opp.bead_unmatched_on(mine));
				if mine.bead_matched_on(opp) > 0 || opp.bead_matched_on(mine) > 0 {
					s += 5;
				}
				if mine.shares_normal(opp) {
					s += 6;
				}
				if mine.braid_fulfilled_by(opp).is_some() || opp.braid_fulfilled_by(mine).is_some()
				{
					s += 14;
				}
				if mine.has_range() {
					s += 8 * i32::from(mine.range_ring_count());
					s += 7 * mine.ranged_zag_hits_on(opp);
					s += 8 * i32::from(mine.ranged_bead_unmatched_on(opp));
				}
				if opp.has_range() {
					s += 6 * i32::from(opp.range_ring_count());
				}
				if !mine.shares_color(opp) {
					s += 3;
				}
			}
		}
	}
	s
}

fn combo_tiles(pool: &[Tile], idx: &[usize]) -> Vec<Tile> {
	idx.iter().map(|&i| pool[i]).collect()
}

fn work_nodes(limit: SearchLimit) -> u64 {
	match limit {
		SearchLimit::Work { nodes, .. } => nodes.max(1),
		SearchLimit::Time(d) => ((d.as_millis() as u64) / 4).clamp(32, 8_000),
	}
}

fn work_iters(limit: SearchLimit) -> u32 {
	match limit {
		SearchLimit::Work { iters, .. } => iters.max(1),
		SearchLimit::Time(d) => ((d.as_millis() as u32) / 80).clamp(2, 24),
	}
}

fn greedy_indices(
	pool: &[Tile],
	k: usize,
	prefix: &[Tile],
	need_extend: usize,
) -> Option<Vec<usize>> {
	if k == 0 {
		return Some(Vec::new());
	}
	let mut order: Vec<usize> = (0..pool.len()).collect();
	order.sort_by(|a, b| {
		tile_score(pool[*b])
			.cmp(&tile_score(pool[*a]))
			.then_with(|| kind_key(pool[*a]).cmp(&kind_key(pool[*b])))
			.then_with(|| a.cmp(b))
	});
	let mut chosen = Vec::new();
	for &i in &order {
		if chosen.len() >= k {
			break;
		}
		let mut trial = prefix.to_vec();
		trial.extend(combo_tiles(pool, &chosen));
		trial.push(pool[i]);
		if !respects_type_cap(&trial) {
			continue;
		}
		let mut rest: Vec<Tile> = Vec::new();
		for (j, t) in pool.iter().enumerate() {
			if j == i || chosen.contains(&j) {
				continue;
			}
			rest.push(*t);
		}
		if need_extend > 0 && !has_legal_prefixed(&trial, &rest, need_extend) {
			continue;
		}
		chosen.push(i);
	}
	(chosen.len() == k).then_some(chosen)
}

fn has_legal_prefixed(prefix: &[Tile], rest: &[Tile], k: usize) -> bool {
	if k == 0 {
		return respects_type_cap(prefix);
	}
	let mut all = prefix.to_vec();
	all.extend_from_slice(rest);
	let _ = prefix;
	if k > rest.len() {
		return respects_type_cap(&all);
	}
	let cap = RING_TYPE_CAP as usize;
	let pz = zag_ring_count(prefix) as usize;
	let pb = braid_ring_count(prefix) as usize;
	let pe = bead_ring_count(prefix) as usize;
	if pz > cap || pb > cap || pe > cap {
		return false;
	}
	let n = rest.len();
	let cap1 = cap + 1;
	let mut reach = vec![false; (k + 1) * cap1 * cap1 * cap1];
	let idx =
		|taken: usize, z: usize, b: usize, e: usize| ((taken * cap1 + z) * cap1 + b) * cap1 + e;
	reach[idx(0, pz, pb, pe)] = true;
	for tile in rest {
		let za = tile.zag_ring_count() as usize;
		let ba = tile.braid_ring_count() as usize;
		let ea = tile.bead_ring_count() as usize;
		for taken in (0..k).rev() {
			for z in 0..=cap {
				for b in 0..=cap {
					for e in 0..=cap {
						if !reach[idx(taken, z, b, e)] {
							continue;
						}
						if z + za <= cap && b + ba <= cap && e + ea <= cap {
							reach[idx(taken + 1, z + za, b + ba, e + ea)] = true;
						}
					}
				}
			}
		}
	}
	for z in 0..=cap {
		for b in 0..=cap {
			for e in 0..=cap {
				if reach[idx(k.min(n), z, b, e)] {
					return true;
				}
			}
		}
	}
	false
}

fn enumerate_limit(n: usize, k: usize) -> u64 {
	if k > n {
		return 0;
	}
	let k = k.min(n - k);
	let mut c = 1u64;
	for i in 0..k {
		c = c.saturating_mul((n - i) as u64) / (i as u64 + 1);
	}
	c
}

fn for_each_combination(n: usize, k: usize, mut f: impl FnMut(&[usize]) -> bool) {
	if k == 0 {
		let _ = f(&[]);
		return;
	}
	if k > n {
		return;
	}
	let mut idx: Vec<usize> = (0..k).collect();
	loop {
		if !f(&idx) {
			return;
		}
		let mut i = k;
		while i > 0 {
			i -= 1;
			if idx[i] != i + n - k {
				idx[i] += 1;
				for j in i + 1..k {
					idx[j] = idx[j - 1] + 1;
				}
				break;
			}
			if i == 0 {
				return;
			}
		}
	}
}

fn best_enumerated(
	pool: &[Tile],
	k: usize,
	prefix: &[Tile],
	opponent: &[Tile],
	need_extend: usize,
	max_combos: u64,
) -> Option<Vec<usize>> {
	let n = pool.len();
	if enumerate_limit(n, k) > max_combos {
		return None;
	}
	let mut best: Option<(i32, Vec<usize>)> = None;
	for_each_combination(n, k, |idx| {
		let mut trial = prefix.to_vec();
		trial.extend(combo_tiles(pool, idx));
		if !respects_type_cap(&trial) {
			return true;
		}
		if need_extend > 0 {
			let chosen: Vec<usize> = idx.to_vec();
			let rest: Vec<Tile> = pool
				.iter()
				.enumerate()
				.filter(|(j, _)| !chosen.contains(j))
				.map(|(_, t)| *t)
				.collect();
			if !has_legal_prefixed(&trial, &rest, need_extend) {
				return true;
			}
		}
		let score = set_score(&trial, opponent);
		let cand = idx.to_vec();
		match &best {
			None => best = Some((score, cand)),
			Some((s, prev)) if score > *s || (score == *s && cand < *prev) => {
				best = Some((score, cand));
			}
			_ => {}
		}
		true
	});
	best.map(|(_, idx)| idx)
}

fn hill_climb(
	pool: &[Tile],
	start: Vec<usize>,
	k: usize,
	prefix: &[Tile],
	opponent: &[Tile],
	need_extend: usize,
	nodes: u64,
) -> Vec<usize> {
	let n = pool.len();
	let mut best = start;
	best.sort_unstable();
	let score_of = |idx: &[usize]| {
		let mut trial = prefix.to_vec();
		trial.extend(combo_tiles(pool, idx));
		set_score(&trial, opponent)
	};
	let legal = |idx: &[usize]| {
		let mut trial = prefix.to_vec();
		trial.extend(combo_tiles(pool, idx));
		if !respects_type_cap(&trial) {
			return false;
		}
		if need_extend == 0 {
			return true;
		}
		let rest: Vec<Tile> = pool
			.iter()
			.enumerate()
			.filter(|(j, _)| !idx.contains(j))
			.map(|(_, t)| *t)
			.collect();
		has_legal_prefixed(&trial, &rest, need_extend)
	};
	let mut best_score = score_of(&best);
	let mut steps = 0u64;
	loop {
		let mut improved = false;
		for s in 0..k {
			for cand in 0..n {
				if steps >= nodes {
					return best;
				}
				if best.contains(&cand) {
					continue;
				}
				steps += 1;
				let mut trial = best.clone();
				trial[s] = cand;
				trial.sort_unstable();
				if !legal(&trial) {
					continue;
				}
				let sc = score_of(&trial);
				if sc > best_score || (sc == best_score && trial < best) {
					best = trial;
					best_score = sc;
					improved = true;
				}
			}
		}
		if !improved {
			break;
		}
	}
	best
}

fn pick_indices(
	pool: &[Tile],
	k: usize,
	prefix: &[Tile],
	opponent: &[Tile],
	need_extend: usize,
	strategy: PickStrategy,
	limit: SearchLimit,
) -> Vec<usize> {
	if k == 0 || pool.is_empty() {
		return Vec::new();
	}
	let k = k.min(pool.len());
	let nodes = work_nodes(limit);
	if let Some(idx) = best_enumerated(pool, k, prefix, opponent, need_extend, nodes.max(8_000)) {
		return idx;
	}
	let greedy = greedy_indices(pool, k, prefix, need_extend)
		.or_else(|| find_legal_subset(pool, k))
		.unwrap_or_else(|| (0..k).collect());
	if strategy == PickStrategy::Greedy {
		return greedy;
	}
	hill_climb(pool, greedy, k, prefix, opponent, need_extend, nodes)
}

fn rollout_bonus(
	own: &[Tile],
	opp: &[Tile],
	me: Player,
	rules: RuleSet,
	seed: u64,
	iters: u32,
) -> i32 {
	if iters == 0 {
		return 0;
	}
	let mut sum = 0i32;
	let _ = me;
	for i in 0..iters {
		let mut game = Game::empty(rules);
		let mut mine = own.to_vec();
		let mut theirs = opp.to_vec();
		for t in mine.iter_mut() {
			t.owner = Player::One;
		}
		for t in theirs.iter_mut() {
			t.owner = Player::Two;
		}
		game.set_bench(Player::One, mine);
		game.set_bench(Player::Two, theirs);
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(seed ^ (u64::from(i) + 1).wrapping_mul(0x9E37));
		for _ in 0..4 {
			if game.status().is_over() {
				break;
			}
			let legal = game.legal_actions();
			if legal.is_empty() {
				break;
			}
			let action = legal[rng.gen_range(0..legal.len())];
			if game.apply(action).is_err() {
				break;
			}
		}
		sum += evaluate(&game, Player::One, 0);
	}
	sum / (iters.max(1) as i32)
}

/// Pick `k` tiles from `pool` as `player`, given already-committed `prefix`
/// (empty on the 7-pick; the revealed 7 on the extension) and the opponent's
/// revealed 7 (`opponent`, empty during the simultaneous 7-pick).
pub fn pick_from_pool(
	pool: &[Tile],
	k: usize,
	prefix: &[Tile],
	opponent: &[Tile],
	need_extend: usize,
	player: Player,
	rules: &RuleSet,
	strategy: PickStrategy,
	limit: SearchLimit,
	seed: u64,
) -> Vec<Tile> {
	let mut idx = pick_indices(pool, k, prefix, opponent, need_extend, strategy, limit);
	if strategy == PickStrategy::Search && k > 0 && !idx.is_empty() {
		let iters = work_iters(limit);
		let mut best = idx.clone();
		let mut best_s = {
			let mut trial = prefix.to_vec();
			trial.extend(combo_tiles(pool, &best));
			set_score(&trial, opponent)
				+ rollout_bonus(&trial, opponent, player, *rules, seed, iters)
		};
		let n = pool.len();
		let k = idx.len();
		let mut steps = 0u64;
		let cap_steps = work_nodes(limit).min(256);
		for s in 0..k {
			for cand in 0..n {
				if steps >= cap_steps {
					break;
				}
				if best.contains(&cand) {
					continue;
				}
				steps += 1;
				let mut trial_idx = best.clone();
				trial_idx[s] = cand;
				trial_idx.sort_unstable();
				let mut trial = prefix.to_vec();
				trial.extend(combo_tiles(pool, &trial_idx));
				if !respects_type_cap(&trial) {
					continue;
				}
				let sc = set_score(&trial, opponent)
					+ rollout_bonus(&trial, opponent, player, *rules, seed ^ cand as u64, iters);
				if sc > best_s || (sc == best_s && trial_idx < best) {
					best = trial_idx;
					best_s = sc;
				}
			}
		}
		idx = best;
	}
	combo_tiles(pool, &idx)
}

/// Simultaneous 7-picks (no opponent knowledge), then +4 with the revealed 7s.
pub fn pick_setup(decks: &[Vec<Tile>; 2], rules: &RuleSet, setup: &SetupConfig) -> SetupRecord {
	let seed = setup.deck_seed.unwrap_or(0);
	if decks[0].len() <= BENCH_SIZE && decks[1].len() <= BENCH_SIZE {
		let mut pick7 = [Vec::new(), Vec::new()];
		let mut extend_picks = [Vec::new(), Vec::new()];
		for p in [Player::One, Player::Two] {
			let deck = &decks[p.index()];
			let (opening, ext) = pick_plan(deck.len());
			pick7[p.index()] = deck.iter().copied().take(opening).collect();
			extend_picks[p.index()] = deck.iter().copied().skip(opening).take(ext).collect();
		}
		return SetupRecord {
			decks: decks.clone(),
			pick7,
			extend: extend_picks,
			strategy: setup.pick,
			deck_seed: seed,
			deck_noise: setup.deck_noise,
		};
	}
	let mut pick7 = [Vec::new(), Vec::new()];
	for p in [Player::One, Player::Two] {
		let deck = &decks[p.index()];
		let (opening, need_ext) = pick_plan(deck.len());
		pick7[p.index()] = pick_from_pool(
			deck,
			opening,
			&[],
			&[],
			need_ext,
			p,
			rules,
			setup.pick,
			setup.limit,
			seed ^ 0x0707,
		);
	}
	let mut extend_picks = [Vec::new(), Vec::new()];
	for p in [Player::One, Player::Two] {
		let deck = &decks[p.index()];
		let mine7 = &pick7[p.index()];
		let opp7 = &pick7[p.other().index()];
		let reserves: Vec<Tile> =
			deck.iter().copied().filter(|t| !mine7.iter().any(|u| u.id == t.id)).collect();
		let k = pick_plan(deck.len()).1;
		extend_picks[p.index()] = pick_from_pool(
			&reserves,
			k,
			mine7,
			opp7,
			0,
			p,
			rules,
			setup.pick,
			setup.limit,
			seed ^ 0xE4E4,
		);
	}
	SetupRecord {
		decks: decks.clone(),
		pick7,
		extend: extend_picks,
		strategy: setup.pick,
		deck_seed: seed,
		deck_noise: setup.deck_noise,
	}
}

/// Run generation + picking. Returns the record; caller installs benches/reserves.
pub fn deal_and_pick(
	rules: &RuleSet,
	setup: &SetupConfig,
	game_seed: u64,
	next_id: &mut u32,
) -> SetupRecord {
	let decks = generate_both_collections(rules, setup, game_seed, next_id);
	let mut cfg = *setup;
	if cfg.deck_seed.is_none() {
		cfg.deck_seed = Some(game_seed);
	}
	pick_setup(&decks, rules, &cfg)
}

/// Draft from already-materialized collections (scenario files).
pub fn pick_from_collections(
	decks: [Vec<Tile>; 2],
	rules: &RuleSet,
	setup: &SetupConfig,
	game_seed: u64,
) -> SetupRecord {
	let mut cfg = *setup;
	if cfg.deck_seed.is_none() {
		cfg.deck_seed = Some(game_seed);
	}
	pick_setup(&decks, rules, &cfg)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{TileColor, STARTING_TOKENS_DEFAULT};

	fn zag_tile(id: u32, n_zag: u8) -> Tile {
		let n = n_zag.clamp(1, 3);
		let parts: Vec<(TileColor, RingType)> = (0..n)
			.map(|i| {
				let c = TileColor::ALL[i as usize];
				(c, RingType::Zag)
			})
			.collect();
		Tile::with_typed_rings(id, Player::One, &parts, n, 1)
	}

	fn braid_tile(id: u32, n_braid: u8) -> Tile {
		let n = n_braid.clamp(2, 3);
		let parts: Vec<(TileColor, RingType)> = (0..n)
			.map(|i| {
				let c = TileColor::ALL[i as usize];
				(c, RingType::Braid)
			})
			.collect();
		Tile::with_typed_rings(id, Player::One, &parts, n, 1)
	}

	fn bead_tile(id: u32, n_bead: u8) -> Tile {
		let n = n_bead.clamp(1, 3);
		let parts: Vec<(TileColor, RingType)> = (0..n)
			.map(|i| {
				let c = TileColor::ALL[i as usize];
				(c, RingType::Bead)
			})
			.collect();
		Tile::with_typed_rings(id, Player::One, &parts, n, 1)
	}

	fn normal_tile(id: u32, color: TileColor) -> Tile {
		Tile::with_typed_rings(id, Player::One, &[(color, RingType::Normal)], 1, 1)
	}

	#[test]
	fn braid_slots_count_toward_the_cap() {
		let seven: Vec<Tile> = (0..7).map(|i| braid_tile(i + 1, 3)).collect();
		assert_eq!(braid_ring_count(&seven), 21);
		assert!(capped_overflow(&seven).is_some());
		let two = vec![braid_tile(1, 3), braid_tile(2, 3)];
		assert_eq!(braid_ring_count(&two), 6);
		assert!(respects_type_cap(&two));
		let over = vec![braid_tile(1, 3), braid_tile(2, 3), braid_tile(3, 2)];
		assert_eq!(braid_ring_count(&over), 8);
		assert_eq!(capped_overflow(&over), Some((RingType::Braid, 8)));
	}

	#[test]
	fn zag_cap_is_six_rings_not_six_tiles() {
		let six_zags: Vec<Tile> = (0..6).map(|i| zag_tile(i + 1, 1)).collect();
		assert!(respects_type_cap(&six_zags));
		let seven_zags: Vec<Tile> = (0..7).map(|i| zag_tile(i + 1, 1)).collect();
		assert_eq!(capped_overflow(&seven_zags), Some((RingType::Zag, 7)));
		let two_triples = vec![zag_tile(1, 3), zag_tile(2, 3)];
		assert!(respects_type_cap(&two_triples));
		let three_triples = vec![zag_tile(1, 3), zag_tile(2, 3), zag_tile(3, 3)];
		assert_eq!(capped_overflow(&three_triples), Some((RingType::Zag, 9)));
	}

	#[test]
	fn normals_are_uncapped_and_caps_are_independent() {
		let normals: Vec<Tile> =
			(0..11).map(|i| normal_tile(i + 1, TileColor::ALL[i as usize % 6])).collect();
		assert!(respects_type_cap(&normals));
		assert_eq!(zag_ring_count(&normals), 0);
		assert_eq!(braid_ring_count(&normals), 0);
		let mixed = vec![
			zag_tile(1, 3),
			zag_tile(2, 3),
			braid_tile(3, 3),
			braid_tile(4, 3),
			normal_tile(5, TileColor::Green),
		];
		assert_eq!(zag_ring_count(&mixed), 6);
		assert_eq!(braid_ring_count(&mixed), 6);
		assert!(respects_type_cap(&mixed));
	}

	#[test]
	fn bead_cap_is_six_rings_not_six_tiles() {
		let six: Vec<Tile> = (0..6).map(|i| bead_tile(i + 1, 1)).collect();
		assert!(respects_type_cap(&six));
		assert_eq!(bead_ring_count(&six), 6);
		let seven: Vec<Tile> = (0..7).map(|i| bead_tile(i + 1, 1)).collect();
		assert_eq!(capped_overflow(&seven), Some((RingType::Bead, 7)));
		let two_triples = vec![bead_tile(1, 3), bead_tile(2, 3)];
		assert!(respects_type_cap(&two_triples));
		let three_triples = vec![bead_tile(1, 3), bead_tile(2, 3), bead_tile(3, 3)];
		assert_eq!(capped_overflow(&three_triples), Some((RingType::Bead, 9)));
		let mixed = vec![
			zag_tile(1, 3),
			zag_tile(2, 3),
			braid_tile(3, 3),
			braid_tile(4, 3),
			bead_tile(5, 3),
			bead_tile(6, 3),
		];
		assert_eq!(zag_ring_count(&mixed), 6);
		assert_eq!(braid_ring_count(&mixed), 6);
		assert_eq!(bead_ring_count(&mixed), 6);
		assert!(respects_type_cap(&mixed));
		assert!(!would_respect_cap(&mixed, bead_tile(7, 1)));
		assert!(would_respect_cap(&six, zag_tile(7, 2)));
	}

	#[test]
	fn extension_that_would_exceed_cap_is_illegal() {
		let seven: Vec<Tile> = (0..6)
			.map(|i| zag_tile(i + 1, 1))
			.chain(std::iter::once(normal_tile(7, TileColor::Green)))
			.collect();
		assert!(respects_type_cap(&seven));
		assert!(!would_respect_cap(&seven, zag_tile(8, 1)));
		assert!(would_respect_cap(&seven, normal_tile(8, TileColor::Blue)));
		assert!(would_respect_cap(&seven, braid_tile(8, 2)));
	}

	#[test]
	fn pick_plan_is_seven_then_four() {
		assert_eq!(pick_plan(27), (7, 4));
		assert_eq!(pick_plan(11), (7, 4));
		assert_eq!(pick_plan(8), (7, 1));
		assert_eq!(pick_plan(5), (5, 0));
		assert_eq!(pick_plan(1), (1, 0));
	}

	#[test]
	fn default_deal_is_27_then_11_bench() {
		let rules = RuleSet::default();
		assert_eq!(rules.deck_size as usize, DECK_SIZE_DEFAULT);
		assert_eq!(rules.starting_bench_size, STARTING_TOKENS_DEFAULT);
		let mut next = 1u32;
		let setup = SetupConfig::for_seed(1);
		let rec = deal_and_pick(&rules, &setup, 1, &mut next);
		assert_eq!(rec.decks[0].len(), 27);
		assert_eq!(rec.decks[1].len(), 27);
		assert_eq!(rec.pick7[0].len(), 7);
		assert_eq!(rec.pick7[1].len(), 7);
		assert_eq!(rec.extend[0].len(), 4);
		assert_eq!(rec.extend[1].len(), 4);
		let b1 = rec.bench(Player::One);
		let b2 = rec.bench(Player::Two);
		assert_eq!(b1.len(), 11);
		assert_eq!(b2.len(), 11);
		assert!(respects_type_cap(&b1));
		assert!(respects_type_cap(&rec.pick7[0]));
		assert_eq!(rec.reserves(Player::One).len(), 16);
		let kinds = |tiles: &[Tile]| tiles.iter().map(|t| kind_key(*t)).collect::<Vec<_>>();
		assert_eq!(kinds(&rec.decks[0]), kinds(&rec.decks[1]));
		assert_eq!(kinds(&b1), kinds(&b2));
	}

	#[test]
	fn picking_is_deterministic_per_seed() {
		let rules = RuleSet::default();
		let setup = SetupConfig::for_seed(42);
		let mut a_id = 1u32;
		let mut b_id = 1u32;
		let a = deal_and_pick(&rules, &setup, 42, &mut a_id);
		let b = deal_and_pick(&rules, &setup, 42, &mut b_id);
		assert_eq!(a, b);
	}

	#[test]
	fn noise_makes_independent_collections() {
		let mut rules = RuleSet::default();
		rules.identical_bags = true;
		let mut setup = SetupConfig::for_seed(9);
		setup.deck_noise = 100;
		let mut next = 1u32;
		let rec = deal_and_pick(&rules, &setup, 9, &mut next);
		let k1: Vec<_> = rec.decks[0].iter().map(|t| kind_key(*t)).collect();
		let k2: Vec<_> = rec.decks[1].iter().map(|t| kind_key(*t)).collect();
		assert_ne!(k1, k2, "100% noise should resample the mirrored copy");
	}

	#[test]
	fn greedy_and_heuristic_never_break_the_cap() {
		let rules = RuleSet::default();
		for (pick, seed) in
			[(PickStrategy::Greedy, 3u64), (PickStrategy::Heuristic, 5), (PickStrategy::Search, 7)]
		{
			let mut setup = SetupConfig::for_seed(seed);
			setup.pick = pick;
			setup.limit = SearchLimit::work(48, 2);
			let mut next = 1u32;
			let rec = deal_and_pick(&rules, &setup, seed, &mut next);
			assert!(respects_type_cap(&rec.pick7[0]), "{pick:?} 7-pick");
			assert!(respects_type_cap(&rec.bench(Player::One)), "{pick:?} 11-bench");
			assert!(respects_type_cap(&rec.bench(Player::Two)), "{pick:?} P2 11-bench");
		}
	}

	#[test]
	fn parse_pick_strategy() {
		assert_eq!(PickStrategy::parse("heuristic").unwrap(), PickStrategy::Heuristic);
		assert_eq!(PickStrategy::parse("search").unwrap(), PickStrategy::Search);
		assert_eq!(PickStrategy::parse("greedy").unwrap(), PickStrategy::Greedy);
		assert!(PickStrategy::parse("random").is_err());
	}
}
