# Deck scenarios

TOML files that replace the default generative 27-tile deal. Used by the app and `apa-kan-bench`:

```bash
cargo run -p apa-kan -- --scenario complementary
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --scenario same --games 12 --think-ms 1000 --no-depth-probe
```

`--scenario` takes a builtin name (`default`, `same`, `independent`, `complementary`, `complementary-swapped`) or a path to a `.toml` file. `--list-scenarios` prints the builtins.

## Schema

```toml
name = "example"
description = "optional"
shuffle = true          # shuffle bags with the game seed (default true)
mirror = false          # copy P1's kinds+order onto P2 (new ids)
generative = false      # sample 27 tiles from the ring process (same as Game::new)

# Complementary construction (18 tiles/player: two colors × 9 kinds):
exclusive_p1 = "green"
exclusive_p2 = "blue"
shared = "orange"

# Or independent decks:
[p1]
colors = ["green", "orange"]   # all 9 kinds of each color
# [[p1.tiles]]
# color = "green"              # or colors = ["green", "teal"] for two rings
# zags = ["teal"]              # which of those rings are zags (or zag = true for all)
# weight = 3                   # omit weight/dots to expand all 9 kinds of that color
# dots = 1
# count = 1
# [[p1.bench]]                 # optional fixed opening bench (consumed from the bag)
# color = "green"
# weight = 3
# dots = 1
```

`default` / `same` is the current opening deal: 27 sampled tiles (ring process), mirrored shuffle. `independent` samples each player's bag separately. Complementary presets are still 18 one-ring tiles of the three common colors and keep `starting_bench` at 7 (11 left in each bag).
