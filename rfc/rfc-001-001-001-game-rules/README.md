# RFC-001-001-001: Apa Kan Game Rules Specification

## Problem Statement

Apa Kan is a two-player abstract strategy board game requiring a precise rules engine to handle tile placement, matching mechanics, and win conditions. The designer has provided the core rules, but several ambiguities need to be resolved for implementation.

## Core Rules (Designer Specification)

### Setup
- 2 players
- 5×5 grid board
- 4 corner squares cannot be occupied and always count as a mismatch
- Each player has 11 square tiles in their stack
- Each tile edge has one of 4 colors: Maroon, Green, Blue, or Orange
- Edges may be single (weight 1) or double (weight 2, drawn with two lines)

### Turn Actions
A player chooses ONE of:
1. **ADD**: Place one tile from your stack onto any empty, non-corner square
2. **MOVE**: Move one of your tiles already on the board
3. **ROTATE**: Rotate one of your tiles on the board

### Matching and Removal
When a tile is placed, if it has more edges that **mismatch** in color than **match**, the tile gets removed.

### Movement Rules
When moving a tile:
1. Tile preserves its cardinal orientation (no rotation during move)
2. Every cardinal edge the tile "moves over" must have at least as much edge weight as the matching edge of the tile
3. Where tiles are connected (share an edge), the maximum weight on either side is the weight of that edge
4. The move must finish in an empty spot

### Rotation Rules
- Player may rotate the tile
- Cannot end in the starting orientation

### Win Condition
The game ends when one player cannot ADD or MOVE a tile; the other player wins.

## Implementation Decisions (Open Questions Resolved)

### 1. Mismatch Counting Against Empty Squares and Board Boundary

**Question**: How do we count mismatches for tiles adjacent to empty squares or the board edge?

**Decision** (CONFIRMED BY DESIGNER): Empty neighboring squares and the board boundary are NOT mismatches - they are NEUTRAL and count as neither match nor mismatch. Only the four corner squares (0,0), (0,4), (4,0), (4,4) are special and always count as a mismatch for any edge facing them.

**Rationale**: This encourages strategic placement and makes the game more forgiving in the early game when few tiles are placed. It also makes corner-adjacent squares more challenging to hold.

**Test Coverage**: `test_empty_adjacent_is_neutral()`, `test_corner_mismatch()`, `test_board_boundary_neutral()`

### 2. Removal Check Timing

**Question**: Does the removal check apply only after ADD, or also after MOVE and ROTATE? Do we check neighbors when a tile moves/rotates?

**Decision** (CONFIRMED BY DESIGNER): The removal check runs BOARD-WIDE at the end of every turn, after any action (ADD/MOVE/ROTATE) completes. All tiles on the board are evaluated for the mismatch condition simultaneously.

**Simultaneous Removal Resolution**: When multiple tiles become mismatched in the same turn:
- All mismatched tiles are identified in a single pass over the board
- All identified tiles are removed simultaneously (atomically)
- Removed tiles are returned to their owner's stack
- Order of removal does not matter since all checks happen before any removals

**Rationale**: End-of-turn board-wide checking creates predictable, fair gameplay where all tiles are evaluated under the same conditions. Simultaneous removal prevents order-dependent cascades and makes the game state deterministic.

**Test Coverage**: `test_move_triggers_removal()`, `test_rotate_triggers_removal()`, `test_simultaneous_removal()`, `test_board_wide_removal_check()`

### 3. Removed Tile Handling

**Question**: Is a removed tile returned to the player's stack or permanently out of play?

**Decision** (CONFIRMED BY DESIGNER): Removed tiles are DISCARDED - permanently out of the game.

**Rationale**: This is how the game progresses toward an end. Players must protect their tiles carefully, as lost tiles cannot be recovered. This creates higher stakes and encourages defensive play.

**Test Coverage**: `test_removed_tile_discarded()`

### 4. Tile Edge Generation

**Question**: How are tile edge colors and weights generated? Are they randomly dealt or predetermined?

**Decision**: For the initial implementation:
- Each player receives a randomized set of 11 tiles generated at game start
- Each tile has 4 edges, each randomly assigned:
  - Color: uniformly random from {Maroon, Green, Blue, Orange}
  - Weight: 80% single (weight 1), 20% double (weight 2)
- The system is parameterized to allow for:
  - Predetermined tile sets (for testing/tournament play)
  - Balanced generation (ensuring fair distribution)
  - Asymmetric player advantages (for difficulty levels)

**Rationale**: Random generation provides replayability. The 80/20 weight distribution keeps double edges special but not too rare. The parameterized system allows future refinement.

**Test Coverage**: `test_tile_generation()`, `test_predetermined_tiles()`

### 5. Movement Path Interpretation

**Question**: What does "moving over" mean? Can tiles move through occupied squares or only empty ones? Single step or multi-step paths?

**Decision** (CONFIRMED BY DESIGNER): Movement is TILE TRAVELING through adjacent occupied tiles.

"It's strictly tile traveling. You can tile travel as far as you want through adjacencies (whether they are yours or your opponent's tiles). You can no longer MOVE if you don't have any [adjacencies]."

**Mechanics**:
- A tile travels through a chain of adjacent occupied tiles (either player's)
- Can travel any distance through valid adjacencies
- Tile maintains its cardinal orientation (no rotation during travel)
- Each step over a tile must satisfy the edge-weight rule:
  - The shared edge must have at least as much weight as the moving tile's matching edge
  - Where two tiles share an edge, that edge's weight = max(both sides)
- The move must END on an empty, non-corner square
- A tile with NO adjacent tiles has no legal MOVE actions

**Implementation**: BFS/flood-fill to find all reachable empty destinations via valid tile adjacencies.

**Rationale**: Creates strategic depth where building connected networks matters. Heavy-edged tiles have restricted mobility. Isolated tiles cannot move.

**Test Coverage**: `test_move_tile_traveling()`, `test_move_multi_step()`, `test_move_weight_constraint()`, `test_move_no_adjacencies()`, `test_move_ends_on_empty()`

### 6. Win Condition Check

**Question**: Does the "cannot ADD or MOVE" check apply to the player whose turn it is, or do we check after they complete their turn?

**Decision**: 
- At the START of a player's turn, check if they have any legal ADD or MOVE actions
- If they have neither, they lose (the other player wins)
- ROTATE-only scenarios are considered a loss (cannot just rotate forever)

**Rationale**: This prevents stalling and ensures the game progresses. Players must maintain mobility to stay in the game.

**Test Coverage**: `test_win_no_legal_moves()`, `test_rotate_only_is_loss()`

## Architecture

### Module Structure
```
src/
  lib.rs           - Bevy app entry point
  main.rs          - Thin binary wrapper
  game/
    mod.rs         - Game module root
    core.rs        - Core data structures (Color, EdgeWeight, Tile, Board)
    rules.rs       - Rule enforcement (placement, removal, validation)
    actions.rs     - Action types and legal move generation
    ai.rs          - [PR3] AI opponent implementation
  ui/
    mod.rs         - UI module root
    board.rs       - Board rendering
    tiles.rs       - Tile rendering
    menu.rs        - Menu and game state screens
    input.rs       - [PR2] Input handling
```

### Key Design Principles
1. **Pure Rust core**: Game logic is independent of Bevy, fully testable
2. **Parameterized rules**: Rule variations can be configured (for future tuning)
3. **Legal move generation**: Efficient iteration over valid actions
4. **Immutable operations**: Game state transformations return new states

## Test Strategy

### Unit Tests
- Tile matching logic (match/mismatch counting)
- Removal conditions (all edge cases)
- Move validation (weights, boundaries, legality)
- Rotation constraints
- Win condition detection

### Integration Tests  
- Full game scenarios
- Multi-turn sequences
- Edge cases (corner adjacency, cascading removal)

### Property-Based Tests (Future)
- Invariant: Valid board state after any legal action
- Invariant: Game always terminates (no infinite loops)

## Open Questions for Designer

1. **Tile Set Balance**: Should both players receive identically symmetric tile sets (mirrored/rotated), or is random asymmetry acceptable?

2. **Simultaneous Removal**: If multiple tiles become mismatched simultaneously (e.g., a placement causes multiple neighbors to mismatch), should they all be removed in one check, or is there an order of evaluation?

3. **Movement Priority**: In the current implementation, tiles can only move one step. Should there be a "slide" mechanic where a tile can move multiple squares in one direction if all edges satisfy the weight constraint?

4. **Rotation Feedback**: When a player rotates a tile, do opponent-adjacent tiles get re-checked for removal immediately, or only on the opponent's next action?

5. **Draw Condition**: Can the game end in a draw (e.g., both players have no legal ADD/MOVE on their respective turns)? Or does the first player unable to move simply lose?

6. **Weight Distribution**: Is the 80% single / 20% double weight distribution appropriate, or should it be tuned (e.g., 70/30, 90/10)?

7. **Color Distribution**: Should there be any constraints on color distribution per tile (e.g., at least 2 different colors per tile) or is fully random acceptable?

## Implementation Phases (PR Structure)

### PR1: Core Engine + Rendering
- Pure Rust rules engine with full test coverage
- Bevy rendering of board, tiles, and stacks
- Title screen → local 2-player game flow

### PR2: Input System
- Touch/mouse-first input for mobile compatibility
- Tile selection, placement, movement, and rotation
- Legal move highlighting and feedback

### PR3: AI Opponent
- Minimax with alpha-beta pruning
- Baseline mobility heuristic
- Improved heuristic with evaluation
- Configurable difficulty (search depth)
- Menu option for PvP or PvE

## Alternatives Considered

### Alternative: Tiles Cannot Move After Placement
This would simplify the rules but reduce strategic depth. MOVE actions create interesting tactical opportunities.

### Alternative: No Removal Cascade
Only check the acted-upon tile, never neighbors. This would be simpler but less dynamic and strategic.

### Alternative: Removed Tiles Permanently Lost
More punishing, could lead to quicker games but less forgiving for new players.

## References

- Original game design by [Designer Name - to be added]
- Bevy 0.19.1 documentation: https://docs.rs/bevy/0.19.1/
