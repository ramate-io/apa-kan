# Deck scenarios

TOML files that replace the default generative deal (`RuleSet::bag_size`, default 11). Used by the app and `apa-kan-bench`:

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
generative = false      # sample bag_size tiles from the ring process (same as Game::new)

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
# [[p1.bench]]                 # optional ordered prefix of the public deck
# color = "green"
# weight = 3
# dots = 1
```

`default` / `same` is the current opening deal: `bag_size` sampled tiles (ring process, default 11), mirrored shuffle, all public. `--deck-size` / `--bag-size` applies to generative scenarios (`--bag-size 27` restores the historical full bag). `independent` samples each player's deck separately. Complementary presets are still 18 one-ring tiles of the three common colors (weight = 1), all unplaced, with 7 spawn tokens.
