# Character illustrations

Twenty-five PNG medallions plus an explicit **`faceless`** catalog id (no
PNG). Ids are the file stem; display names title-case that stem with dashes
turned into spaces.

| Id | File | Display name |
|---|---|---|
| `faceless` | — | Faceless |
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

Character-pass v1 tables live in `pass/v1/` (specific map, embedding rows,
fallback set, plus `config.json` for version / τ / k). They sit behind
`CharacterTables` so a later database can replace the JSON.

Bevy `AssetServer` paths (once wired) are `characters/<filename>.png`, relative
to this crate's `assets/` folder.

Generation assigns a character on each tile (cosmetic), including `faceless`.
The PNG renderer draws painted medallions in the tile center and leaves
`faceless` as the plain inner disc. See
`apa-kan/game/screenshots/generation/` and
[rfc-000-000-00n-generation](../../../../rfc/rfc-000-000-00n-generation/README.md).

`faceless` may appear in the specific map, the aesthetic embedding, and the
fallback set. It is the only catalog id allowed to sit in both embedding and
fallback. The other fallback ids are `zaster` and `nocunist`.
