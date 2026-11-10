# Apa Kan

A small turn-based game of colors and adjacency.

## About

Apa Kan is a turn-based strategy game being developed by Ramate LLC using Rust and the Bevy game engine.

## Development

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- Rust 1.95.0 (managed by Nix)

### Getting Started

Enter the development environment:

```bash
nix develop
```

Build and run the game:

```bash
cargo run -p apa-kan
```

### Play against a bot

The desktop game supports AI opponents with configurable settings:

```bash
# Play as P1 against Ologbon on 5x5 scored rules (500ms think time)
cargo run -p apa-kan -- --bot ologbon --seat 1 --size 5 --rules scored --think-ms 500

# Play as P2 against Olusare on 7x7 scored rules
cargo run -p apa-kan -- --bot olusare --seat 2 --size 7 --rules scored

# Play against MCTS on 5x5 old rules
cargo run -p apa-kan -- --bot mcts --rules old

# Play against random bot (Ore)
cargo run -p apa-kan -- --bot ore
```

**Available options:**
- `--bot <none|ore|olusare|ologbon|mcts>` (default: none = hot-seat)
- `--seat <1|2>` Human player's seat (default: 1)
- `--size <5|7>` Board size (default: 5)
- `--rules <scored|old>` Rule set (default: scored)
- `--think-ms <n>` Bot think time in milliseconds (default: 500)

**Gameplay:**
- On your turn, green **ADD** markers show legal placement squares. Click one, then click a tile from your bench (bottom of the screen).
- Click one of your tiles on the board to see purple **MOVE** destinations. Press `R` to rotate, then click a destination.
- Both benches stay visible: yours at the bottom, the opponent's at the top, with edge colors and P1/P2 labels.
- When it's your turn and you have no legal moves in scored mode, press `P` or the Pass button
- Each action is described on the **Last move** line and printed to stdout
- Status display shows: whose turn, tile counts, ADD banks remaining, and turn number

Build for release:

```bash
cargo build -p apa-kan --release
```

### Testing

Run tests:

```bash
cargo test
```

### Code Style

Format code with:

```bash
cargo fmt --all
```

Check with:

```bash
cargo check
```

## Project Structure

- `apa-kan/game/` - Main game binary
- `rfc/` - Request for Comments (design documents)
- `packaging/` - Platform-specific packaging scripts and configuration
- `.github/workflows/` - CI/CD workflows

## Packaging

The project supports packaging for multiple platforms:

- Desktop: Linux, macOS, Windows
- Mobile: iOS, Android

See [packaging/README.md](packaging/README.md) for detailed packaging instructions and requirements.

## Contributing

This is a source-available project. See [LICENSE.md](LICENSE.md) for terms of use.

Contributions are welcome via pull requests. Any submitted contributions may be subject to separate contribution terms.

## RFCs

Feature proposals and technical designs are documented in the `rfc/` directory. See [rfc/README.md](rfc/README.md) for more information.

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details. 
