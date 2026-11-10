# New Rules Implementation Plan

## Rule A: Fixed Rotation Direction
- Add `rotation_direction: RotationDirection` enum (CW/CCW) to Tile struct
- Randomize direction during tile generation
- Update MOVE to accept 0 (keep) or 1 (rotate in direction) instead of 0-3
- Update action generation to only produce 2 rotation choices per destination
- Update UI to show rotation arrow on tiles
- Update input to toggle between 2 orientations

## Rule B: ADD Bank
- Add `add_bank_p1: u8` and `add_bank_p2: u8` to Game struct
- Initialize to 7 at game start
- Decrement on ADD action
- Increment when opponent tile removed
- Block ADD when bank is 0
- Extend tile stack if needed when ADDs remain but stack empty

## Rule C: Moved-Tile Immunity
- Track last moved tile ID and turn number
- Skip removal check for immune tile
- Immunity expires after opponent's turn or when ≤7 tiles on board

## RuleSet Config
```rust
struct RuleSet {
    fixed_rotation: bool,      // Rule A
    add_bank: bool,            // Rule B
    moved_tile_immunity: bool, // Rule C (off by default)
}
```

## Implementation Order
1. PR #3: Core rules engine + RuleSet
2. PR #4: Input/UI updates
3. PR #5: Bot updates + search optimizations + benchmark

## Benchmark Requirements
- 4 variants: old, A-only, A+B, A+B+C
- Depth 3 if feasible (add optimizations)
- 100+ games per pairing (50 per seat)
- 6 pairings each variant = 2400+ total games

## Search Optimizations
- Transposition table (Zobrist hashing)
- Move ordering (captures first)
- Iterative deepening with time budget
