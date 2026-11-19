# RFC 001-001-007: Character illustration library

## Problem

Apa Kan tiles currently show rings, capture marks, and emblems, all drawn in
code. There is no catalog of **character** illustrations that generation can
attach to a tile. Twenty-five named PNG medallions exist as source art; they
need to live in the Bevy asset tree under stable ids.

This RFC only **lays the groundwork**. Wiring characters into tile/deck
generation, rules, and the painter is a follow-up.

## Proposed solution

Check the twenty-five PNGs into the game crate's Bevy asset folder, unchanged,
and give each a stable id so later generation can reference them without
caring about filenames.

| Field | Source |
|---|---|
| **id** | PNG file stem (`waking-rose.png` → `waking-rose`) |
| **path** | Bevy-relative from `apa-kan/game/assets/` (`characters/waking-rose.png`) |
| **display_name** | File stem, dashes turned into spaces, title-cased (`Waking Rose`) |

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
| 1 | `cadredl` | `cadredl.png` | Cadredl |
| 2 | `captain` | `captain.png` | Captain |
| 3 | `cosam` | `cosam.png` | Cosam |
| 4 | `harteld` | `harteld.png` | Harteld |
| 5 | `hartish` | `hartish.png` | Hartish |
| 6 | `lanyan` | `lanyan.png` | Lanyan |
| 7 | `magrenedl` | `magrenedl.png` | Magrenedl |
| 8 | `melets` | `melets.png` | Melets |
| 9 | `merenmarch` | `merenmarch.png` | Merenmarch |
| 10 | `nocunfall` | `nocunfall.png` | Nocunfall |
| 11 | `nocunist` | `nocunist.png` | Nocunist |
| 12 | `ocunbun` | `ocunbun.png` | Ocunbun |
| 13 | `sam` | `sam.png` | Sam |
| 14 | `savist` | `savist.png` | Savist |
| 15 | `skulkenflock` | `skulkenflock.png` | Skulkenflock |
| 16 | `springfall` | `springfall.png` | Springfall |
| 17 | `squib` | `squib.png` | Squib |
| 18 | `tellbrok` | `tellbrok.png` | Tellbrok |
| 19 | `tiblone` | `tiblone.png` | Tiblone |
| 20 | `tissen` | `tissen.png` | Tissen |
| 21 | `toppenpol` | `toppenpol.png` | Toppenpol |
| 22 | `waking-rose` | `waking-rose.png` | Waking Rose |
| 23 | `warsam` | `warsam.png` | Warsam |
| 24 | `westerlit` | `westerlit.png` | Westerlit |
| 25 | `zaster` | `zaster.png` | Zaster |

Twelve of these are byte-identical renames of the placeholder medallions.
`tiblone` and `springfall` replace the old bird / three-rosettes art. Eleven
are new. Source art is typically a circular medallion on a black field; sizes
range from about 1k to 4k px, and a few files are not square.

## Implementation

- PNGs: `apa-kan/game/assets/characters/*.png` (byte-for-byte from the source zip)
- Manifest: `apa-kan/game/assets/characters/manifest.toml`
- Registry: `apa-kan/game/src/characters.rs` (`CharacterLibrary::load` / `get`)
- Folder note: `apa-kan/game/assets/characters/README.md`
- Preview painter: `draw_tile_preview` / `write_png_view_preview` in `render.rs`,
  sized by `TileRingDraw::character_fit_radius`. Screenshots:
  `apa-kan/game/screenshots/characters/`. Not wired into `sample_tile` or Bevy.
- Shared layout: `TileRingDraw` emblem and capture-mark radii are smaller so
  the center disc has more room. Glyph shapes are unchanged. That layout is
  live tile rendering (PNG + Bevy); generation is unchanged.

`game.rs` and `deck.rs` still do not assign characters.

## Alternatives considered

- **Hard-coded Rust array, no TOML.** Rejected as the source of truth: adding
  art would require a code edit, and Bevy later wants a path string anyway.
- **Ids that keep a numeric prefix.** Rejected; the names are identity.
  Display order follows the alphabetical manifest list.
- **Git LFS.** Rejected; this repo does not use LFS, and these files are small
  enough for ordinary Git.
- **Downsampling on check-in.** Rejected; this PR keeps the source PNGs
  unchanged. Runtime size can be a later packing step.

## Open questions

1. **How does a character attach to a tile?** Decided in
   [rfc-000-000-00n-generation](../rfc-000-000-00n-generation/README.md):
   always a `CharacterId`. The empty face is the explicit catalog id
   `faceless` (no PNG), not `None`.
2. **Where does it sit on the face?** The preview places a circular clip of the
   PNG in the **center**, inside the innermost ring, under the ring strokes.
   Cream medallions on P1's light fill (and dark faces on P2's dark fill) can
   go low-contrast. Final slot, scale, and whether to drop the medallion's own
   background are still open.
3. **Factions / rules text.** Display names now match the file stems. Extra
   flavor, factions, or rules copy are still TBD.
4. **Runtime resolution.** Keep the 1–4k source files, or pack smaller
   textures for desktop/mobile?
5. **Non-square sources.** `zaster.png` is 1984×1985; `savist`, `squib`,
   `toppenpol`, `waking-rose`, and `ocunbun` are farther off square. The
   preview circular-clips and preserves aspect ratio.
