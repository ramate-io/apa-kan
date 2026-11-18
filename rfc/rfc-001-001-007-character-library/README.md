# RFC 001-001-007: Character illustration library

## Problem

Apa Kan tiles currently show rings, capture marks, and emblems, all drawn in
code. There is no catalog of **character** illustrations that generation can
attach to a tile. Fourteen square PNG medallions exist as source art, but they
are not in the Bevy asset tree and have no stable ids.

This RFC only **lays the groundwork**. Wiring characters into tile/deck
generation, rules, and the painter is a follow-up.

## Proposed solution

Check the fourteen PNGs into the game crate's Bevy asset folder, unchanged, and
give each a stable id so later generation can reference them without caring
about filenames.

| Field | Source |
|---|---|
| **id** | Filename slug with the numeric prefix dropped (`01-blue-diamond-lattice.png` → `blue-diamond-lattice`) |
| **path** | Bevy-relative from `apa-kan/game/assets/` (`characters/01-blue-diamond-lattice.png`) |
| **display_name** | Title-cased remainder of the filename (placeholder; may be renamed later) |

Files stay at `apa-kan/game/assets/characters/` so Bevy's default `AssetPlugin`
(`assets/` next to this crate) can load them later as
`characters/<file>.png`. No Git LFS: the repo does not use it, and the largest
file is under 2 MiB.

The catalog is `apa-kan/game/assets/characters/manifest.toml`.
`CharacterLibrary` in `apa-kan/game/src/characters.rs` parses that TOML and
looks up entries by id. Tests assert every listed file exists and decodes as a
PNG, and that every PNG in the folder is listed.

**Out of scope for this RFC:** assigning a character on `Tile`, rolling one
during `sample_tile`, CLI flags, collectors, search encoding, JSONL, scenario
TOML, or any rule change. The PNG renderer has a **preview-only** path that
draws a character medallion in the tile center (inside the innermost ring) so
the art can be judged against rings, emblems, and capture marks. Generation
and the live Bevy painter do not use it.

### Catalog

| # | Id | File | Display name |
|---|---|---|---|
| 1 | `blue-diamond-lattice` | `01-blue-diamond-lattice.png` | Blue Diamond Lattice |
| 2 | `green-pink-diamond-heart` | `02-green-pink-diamond-heart.png` | Green Pink Diamond Heart |
| 3 | `crossed-needles` | `03-crossed-needles.png` | Crossed Needles |
| 4 | `spirograph-diamond` | `04-spirograph-diamond.png` | Spirograph Diamond |
| 5 | `bird-of-paradise-pot` | `05-bird-of-paradise-pot.png` | Bird Of Paradise Pot |
| 6 | `twin-star-comets` | `06-twin-star-comets.png` | Twin Star Comets |
| 7 | `star-medallion` | `07-star-medallion.png` | Star Medallion |
| 8 | `teal-four-point-star` | `08-teal-four-point-star.png` | Teal Four Point Star |
| 9 | `black-beaked-face` | `09-black-beaked-face.png` | Black Beaked Face |
| 10 | `pink-claw-marks` | `10-pink-claw-marks.png` | Pink Claw Marks |
| 11 | `three-rosettes` | `11-three-rosettes.png` | Three Rosettes |
| 12 | `petal-diamond-cluster` | `12-petal-diamond-cluster.png` | Petal Diamond Cluster |
| 13 | `lavender-square-spirograph` | `13-lavender-square-spirograph.png` | Lavender Square Spirograph |
| 14 | `green-hatched-bowl` | `14-green-hatched-bowl.png` | Green Hatched Bowl |

Source art is square (or 1 px off) in the 1122–3022 px range, typically a
circular medallion on a black field.

## Implementation

- PNGs: `apa-kan/game/assets/characters/*.png` (byte-for-byte from the source zip)
- Manifest: `apa-kan/game/assets/characters/manifest.toml`
- Registry: `apa-kan/game/src/characters.rs` (`CharacterLibrary::load` / `get`)
- Folder note: `apa-kan/game/assets/characters/README.md`
- Preview painter: `draw_tile_preview` / `write_png_view_preview` in `render.rs`,
  sized by `TileRingDraw::character_fit_radius`. Screenshots:
  `apa-kan/game/screenshots/characters/`. Not wired into `sample_tile` or Bevy.

`game.rs` and `deck.rs` still do not assign characters.

## Alternatives considered

- **Hard-coded Rust array, no TOML.** Rejected as the source of truth: adding
  art would require a code edit, and Bevy later wants a path string anyway.
- **Ids that keep the numeric prefix** (`01-blue-diamond-lattice`). Rejected;
  the numbers are zip order, not identity. Display order can still follow the
  manifest list.
- **Git LFS.** Rejected; this repo does not use LFS, and these files are small
  enough for ordinary Git.
- **Downsampling on check-in.** Rejected; this PR keeps the source PNGs
  unchanged. Runtime size can be a later packing step.

## Open questions

1. **How does a character attach to a tile?** One optional slot (like emblem),
   always-on, or a rarer roll? Not decided; do not generate yet.
2. **Where does it sit on the face?** The preview places a circular clip of the
   PNG in the **center**, inside the innermost ring, under the ring strokes.
   Cream medallions on P1's light fill (and the black-beaked face on P2's dark
   fill) can go low-contrast. Final slot, scale, and whether to drop the
   medallion's own background are still open.
3. **Display names.** Filename-derived placeholders. Final names, factions, or
   rules text are TBD.
4. **Runtime resolution.** Keep the 1–3k source files, or pack smaller
   textures for desktop/mobile?
5. **`04-spirograph-diamond.png` is 1984×1985.** Treat as square, or recrop?
