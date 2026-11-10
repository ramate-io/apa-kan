//! Data-driven geometric board graphs.
//!
//! The default board is an 8-petal rosette: eight equal circles that all pass
//! through the origin, with centers equally spaced on a circle of the same
//! radius. Nodes are the origin plus every pairwise intersection (deduped with
//! a tolerance). Edges are the circular arcs between consecutive nodes along
//! each generating circle.

use std::f64::consts::PI;

pub type NodeId = u8;

/// One generating circle of a board.
#[derive(Clone, Debug)]
pub struct CircleSpec {
	pub center: [f64; 2],
	pub radius: f64,
}

/// A directed CCW arc along one generating circle, from `from` to `to`.
#[derive(Clone, Debug)]
pub struct ArcEdge {
	pub circle: u8,
	pub from: NodeId,
	pub to: NodeId,
	pub center: [f64; 2],
	pub radius: f64,
	pub angle_from: f64,
	pub angle_to: f64,
}

impl ArcEdge {
	/// Sample the arc, travelling CCW from `from` to `to`.
	pub fn sample(&self, n: usize) -> Vec<[f64; 2]> {
		let n = n.max(2);
		let mut sweep = self.angle_to - self.angle_from;
		if sweep <= 0.0 {
			sweep += 2.0 * PI;
		}
		(0..n)
			.map(|i| {
				let t = i as f64 / (n - 1) as f64;
				let a = self.angle_from + sweep * t;
				[self.center[0] + self.radius * a.cos(), self.center[1] + self.radius * a.sin()]
			})
			.collect()
	}
}

/// A concrete board: nodes, positions, unique adjacency, circle-arc edges, and
/// spawn nodes. Other shapes can be built with [`Board::from_circles`].
#[derive(Clone, Debug)]
pub struct Board {
	name: String,
	positions: Vec<[f64; 2]>,
	neighbors: Vec<Vec<NodeId>>,
	incident_arcs: Vec<Vec<usize>>,
	arcs: Vec<ArcEdge>,
	circles: Vec<CircleSpec>,
	circle_cycles: Vec<Vec<NodeId>>,
	spawns: Vec<NodeId>,
	center: NodeId,
	construction_radius: f64,
}

impl Board {
	/// Eight equal circles through the origin, centers at radius `radius` and
	/// angles `offset + k * 45°`. The default offset places an outer petal tip
	/// on the +Y axis so the layout matches the reference drawing.
	pub fn rosette_8() -> Self {
		Self::rosette_8_with(1.0, 67.5_f64.to_radians())
	}

	pub fn rosette_8_with(radius: f64, offset: f64) -> Self {
		assert!(radius > 0.0, "rosette radius must be positive");
		let mut circles = Vec::with_capacity(8);
		for k in 0..8 {
			let theta = offset + (k as f64) * (PI / 4.0);
			circles
				.push(CircleSpec { center: [radius * theta.cos(), radius * theta.sin()], radius });
		}
		Self::from_circles("rosette-8", &circles, radius * 1e-8_f64.max(1e-12))
	}

	/// Build a board from generating circles. Nodes are all pairwise
	/// intersections, deduped within `tolerance`. Spawn nodes are the unique
	/// ring of 8 nodes at the middle positive radius (the yellow ring on the
	/// 8-petal rosette). Other geometries may set spawns after construction
	/// via [`Board::with_spawns`].
	pub fn from_circles(name: &str, circles: &[CircleSpec], tolerance: f64) -> Self {
		let tolerance = tolerance.max(1e-12);
		let mut positions: Vec<[f64; 2]> = Vec::new();
		push_dedup(&mut positions, [0.0, 0.0], tolerance);

		for i in 0..circles.len() {
			for j in (i + 1)..circles.len() {
				for p in circle_circle_intersections(&circles[i], &circles[j], tolerance) {
					push_dedup(&mut positions, p, tolerance);
				}
			}
		}

		// Canonical order: center first, then angle, then radius. This makes
		// 8-fold rotation a simple permutation of ids.
		let center_idx = nearest(&positions, [0.0, 0.0]);
		let center_pos = positions[center_idx];
		positions.swap(0, center_idx);
		let rest = &mut positions[1..];
		rest.sort_by(|a, b| {
			let aa = atan2_pos(*a);
			let ba = atan2_pos(*b);
			match aa.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal) {
				std::cmp::Ordering::Equal => {
					norm(*a).partial_cmp(&norm(*b)).unwrap_or(std::cmp::Ordering::Equal)
				}
				o => o,
			}
		});
		let _ = center_pos;

		let n = positions.len();
		let mut circle_cycles = Vec::with_capacity(circles.len());
		let mut arcs = Vec::new();
		for (ci, circle) in circles.iter().enumerate() {
			let mut on: Vec<NodeId> = (0..n)
				.filter(|&i| {
					(dist(positions[i], circle.center) - circle.radius).abs() <= tolerance * 8.0
				})
				.map(|i| i as NodeId)
				.collect();
			on.sort_by(|&a, &b| {
				let aa = atan2_about(positions[a as usize], circle.center);
				let ba = atan2_about(positions[b as usize], circle.center);
				aa.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
			});
			if on.len() >= 2 {
				for k in 0..on.len() {
					let from = on[k];
					let to = on[(k + 1) % on.len()];
					let angle_from = atan2_about(positions[from as usize], circle.center);
					let mut angle_to = atan2_about(positions[to as usize], circle.center);
					if angle_to <= angle_from {
						angle_to += 2.0 * PI;
					}
					arcs.push(ArcEdge {
						circle: ci as u8,
						from,
						to,
						center: circle.center,
						radius: circle.radius,
						angle_from,
						angle_to,
					});
				}
			}
			circle_cycles.push(on);
		}

		let mut neighbors = vec![Vec::new(); n];
		let mut incident_arcs = vec![Vec::new(); n];
		for (ai, arc) in arcs.iter().enumerate() {
			incident_arcs[arc.from as usize].push(ai);
			incident_arcs[arc.to as usize].push(ai);
			if !neighbors[arc.from as usize].contains(&arc.to) {
				neighbors[arc.from as usize].push(arc.to);
			}
			if !neighbors[arc.to as usize].contains(&arc.from) {
				neighbors[arc.to as usize].push(arc.from);
			}
		}

		let construction_radius = circles.first().map(|c| c.radius).unwrap_or(1.0);
		let spawns = identify_spawn_ring(&positions, construction_radius, tolerance);

		Self {
			name: name.to_string(),
			positions,
			neighbors,
			incident_arcs,
			arcs,
			circles: circles.to_vec(),
			circle_cycles,
			spawns,
			center: 0,
			construction_radius,
		}
	}

	pub fn with_spawns(mut self, spawns: Vec<NodeId>) -> Self {
		self.spawns = spawns;
		self
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn node_count(&self) -> usize {
		self.positions.len()
	}

	pub fn center(&self) -> NodeId {
		self.center
	}

	/// Unique neighbors of the center — the inner-ring attachments of the
	/// eight petals. Majority-of-petals reads this set (5 of 8 on the default
	/// rosette).
	pub fn inner_ring(&self) -> &[NodeId] {
		self.neighbors(self.center)
	}

	pub fn construction_radius(&self) -> f64 {
		self.construction_radius
	}

	pub fn position(&self, id: NodeId) -> [f64; 2] {
		self.positions[id as usize]
	}

	pub fn positions(&self) -> &[[f64; 2]] {
		&self.positions
	}

	pub fn neighbors(&self, id: NodeId) -> &[NodeId] {
		&self.neighbors[id as usize]
	}

	pub fn incident_arc_count(&self, id: NodeId) -> usize {
		self.incident_arcs[id as usize].len()
	}

	pub fn incident_arcs(&self, id: NodeId) -> &[usize] {
		&self.incident_arcs[id as usize]
	}

	pub fn arcs(&self) -> &[ArcEdge] {
		&self.arcs
	}

	pub fn circles(&self) -> &[CircleSpec] {
		&self.circles
	}

	pub fn circle_cycle(&self, circle: u8) -> &[NodeId] {
		&self.circle_cycles[circle as usize]
	}

	pub fn circle_count(&self) -> usize {
		self.circles.len()
	}

	pub fn spawns(&self) -> &[NodeId] {
		&self.spawns
	}

	pub fn is_spawn(&self, id: NodeId) -> bool {
		self.spawns.contains(&id)
	}

	/// Index of the unique generating-circle arc whose endpoints are `a` and `b`
	/// (either direction) on `circle`. Used for no-edge-revisit MOVE paths.
	pub fn undirected_arc_index(&self, circle: u8, a: NodeId, b: NodeId) -> Option<usize> {
		self.arcs.iter().position(|arc| {
			arc.circle == circle
				&& ((arc.from == a && arc.to == b) || (arc.from == b && arc.to == a))
		})
	}

	pub fn next_on_circle(&self, circle: u8, node: NodeId, ccw: bool) -> Option<NodeId> {
		let cycle = self.circle_cycles.get(circle as usize)?;
		let i = cycle.iter().position(|&n| n == node)?;
		let len = cycle.len();
		if len == 0 {
			return None;
		}
		if ccw {
			Some(cycle[(i + 1) % len])
		} else {
			Some(cycle[(i + len - 1) % len])
		}
	}

	pub fn circles_through(&self, node: NodeId) -> impl Iterator<Item = u8> + '_ {
		self.circle_cycles.iter().enumerate().filter_map(move |(i, cycle)| {
			if cycle.contains(&node) {
				Some(i as u8)
			} else {
				None
			}
		})
	}

	pub fn radius_of(&self, id: NodeId) -> f64 {
		norm(self.positions[id as usize])
	}

	/// Map each node to its image under a rotation about the origin.
	pub fn rotation_map(&self, radians: f64, tolerance: f64) -> Option<Vec<NodeId>> {
		let mut map = vec![0u8; self.node_count()];
		let mut used = vec![false; self.node_count()];
		for i in 0..self.node_count() {
			let p = rotate(self.positions[i], radians);
			let j = nearest(&self.positions, p);
			if dist(self.positions[j], p) > tolerance {
				return None;
			}
			if used[j] {
				return None;
			}
			used[j] = true;
			map[i] = j as NodeId;
		}
		Some(map)
	}
}

fn identify_spawn_ring(positions: &[[f64; 2]], radius: f64, tolerance: f64) -> Vec<NodeId> {
	// Positive radii clustered; the middle cluster is the yellow spawn ring.
	let mut radii: Vec<f64> =
		positions.iter().map(|p| norm(*p)).filter(|r| *r > tolerance * 10.0).collect();
	radii.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
	let mut clusters: Vec<f64> = Vec::new();
	for r in radii {
		if clusters.last().map(|c| (r - c).abs() > radius * 0.05).unwrap_or(true) {
			clusters.push(r);
		}
	}
	if clusters.len() < 2 {
		return Vec::new();
	}
	let mid = clusters[clusters.len() / 2];
	positions
		.iter()
		.enumerate()
		.filter(|(_, p)| (norm(**p) - mid).abs() <= radius * 0.05)
		.map(|(i, _)| i as NodeId)
		.collect()
}

fn circle_circle_intersections(a: &CircleSpec, b: &CircleSpec, tolerance: f64) -> Vec<[f64; 2]> {
	let dx = b.center[0] - a.center[0];
	let dy = b.center[1] - a.center[1];
	let d = (dx * dx + dy * dy).sqrt();
	if d <= tolerance {
		return Vec::new();
	}
	if d > a.radius + b.radius + tolerance {
		return Vec::new();
	}
	if d < (a.radius - b.radius).abs() - tolerance {
		return Vec::new();
	}
	let alpha = (a.radius * a.radius - b.radius * b.radius + d * d) / (2.0 * d);
	let h2 = a.radius * a.radius - alpha * alpha;
	let px = a.center[0] + alpha * dx / d;
	let py = a.center[1] + alpha * dy / d;
	if h2.abs() <= tolerance * tolerance {
		return vec![[px, py]];
	}
	if h2 < 0.0 {
		return Vec::new();
	}
	let h = h2.sqrt();
	vec![[px - h * dy / d, py + h * dx / d], [px + h * dy / d, py - h * dx / d]]
}

fn push_dedup(points: &mut Vec<[f64; 2]>, p: [f64; 2], tolerance: f64) {
	if points.iter().any(|q| dist(*q, p) <= tolerance) {
		return;
	}
	points.push(p);
}

fn nearest(points: &[[f64; 2]], p: [f64; 2]) -> usize {
	let mut best = 0;
	let mut best_d = f64::INFINITY;
	for (i, q) in points.iter().enumerate() {
		let d = dist(*q, p);
		if d < best_d {
			best_d = d;
			best = i;
		}
	}
	best
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
	let dx = a[0] - b[0];
	let dy = a[1] - b[1];
	(dx * dx + dy * dy).sqrt()
}

fn norm(p: [f64; 2]) -> f64 {
	(p[0] * p[0] + p[1] * p[1]).sqrt()
}

fn atan2_pos(p: [f64; 2]) -> f64 {
	let a = p[1].atan2(p[0]);
	if a < 0.0 {
		a + 2.0 * PI
	} else {
		a
	}
}

fn atan2_about(p: [f64; 2], c: [f64; 2]) -> f64 {
	let a = (p[1] - c[1]).atan2(p[0] - c[0]);
	if a < 0.0 {
		a + 2.0 * PI
	} else {
		a
	}
}

fn rotate(p: [f64; 2], radians: f64) -> [f64; 2] {
	let (s, c) = radians.sin_cos();
	[p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

	fn board() -> Board {
		Board::rosette_8()
	}

	#[test]
	fn rosette_has_25_nodes_and_8_circles() {
		let b = board();
		assert_eq!(b.node_count(), 25, "center + 8 inner + 8 spawn + 8 outer");
		assert_eq!(b.circle_count(), 8);
		assert_eq!(b.center(), 0);
		assert!(b.radius_of(0) < 1e-9);
		for cycle in 0..8 {
			assert_eq!(b.circle_cycle(cycle as u8).len(), 7, "each circle has 7 nodes");
		}
		assert_eq!(b.arcs().len(), 56, "8 circles × 7 CCW arcs");
	}

	#[test]
	fn non_center_nodes_have_four_incident_arcs() {
		let b = board();
		assert_eq!(b.incident_arc_count(b.center()), 16, "center lies on all 8 circles");
		assert_eq!(b.neighbors(b.center()).len(), 8);
		for id in 1..b.node_count() as NodeId {
			assert_eq!(
				b.incident_arc_count(id),
				4,
				"node {id} r={:.4} should have 4 incident arcs",
				b.radius_of(id)
			);
		}
	}

	#[test]
	fn inner_ring_has_double_arc_to_center() {
		let b = board();
		let r = b.construction_radius();
		let inner: Vec<NodeId> = (0..b.node_count() as NodeId)
			.filter(|&id| (b.radius_of(id) - 2.0 * r * (3.0 * PI / 8.0).cos()).abs() < 0.05 * r)
			.collect();
		assert_eq!(inner.len(), 8);
		for id in inner {
			assert_eq!(b.neighbors(id).len(), 3, "unique neighbors: two spawns + center");
			assert!(b.neighbors(id).contains(&b.center()));
			let to_center = b
				.arcs()
				.iter()
				.filter(|a| {
					(a.from == id && a.to == b.center()) || (a.to == id && a.from == b.center())
				})
				.count();
			assert_eq!(to_center, 2, "two distinct circle-arcs to the center");
		}
	}

	#[test]
	fn eight_spawns_form_an_independent_set() {
		let b = board();
		assert_eq!(b.spawns(), &[2, 5, 8, 11, 14, 17, 20, 23]);
		let r = b.construction_radius();
		let expected = (2.0_f64).sqrt() * r;
		for &s in b.spawns() {
			assert!((b.radius_of(s) - expected).abs() < 0.05 * r, "spawn on middle ring");
			assert_ne!(s, b.center());
			for &n in b.neighbors(s) {
				assert!(!b.is_spawn(n), "no two spawns are adjacent");
			}
		}
		// Spawns are 8-fold: one every 45°.
		let mut angles: Vec<f64> = b.spawns().iter().map(|&s| atan2_pos(b.position(s))).collect();
		angles.sort_by(|a, c| a.partial_cmp(c).unwrap());
		for w in angles.windows(2) {
			let d = w[1] - w[0];
			assert!((d - PI / 4.0).abs() < 1e-6, "spawns evenly spaced, got {d}");
		}
	}

	#[test]
	fn eight_fold_rotational_symmetry() {
		let b = board();
		let tol = b.construction_radius() * 1e-6;
		for k in 1..8 {
			let map = b
				.rotation_map((k as f64) * PI / 4.0, tol * 20.0)
				.expect("rotation should map nodes to nodes");
			assert_eq!(map[b.center() as usize], b.center());
			for id in 0..b.node_count() as NodeId {
				let rid = map[id as usize];
				assert_eq!(b.is_spawn(id), b.is_spawn(rid));
				let mut nbs: Vec<NodeId> =
					b.neighbors(id).iter().map(|&n| map[n as usize]).collect();
				let mut rnbs: Vec<NodeId> = b.neighbors(rid).to_vec();
				nbs.sort();
				rnbs.sort();
				assert_eq!(nbs, rnbs, "adjacency preserved under {k}*45° for node {id}");
			}
		}
	}

	#[test]
	fn undirected_arc_index_distinguishes_double_center_edges() {
		let b = board();
		let inner = (1..b.node_count() as NodeId)
			.find(|&id| b.neighbors(id).contains(&b.center()))
			.expect("an inner node");
		let mut idxs = Vec::new();
		for circle in b.circles_through(inner) {
			if let Some(i) = b.undirected_arc_index(circle, inner, b.center()) {
				idxs.push(i);
			}
		}
		idxs.sort();
		idxs.dedup();
		assert_eq!(idxs.len(), 2, "two distinct circle-arcs between inner and center");
	}

	#[test]
	fn pairwise_intersections_are_deduped() {
		let b = board();
		let r = b.construction_radius();
		let mut radii: Vec<f64> = (0..b.node_count() as NodeId).map(|id| b.radius_of(id)).collect();
		radii.sort_by(|a, c| a.partial_cmp(c).unwrap());
		let unique = {
			let mut u = Vec::new();
			for rad in radii {
				if u.last().map(|c: &f64| (rad - *c).abs() > 0.05 * r).unwrap_or(true) {
					u.push(rad);
				}
			}
			u
		};
		assert_eq!(unique.len(), 4, "center + three rings");
	}
}
