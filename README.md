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
