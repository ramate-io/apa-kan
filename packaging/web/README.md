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
