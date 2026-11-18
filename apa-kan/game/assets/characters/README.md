# Character illustrations

Fourteen square PNG medallions for later tile/deck generation. Filenames are
placeholder descriptions from the source art.

| Id | File | Display name |
|---|---|---|
| `blue-diamond-lattice` | `01-blue-diamond-lattice.png` | Blue Diamond Lattice |
| `green-pink-diamond-heart` | `02-green-pink-diamond-heart.png` | Green Pink Diamond Heart |
| `crossed-needles` | `03-crossed-needles.png` | Crossed Needles |
| `spirograph-diamond` | `04-spirograph-diamond.png` | Spirograph Diamond |
| `bird-of-paradise-pot` | `05-bird-of-paradise-pot.png` | Bird Of Paradise Pot |
| `twin-star-comets` | `06-twin-star-comets.png` | Twin Star Comets |
| `star-medallion` | `07-star-medallion.png` | Star Medallion |
| `teal-four-point-star` | `08-teal-four-point-star.png` | Teal Four Point Star |
| `black-beaked-face` | `09-black-beaked-face.png` | Black Beaked Face |
| `pink-claw-marks` | `10-pink-claw-marks.png` | Pink Claw Marks |
| `three-rosettes` | `11-three-rosettes.png` | Three Rosettes |
| `petal-diamond-cluster` | `12-petal-diamond-cluster.png` | Petal Diamond Cluster |
| `lavender-square-spirograph` | `13-lavender-square-spirograph.png` | Lavender Square Spirograph |
| `green-hatched-bowl` | `14-green-hatched-bowl.png` | Green Hatched Bowl |

`manifest.toml` is the source of truth for id / Bevy path / display name.
`CharacterLibrary` in `apa-kan/game/src/characters.rs` parses it.

Bevy `AssetServer` paths (once wired) are `characters/<filename>.png`, relative
to this crate's `assets/` folder.

**Not used in play yet.** Generation, rules, and the painter do not assign
characters. See [rfc-001-001-007-character-library](../../../../rfc/rfc-001-001-007-character-library/README.md).
