# RFC 000-000-00n: Tile generation

Follow-up to [rfc-001-001-007-character-library](../rfc-001-001-007-character-library/README.md). That RFC catalogs twenty-five character ids; this one assigns a character to a generated tile.

## Problem

`sample_tile_with` already rolls rings, capture marks, and an emblem. Characters exist as PNG ids in `CharacterLibrary` but are not attached to `Tile`. Assignment needs to be deterministic, versioned, and authorable: a few exact lore maps, a soft aesthetic default, and a leftover generic set — without training a model or requiring an admin to fill every axis.

## Proposed solution

Two top-level passes:

- **Core pass:** rings, markings, and emblems.
- **Character pass:** maps the finished core tile to a catalog character.

The character pass is a pure function of `(core tile, tile.id, pass version)`. It does not affect overwhelm, search `kind_index`, or deck cost.

```
core tile
  → exact CoreKey in the specific map?
        yes → authored set, pick tile.id % len
  → else neighborhood { c | cosine(tile, c) ≥ τ } minus fallback-reserved
        non-empty → sort by id, pick tile.id % len
        empty     → fallback set, hash(tile.id) % len
```

## Core Pass

- After the ID is generated, the core pass fills rings, markings, and emblem.
- The pass is a readable decision tree. Each step is a weighted `choose` (or a coin) seeded from `id.mix(step)` so the same id always replays.
- Parameters live on one **`CorePass`** value. `CorePass::base()` is the version's published distribution; rarity is always quoted against that, not against event skew.
- Any other `CorePass` is a **reward table** or a **collector**: generating a 27-tile collection with it hands the AI (or a player) a skewed bag. This is the same idea as today's `DeckArchetype`, but one struct instead of a handful of numerators.

```rust
/// One versioned generator. Event rewards and collectors are other values of this.
struct CorePass {
    version: u16,
    rings: RingPass,
    colors: ColorPass,
    markings: MarkingPass,
    emblems: EmblemPass,
}

struct CoreTile {
    ring_type: Vec<RingType>,   // length n_rings, slot order inner→outer
    rings: Vec<TileColor>,      // parallel
    dots: u8,                   // MOVE length 1..=3
    capture: CaptureMark,       // Dot | Triangle
    emblem: Emblem,             // None | CrescentMoon | TwoZagZs | Blocker | Rosette
}

struct TileId(u32);

impl TileId {
    fn mix(self, step: impl Hash) -> Self { /* domain-separated child of `self` */ }

    /// Weighted draw. Zero and negative weights floor to 0; at least one must stay positive.
    fn choose<T: Copy>(self, from: &[T], weights: &[i32]) -> T { /* ... */ }

    /// `choose` on `[false, true]` with `[light, heavy]`-style weights.
    fn sample(self, weights: &[u32]) -> bool {
        Self::choose(self, &[false, true], &[weights[0] as i32, weights[1] as i32])
    }
}

impl CorePass {
    fn base() -> Self { /* fields below; rarity is computed from this */ }

    fn run(id: TileId, cfg: &CorePass) -> CoreTile {
        let ring_type = RingPass::run(id, &cfg.rings);
        let rings = ColorPass::run(id, &cfg.colors, ring_type.len());
        let (dots, capture) = MarkingPass::run(id, &cfg.markings, &ring_type);
        let emblem = EmblemPass::run(id, &cfg.emblems, &ring_type, dots, capture);
        CoreTile { ring_type, rings, dots, capture, emblem }
    }

    /// Example collector: more consecutive heavies resolve as braid.
    fn braid_collector() -> Self {
        let mut cfg = Self::base();
        cfg.rings.heavy_weights = [1, 1];            // 50% heavy per slot
        cfg
    }
}
```

### Ring pass

```rust
struct RingPass {
    /// Weights for n_rings = 1, 2, 3.
    count_weights: [u32; 3],
    /// Per slot: [light, heavy]. Heavy is **braid only**.
    heavy_weights: [u32; 2],
    /// Leftover slots, including a lone / gapped heavy coin: [normal, zag, bead].
    light_weights: [u32; 3],
}

impl RingPass {
    fn base() -> Self {
        Self {
            count_weights: [5, 3, 1],
            heavy_weights: [3, 1],        // 1 in 4 heavy (braid group)
            light_weights: [5, 3, 1],    // normal : zag : bead
        }
    }

    fn consecutive(idx: &[usize]) -> bool {
        idx.windows(2).all(|w| w[1] == w[0] + 1)
    }

    fn run(id: TileId, cfg: &RingPass) -> Vec<RingType> {
        let n = TileId::choose(id.mix("n"), &[1u8, 2, 3], &cfg.count_weights.map(|x| x as i32));
        let heavy: Vec<usize> = (0..n as usize)
            .filter(|&i| TileId::sample(id.mix(("h", i)), &cfg.heavy_weights))
            .collect();

        let mut types = vec![RingType::Normal; n as usize];

        if heavy.len() >= 2 && RingPass::consecutive(&heavy) {
            for &i in &heavy {
                types[i] = RingType::Braid;
            }
        }

        // Lone or gapped heavy coins fall through to light (single beads allowed).
        for i in 0..n as usize {
            let in_group = heavy.len() >= 2 && RingPass::consecutive(&heavy) && heavy.contains(&i);
            if !in_group {
                types[i] = TileId::choose(
                    id.mix(("lite", i)),
                    &[RingType::Normal, RingType::Zag, RingType::Bead],
                    &cfg.light_weights.map(|x| x as i32),
                );
            }
        }
        types
    }
}
```

| Step | Live type | Base weights | Odds |
|---|---|---|---|
| Ring count | `n_rings` 1 / 2 / 3 | 5 : 3 : 1 | 5/9 · 1/3 · 1/9 |
| Per slot | light / heavy (braid) | 3 : 1 | 3/4 · 1/4 |
| Consecutive heavies (≥2) | `Braid` | — | always |
| Light slot | `Normal` / `Zag` / `Bead` | 5 : 3 : 1 | 5/9 · 1/3 · 1/9 |

Live `sample_tile_with` IndependentMax: per-slot heavy coin `braid_numer / 7`; consecutive ≥2 heavies → braid; lone/gapped heavies fall through to 5:3:1 light (single beads allowed). `--channel-mode additive` restores piece-level 1/7 braid occupying the first 2–3 slots, then 2–3 bead groups. See [`rfc-001-001-008`](../rfc-001-001-008-independent-max-channels/README.md).

### Color pass

```rust
struct ColorPass {
    /// Green, Blue, Orange, LavenderPink, Teal, Maroon.
    weights: [u32; 6],
    /// After drawing color i, add `drift[i]` to the running weights (hysteresis).
    /// Base is zero: independent draws. Events/collectors raise the diagonal to cluster.
    drift: [[i32; 6]; 6],
}

impl ColorPass {
    const COLORS: [TileColor; 6] = [
        TileColor::Green,
        TileColor::Blue,
        TileColor::Orange,
        TileColor::LavenderPink,
        TileColor::Teal,
        TileColor::Maroon,
    ];

    fn base() -> Self {
        Self {
            weights: [81, 81, 81, 9, 9, 3],
            drift: [[0; 6]; 6],
        }
    }

    fn run(id: TileId, cfg: &ColorPass, n: usize) -> Vec<TileColor> {
        let mut w: [i32; 6] = cfg.weights.map(|x| x as i32);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let color = TileId::choose(id.mix(("c", i)), &ColorPass::COLORS, &w);
            out.push(color);
            for (j, d) in cfg.drift[color as usize].iter().enumerate() {
                w[j] += d;
            }
        }
        out
    }
}
```

| `TileColor` | Base weight | Odds (total 264) |
|---|---|---|
| Green / Blue / Orange | 81 | 81/264 ≈ 30.7% each |
| LavenderPink / Teal | 9 | 9/264 ≈ 3.4% each |
| Maroon | 3 | 3/264 ≈ 1.1% |

Base drift is all zeros. A maroon-collector might set `drift[Maroon][Maroon] = +40`; a “keep last color” event might add a large diagonal after each draw.

### Marking pass

```rust
struct MarkingPass {
    /// Weights for dots = 1, 2, 3 (MOVE length).
    count_weights: [u32; 3],
    /// [triangle, dot].
    capture_weights: [u32; 2],
    /// Per `RingType` (Normal, Zag, Braid, Bead): row = min(count, 2), col = dots 1/2/3.
    depreciations: [[[i32; 3]; 3]; 4],
}

impl MarkingPass {
    fn base() -> Self {
        let tax = [
            [0, 0, 0],
            [0, -1, -2],
            [0, -2, -4],
        ];
        Self {
            count_weights: [17, 11, 7],
            capture_weights: [1, 1],
            depreciations: [tax; 4],
        }
    }

    fn run(id: TileId, cfg: &MarkingPass, ring_type: &[RingType]) -> (u8, CaptureMark) {
        let mut w: [i32; 3] = cfg.count_weights.map(|x| x as i32);
        for typ in [RingType::Normal, RingType::Zag, RingType::Braid, RingType::Bead] {
            let n = ring_type.iter().filter(|t| **t == typ).count().min(2);
            for (j, d) in cfg.depreciations[typ as usize][n].iter().enumerate() {
                w[j] += d;
            }
        }
        let dots = TileId::choose(id.mix("dots"), &[1u8, 2, 3], &w);
        let capture = TileId::choose(
            id.mix("cap"),
            &[CaptureMark::Triangle, CaptureMark::Dot],
            &cfg.capture_weights.map(|x| x as i32),
        );
        (dots, capture)
    }
}
```

Busy faces (more rings of a type) tax long MOVE. Capture shape does not.

| `dots` | Base weight | Odds before tax |
|---|---|---|
| 1 | 17 | 17/35 |
| 2 | 11 | 11/35 |
| 3 | 7 | 7/35 = 1/5 |

| `CaptureMark` | Base weight | Odds | Rule |
|---|---|---|---|
| Triangle | 1 | 1/2 | captures → spawn tokens |
| Dot | 1 | 1/2 | captures → mover's bench |

Depreciation rows (same table for every `RingType` in the base). Row = how many rings of that type (0, 1, 2+). Col = added to the 1 / 2 / 3-mark weights.

| Count of type | Δ 1-mark | Δ 2-mark | Δ 3-mark |
|---|---|---|---|
| 0 | 0 | 0 | 0 |
| 1 | 0 | −1 | −2 |
| 2+ | 0 | −2 | −4 |

Taxes **sum** across types, then floor at 0.

### Emblem pass

```rust
struct EmblemPass {
    /// None, CrescentMoon, TwoZagZs, Blocker, Rosette.
    weights: [u32; 5],
    /// Per ring type, row = min(count, 2), col = emblem outcome.
    ring_depreciations: [[[i32; 5]; 3]; 4],
    /// Per capture (Triangle, Dot), row = dots-1, col = emblem outcome.
    marking_depreciations: [[[i32; 5]; 3]; 2],
}

impl EmblemPass {
    const EMBLEMS: [Emblem; 5] = [
        Emblem::None,
        Emblem::CrescentMoon,
        Emblem::TwoZagZs,
        Emblem::Blocker,
        Emblem::Rosette,
    ];

    fn base() -> Self {
        let ring_tax = [
            [0, 0, 0, 0, 0],
            [0, -1, -1, -1, -1],
            [0, -2, -2, -2, -2],
        ];
        let mark_tax = ring_tax;
        Self {
            weights: [197, 7, 7, 7, 7],
            ring_depreciations: [ring_tax; 4],
            marking_depreciations: [mark_tax; 2],
        }
    }

    fn run(
        id: TileId,
        cfg: &EmblemPass,
        ring_type: &[RingType],
        dots: u8,
        capture: CaptureMark,
    ) -> Emblem {
        let mut w: [i32; 5] = cfg.weights.map(|x| x as i32);
        for typ in [RingType::Normal, RingType::Zag, RingType::Braid, RingType::Bead] {
            let n = ring_type.iter().filter(|t| **t == typ).count().min(2);
            for (j, d) in cfg.ring_depreciations[typ as usize][n].iter().enumerate() {
                w[j] += d;
            }
        }
        let cap_i = capture as usize; // Triangle = 0, Dot = 1
        let dot_i = (dots.clamp(1, 3) - 1) as usize;
        for (j, d) in cfg.marking_depreciations[cap_i][dot_i].iter().enumerate() {
            w[j] += d;
        }
        TileId::choose(id.mix("emb"), &EmblemPass::EMBLEMS, &w)
    }
}
```

Busy rings and long MOVE tax **glyphs**, not `None`.

| `Emblem` | Base weight | Odds before tax |
|---|---|---|
| `None` | 197 | 197/225 ≈ 87.6% |
| `CrescentMoon` | 7 | 7/225 ≈ 3.1% |
| `TwoZagZs` | 7 | 7/225 |
| `Blocker` | 7 | 7/225 |
| `Rosette` | 7 | 7/225 |

Ring tax (same for every `RingType`) and marking tax (same for `Dot` / `Triangle`):

| Count or `dots` | Δ None | Δ each glyph |
|---|---|---|
| 0 rings / 1 mark | 0 | 0 |
| 1 ring of type / 2 marks | 0 | −1 |
| 2+ rings of type / 3 marks | 0 | −2 |

### Rewards and collectors

`CorePass::base()` is what players see on the rarity chart. Anything else is a named preset of the same struct:

| Use | How |
|---|---|
| Event reward | e.g. raise `colors.weights[Maroon]`, or set a maroon drift diagonal |
| `--p1-deck braid` (replaces `DeckArchetype::BraidHeavy`) | bump `rings.heavy_weights` toward braid |
| Bead collector | bump `rings.light_weights` toward bead (beads are light) |
| Emblem collector | raise `emblems.weights[CrescentMoon]` (etc.), optionally zero the others |
| Search H2H | deal each seat's 27 from that seat's `CorePass`; the picker still runs 7+4 |

Rarity quoted in UI and this RFC is always `base()`, even when the live deal used a collector.

## Character Pass

The pass is versioned. A version freezes the symbol basis, relation table, specific maps, τ, optional k, and the fallback set. Adding or reweighting a character bumps the version so old deals replay.

Three tiers:

1. **Specific mapping** — exact combinatorial keys to an authored set of character ids.
2. **Aesthetic embedding** — sparse admin weights place a character in a frozen symbol space; unmapped tiles take a neighborhood under cosine τ, then the same `id % len` pick as the map.
3. **Fallback set** — reserved ids used only when the neighborhood is empty.

Keys and table rows use catalog ids (`teal-four-point-star`), not display names.

Owner is not part of the key. Mapped characters stay in the embedding so unmapped cousins can still land nearby. Fallback ids are omitted from the embedding so they cannot steal a neighborhood slot.

### Specific mapping

The key is the unordered multiset of live `(TileColor, RingType)` slots, plus `dots`, `CaptureMark`, and `Emblem`. Slot order is ignored (braid-as-a-group; matches how faces are named). `kind_index` is the wrong key: it is ordered and ignores capture and emblem.

```rust
CoreKey {
    rings: Multiset<(TileColor, RingType)>,
    dots: u8,                 // 1..=3
    capture: CaptureMark,     // Dot | Triangle
    emblem: Emblem,
}

// example
(
    rings: [
        (TileColor::Green, RingType::Braid),
        (TileColor::Blue, RingType::Braid),
        (TileColor::Teal, RingType::Normal),
    ].unordered(),
    dots: 3,
    capture: CaptureMark::Dot,
    emblem: Emblem::Rosette,
) -> ["andrew", "belinda"]   // catalog ids; names TBD
```

On a hit, pick `tile.id % set.len()`. Do not also consult cosine.

### Aesthetic embedding

Frozen symbol basis. No training. Pure symbols (each color, each ring type, each non-empty emblem, capture, dots) are points in the space and are the admin-facing attributes.

**Tile encoding.** Each live ring is a Kronecker atom `tile_rgb(c) ⊗ type_onehot(t)` (3×4 = 12D). Sum those atoms, L2-normalize, then concat downweighted extras and normalize again:

| Block | Weight |
|---|---|
| ring atoms (normalized 12D) | 1.0 |
| emblem (one-hot; `None` is origin) | 0.5 |
| `n_rings / 3` | 0.2 |
| capture (dot vs triangle) | 0.15 |
| `dots / 3` | 0.15 |

Counts, not a binary “has braid.” Inner vs outer slot order is not a v1 dim.

**Admin insert (sparse).** A new character is a row of weights on the attributes the author cares about. Omitted keys are 0: no opinion, not “a bit of everything.” An all-zero row is invalid.

```toml
id = "pink-claw-marks"
color.lavender-pink = 2.0
type.zag = 1.0
emblem.zs = 0.8
# color.green, type.normal, marks, … omitted → 0
```

Composition is **additive**, not an outer product of the admin’s sparse maps (that product would send a color-only row to the origin). Each specified weight adds or subtracts that symbol’s anchor:

```text
char = normalize(
    Σ w_color[c]  · ColorAnchor(c)     # rgb(c) ⊗ uniform types
  + Σ w_type[t]   · TypeAnchor(t)      # uniform colors ⊗ onehot(t)
  + Σ w_emblem[e] · EmblemAnchor(e)
  + Σ w_marks     · …
  + ε_id
)
```

Negative weights repel. One filled-in weight is a broad skew (large neighborhood); more weights shrink it. `ε_id` is a small id-derived residual so two identically tagged characters are not the same point.

Distance is **cosine**. Perceptual hash is not used: the procedural ring face and the painted medallion do not share a pixel domain.

**Pick.** Same rule as specific mapping, different set:

```text
S = { c | cosine(encode(tile), encode(c)) ≥ τ, c ∉ fallback }
if S empty → fallback[hash(id) % n]
else       → sort_by_id(S)[id % |S|]
```

Do not take argmin (every teal tile would collapse onto one face). Optional cap: keep the k nearest of S, then mod. Sort by id before the mod so iteration order cannot change the pick.

### Fallback set

A disjoint reserve of catalog ids. Used only on a τ miss (neighborhood empty). Pick `hash(tile.id) % set.len()`. These ids are excluded from cosine neighborhoods.

## Implementation

Not wired yet. When it is:

- `CorePass` + `RingPass` / `ColorPass` / `MarkingPass` / `EmblemPass` replace `sample_tile_with` knobs and `DeckArchetype` stuffing. `CorePass::base()` is the rarity chart; named presets are rewards/collectors.
- Character pass tables next to the catalog (`assets/characters/pass.vN.toml`): specific maps, sparse relation rows, τ, optional k, fallback ids
- `characters.rs`: `CoreKey`, `encode_tile`, `assign_character(tile, version)`
- `Tile.character: Option<CharacterId>` — cosmetic; not in `kind_index` / search / overwhelm
- Tests: map hits, sparse color-only rows are not the origin, preview-style neighborhoods, τ miss → fallback, version freeze; `CorePass::base()` replay from `id`

## Alternatives considered

- **One-hot 24 independent `(color, type)` dims.** Teal-zag would not sit near teal-normal.
- **Type counts + color histogram.** Loses which color is the zag.
- **Outer-product of the admin’s sparse color and type maps.** A color-only insert is the zero vector.
- **Unspecified = uniform over colors.** Washes out type-only characters.
- **Train / CLIP / pHash the PNGs.** Undiffable, ignores authored weights, twenty-five points.
- **Always nearest neighbor.** Zero variety inside a color family.
- **pHash(tile render, character PNG) as the τ gate.** Different pixel domains.

## Open questions

1. Always-on character vs a rarer roll (still open in 007).
2. Numeric τ / k after plotting the twenty-five against a sampled core deal.
3. Inner/outer ring order as a later dim.
4. Fallback membership: a hard-reserved disjoint set vs “catalog minus mapped.”
5. Personal names vs catalog ids in the specific-map examples.
6. Whether a color-only row spreads across all four types or only the three common colors.
