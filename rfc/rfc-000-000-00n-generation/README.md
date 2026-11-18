# RFC 000-000-00n: Tile generation

Follow-up to [rfc-001-001-007-character-library](../rfc-001-001-007-character-library/README.md). That RFC catalogs fourteen character ids; this one assigns a character to a generated tile.

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

- Pass tables next to the catalog (`assets/characters/pass.vN.toml`): specific maps, sparse relation rows, τ, optional k, fallback ids
- `characters.rs`: `CoreKey`, `encode_tile`, `assign_character(tile, version)`
- `Tile.character: Option<CharacterId>` — cosmetic; not in `kind_index` / search / overwhelm
- Tests: map hits, sparse color-only rows are not the origin, preview-style neighborhoods, τ miss → fallback, version freeze

## Alternatives considered

- **One-hot 24 independent `(color, type)` dims.** Teal-zag would not sit near teal-normal.
- **Type counts + color histogram.** Loses which color is the zag.
- **Outer-product of the admin’s sparse color and type maps.** A color-only insert is the zero vector.
- **Unspecified = uniform over colors.** Washes out type-only characters.
- **Train / CLIP / pHash the PNGs.** Undiffable, ignores authored weights, fourteen points.
- **Always nearest neighbor.** Zero variety inside a color family.
- **pHash(tile render, character PNG) as the τ gate.** Different pixel domains.

## Open questions

1. Always-on character vs a rarer roll (still open in 007).
2. Numeric τ / k after plotting the fourteen against a sampled core deal.
3. Inner/outer ring order as a later dim.
4. Fallback membership: a hard-reserved disjoint set vs “catalog minus mapped.”
5. Personal names vs catalog ids in the specific-map examples.
6. Whether a color-only row spreads across all four types or only the three common colors.
