# Web client (wasm32-unknown-unknown)

Bevy owns the entire game UI (home, Head-to-Head lobby, match). This directory is a thin static host: a canvas, resize/focus, and `window.open` for external and payment links. There is no Dioxus chrome and no in-app browser.

## Build and serve

```bash
nix develop   # optional; rust-toolchain includes wasm32-unknown-unknown
packaging/scripts/build-web.sh
packaging/scripts/serve-web.sh   # http://127.0.0.1:8080/
```

`build-web.sh` compiles the `apa-kan` binary for `wasm32-unknown-unknown` with WebGL2, runs `wasm-bindgen --target web`, optionally `wasm-opt -Oz`, copies `apa-kan/game/web/index.html`, and writes `dist/web/size.txt`.

WebGPU is a trivial feature override, not the default:

```bash
cargo build -p apa-kan --bin apa-kan --release --target wasm32-unknown-unknown --features webgpu
```

## Host and domain

The web game gets its own domain, which is not chosen yet, so the client hardcodes no host. `apa-kan/game/web/index.html` carries `apakan:*` meta tags that [`host.rs`](../../apa-kan/game/src/host.rs) reads at startup:

| Meta tag | Env var | When unset |
|---|---|---|
| `apakan:public-url` | `APAKAN_PUBLIC_URL` | `window.location.origin` |
| `apakan:api-url` | `APAKAN_API_URL` | offline: `/game/<id>` stays a local seeded match |
| `apakan:market-url` | `APAKAN_MARKET_URL` | `https://symbolic.software/` |
| `apakan:payment-url` | `APAKAN_PAYMENT_URL` | `https://symbolic.software/` |
| `apakan:legal-url` | `APAKAN_LEGAL_URL` | the repo `LICENSE.md` |

`packaging/scripts/configure-web-host.sh` fills them. `build-web.sh` runs it at build time, and the Docker image runs it again at container start, so one image serves any domain:

```bash
APAKAN_PUBLIC_URL=https://play.example packaging/scripts/build-web.sh
docker run --rm -p 8080:80 -e APAKAN_PUBLIC_URL=https://play.example -e APAKAN_API_URL=https://api.play.example apa-kan-web
```

Native builds take the same env vars, or `--public-url` / `--api-url`. Settings shows the resolved host.

## Offline Divisions

Head-to-Head → Reward → **Divisions (offline)** plays a rated bot ladder with no server. It follows the core-split RFC's divisions decision: a bot ladder, separate from human games and from the account Elo.

- Everyone starts at 1000 Elo in Bronze. K = 32, floor 100.
- Bands (floor → opponent): Bronze 0 → mcts 80 ms, Silver 1100 → mcts 150 ms, Gold 1250 → mcts 600 ms, Platinum 1400 → mcts 1 s, Diamond 1550 → mcts 2.5 s. The band only sets which existing bot plays and its think budget.
- Provisional progression defaults (not measured strengths): each bot plays at a fixed rating (900 / 1150 / 1300 / 1450 / 1600). Your Elo moves against it.
- A finished game is rated once. Leaving an unfinished game does not rate it.
- The ladder is saved in `localStorage` (`apakan.divisions.v1`) on web, and in `$XDG_DATA_HOME/apa-kan/apakan.divisions.v1.json` on native (override with `APAKAN_DIVISIONS_FILE`).
- Deep link: `#/h2h?reward=divisions`.

## Deep links

Incoming links use the same platform-neutral hooks as Android (#95) and iOS (#96): `open_url`, `IncomingUrl { uri }`, and `push_incoming_url`. The host page’s location (`/game/<id>`, `#/h2h`, `apakan://…`) is pushed into that queue; Bevy routes it.

The same [`Route`](../../apa-kan/game/src/route.rs) parser handles:

| Web | Native |
|---|---|
| `/` or `#/` | `apakan://home` |
| `/h2h` or `#/h2h` | `apakan://h2h` |
| `/game/<id>` or `#/game/<id>` | `apakan://game/<id>` |
| `/market`, `/settings`, `/puzzles` | `apakan://market` … |

Hash routes work on any static file server. Path routes need a SPA fallback (see `nginx.conf`). Query pairs such as `#/h2h?opponent=bot&bot=minimax&time=2+1` are applied to the lobby. `/game/<id>` starts a local seeded match until a server hosts real ids.

## Bot search (no tab freeze)

The live client **time-slices** search (~8 ms per animation frame) via `Searcher::think_slice`. The minimax TT and MCTS tree stay in the Bevy world.

A web worker was rejected: `wasm32-unknown-unknown` has no threads without `atomics` + `SharedArrayBuffer` (COOP/COEP headers), and the transposition table is too large to clone into a worker each ply. Root-parallel MCTS (`mcts_root_threads > 1`) is forced to 1 on wasm.

`getrandom` uses the `wasm_js` backend (`--cfg getrandom_backend="wasm_js"` in `.cargo/config.toml`). `android-native-activity` is a target-cfg Android feature and is not in the wasm graph.

## Docker / static hosting

```bash
packaging/scripts/build-web.sh
docker build -f packaging/web/Dockerfile -t apa-kan-web .
docker run --rm -p 8080:80 apa-kan-web
```

Any static host (GitHub Pages, nginx, S3+CloudFront) can serve `dist/web/`. For path-style deep links, configure a fallback to `index.html`.
