# Character illustrations

Twenty-five PNG medallions for later tile/deck generation. Ids are the file
stem; display names title-case that stem with dashes turned into spaces.

| Id | File | Display name |
|---|---|---|
| `cadredl` | `cadredl.png` | Cadredl |
| `captain` | `captain.png` | Captain |
| `cosam` | `cosam.png` | Cosam |
| `harteld` | `harteld.png` | Harteld |
| `hartish` | `hartish.png` | Hartish |
| `lanyan` | `lanyan.png` | Lanyan |
| `magrenedl` | `magrenedl.png` | Magrenedl |
| `melets` | `melets.png` | Melets |
| `merenmarch` | `merenmarch.png` | Merenmarch |
| `nocunfall` | `nocunfall.png` | Nocunfall |
| `nocunist` | `nocunist.png` | Nocunist |
| `ocunbun` | `ocunbun.png` | Ocunbun |
| `sam` | `sam.png` | Sam |
| `savist` | `savist.png` | Savist |
| `skulkenflock` | `skulkenflock.png` | Skulkenflock |
| `springfall` | `springfall.png` | Springfall |
| `squib` | `squib.png` | Squib |
| `tellbrok` | `tellbrok.png` | Tellbrok |
| `tiblone` | `tiblone.png` | Tiblone |
| `tissen` | `tissen.png` | Tissen |
| `toppenpol` | `toppenpol.png` | Toppenpol |
| `waking-rose` | `waking-rose.png` | Waking Rose |
| `warsam` | `warsam.png` | Warsam |
| `westerlit` | `westerlit.png` | Westerlit |
| `zaster` | `zaster.png` | Zaster |

`manifest.toml` is the source of truth for id / Bevy path / display name.
`CharacterLibrary` in `apa-kan/game/src/characters.rs` parses it.

Bevy `AssetServer` paths (once wired) are `characters/<filename>.png`, relative
to this crate's `assets/` folder.

**Not used in play yet.** Generation and the live painter do not assign
characters. A preview-only PNG path draws each medallion in the tile center
(inside the rings); see `apa-kan/game/screenshots/characters/` and
[rfc-001-001-007-character-library](../../../../rfc/rfc-001-001-007-character-library/README.md).
