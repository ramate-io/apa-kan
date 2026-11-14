# Deck scenarios

TOML files that replace the default generative collection (`RuleSet::deck_size`, default 27), then draft a 7+4 public bench. Used by the app and `apa-kan-bench`:

```bash
cargo run -p apa-kan -- --scenario complementary
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --scenario same --games 12 --no-depth-probe
```

`--scenario` takes a builtin name (`default`, `same`, `independent`, `complementary`, `complementary-swapped`) or a path to a `.toml` file. `--list-scenarios` prints the builtins.

## Schema

```toml
name = "example"
description = "optional"
shuffle = true          # shuffle bags with the game seed (default true)
mirror = false          # copy P1's kinds+order onto P2 (new ids)
generative = false      # sample deck_size tiles from the ring process (same as Game::new)

# Complementary construction (18 tiles/player: two colors × 3 dots, repeated):
exclusive_p1 = "green"
exclusive_p2 = "blue"
shared = "orange"

# Or independent decks:
[p1]
colors = ["green", "orange"]   # all dots kinds of each color
# [[p1.tiles]]
# color = "green"              # or colors = ["green", "teal"] for two rings
# zags = ["teal"]              # which of those rings are zags (or zag = true for all)
# weight = 3                   # ignored; weight is the ring-slot count
#                              # omit weight/dots to expand all dots kinds of that color
# dots = 1
# count = 1
# [[p1.bench]]                 # optional ordered prefix of a small collection
# color = "green"
# weight = 3
# dots = 1
```

`default` / `same` is the current opening deal: `deck_size` sampled tiles (ring process, default 27), mirrored, then a 7+4 public bench under the 6-ring-per-type cap (braids count by slots). `--deck-size` / `--bag-size` applies to generative scenarios. `independent` samples each player's collection separately. Complementary presets are still 18 one-ring tiles of the three common colors (weight = 1), drafted to an 11-tile bench + 7 reserves, with 7 spawn tokens. Collections of 11 or fewer keep listed order.
