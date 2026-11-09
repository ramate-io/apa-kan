# RFC-002-002-002: AI Opponent Heuristics

## Problem Statement

Apa Kan needs computer opponents of varying skill levels to provide engaging single-player gameplay. The AI must handle the updated rules (MOVE with rotation, threefold repetition draws, 100-turn limit) and provide meaningful strategic play.

## Bot Implementations

### Ore (Random Selector)
**Strategy**: Uniformly random legal move selection with seeded RNG for reproducibility.

**Purpose**: Baseline opponent and control for benchmarks.

**Complexity**: O(1) decision time after move generation.

### Olusare (Mobility-Based Minimax)
**Strategy**: Minimax with alpha-beta pruning using mobility heuristic.

**Heuristic**: Evaluates positions based on:
- Terminal states: Win = +10000, Loss = -10000, Draw = 0
- Non-terminal: Count of legal ADD and MOVE actions for each player
- Score = (my_mobility - opponent_mobility) * 10

**Purpose**: Intermediate opponent focusing on keeping options open.

**Complexity**: O(b^d) where b is branching factor, d is search depth.

### Ologbon (Enhanced Minimax)
**Strategy**: Minimax with alpha-beta pruning using comprehensive heuristic.

**Heuristic**: Multi-factor evaluation:
- Terminal states: Win = +10000, Loss = -10000, Draw = 0
- Non-terminal weighted sum:
  - Tile count: +100 per tile
  - Mobility: +10 per legal action
  - Center control: +20 per tile within 1 square of center
  - Stack advantage: +5 per tile in hand

**Purpose**: Strong opponent with positional understanding.

**Complexity**: O(b^d) with higher constant factor than Olusare.

## Draw Handling

All bots evaluate draws as score 0 (neutral), with the following ordering in minimax:
- Win > Draw > Loss
- Bots prefer wins over draws when possible
- Bots prefer draws over losses when facing inevitable defeat

Draw conditions recognized in search:
1. **Threefold Repetition**: Position hash matches 2 and 4 turns ago
2. **100-Turn Limit**: Turn counter reaches 100

## Search Configuration

**Default Depth**: 3 (configurable)
**Time Budget**: Optional per-move time limit (default: none for deterministic benchmark)
**Alpha-Beta Pruning**: Enabled on all minimax bots

## Benchmark Harness

### Methodology
- Pairwise round-robin: Each bot plays all others including itself (mirror matches)
- Balanced sides: 50 games per side (100 total per pairing)
- Deterministic: Seeded tile generation and AI RNG for reproducibility
- Turn limit: 100 turns enforced as native game rule

### Metrics Tracked

**Outcome Metrics**:
- Wins/draws/losses per bot per seat
- End reasons: no legal moves, repetition draw, 100-turn limit
- Average game length (turns)

**Action Statistics**:
- ADD vs MOVE action counts (mean, median, min, max)
- Moves with rotation (rotation_count > 0)
- Action type share (ADD%, MOVE%)
- Tracked for: winner's actions, all player actions, by seat

**First-Move Advantage (FMA)**:
- Mirror match first-player win rate with 95% confidence interval
- Assesses game balance and positional value

### Benchmark Results

Benchmark output includes:
1. Pairwise results table (wins/draws per matchup)
2. End reason breakdown (no legal moves vs draws)
3. First-move advantage analysis with statistical significance
4. Action type statistics for winners
5. Action type statistics for all players
6. Overall summary statistics

Full results appended after benchmark completes.

## Implementation Notes

### Bot Naming (Yoruba)
- **Ore**: Friend (simple, approachable)
- **Olusare**: Bringer of good fortune (strategic mobility)
- **Ologbon**: Wise one (comprehensive heuristic)

### Technical Details
- All bots use seeded RNG for reproducibility
- Minimax implements standard alpha-beta with move ordering
- Position evaluation is symmetric (zero-sum)
- Time budget optional (disabled for benchmark determinism)
- Draw detection integrated in rules engine

---

## Benchmark Results

(Results will be appended after benchmark completes)
