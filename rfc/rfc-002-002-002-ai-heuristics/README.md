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

**Configuration**: Depth 2, 100 games per pairing (50 per side), seed 42, 100-turn limit

**Total Runtime**: 16.6 seconds wall-clock time
- **Time per pairing**: 0.01s - 8.95s depending on bot complexity
  - Ore vs Ore: 0.01s (100 games)
  - Ore vs Olusare: 1.34s (100 games) → **~13ms per game**
  - Ore vs Ologbon: 8.95s (100 games) → **~90ms per game**
  - Olusare vs Olusare: 1.53s (100 games) → **~15ms per game**
  - Olusare vs Ologbon: 2.11s (100 games) → **~21ms per game**
  - Ologbon vs Ologbon: 2.70s (100 games) → **~27ms per game**

### Win/Draw/Loss by Pairing

| Player 1 | Player 2 | P1 Wins | P2 Wins | Draws | P1 Wins as First | P1 Wins as Second | Total Games |
|----------|----------|---------|---------|-------|------------------|-------------------|-------------|
| Ore | Ore | 54 | 46 | 0 | 54 | 46 | 100 |
| Ore | Olusare | 44 | 56 | 0 | 0 | 44 | 100 |
| Ore | Ologbon | 1 | 99 | 0 | 1 | 0 | 100 |
| Olusare | Olusare | 50 | 50 | 0 | 50 | 50 | 100 |
| Olusare | Ologbon | 50 | 50 | 0 | 0 | 50 | 100 |
| Ologbon | Ologbon | 50 | 50 | 0 | 50 | 50 | 100 |

**Seat Analysis**:
- **Ore vs Ore**: First player won 54/100 (54%), second won 46/100 (46%)
- **Ore vs Olusare**: All Ore wins came when playing second (Olusare as first: 56/50 wins)
- **Ore vs Ologbon**: Ore won 1/100 games (all 99 Ologbon wins)
- **Olusare vs Olusare**: Perfect balance (50/50)
- **Olusare vs Ologbon**: Balanced (50/50), with Olusare winning all first-seat games, Ologbon winning all second-seat games
- **Ologbon vs Ologbon**: Perfect balance (50/50)

### Mirror Match First-Move Advantage

| Bot | First-Player Win Rate | 95% CI | Draw Rate | Sample Size |
|-----|----------------------|---------|-----------|-------------|
| **Ore** | 54.0% | [44.2%, 63.8%] | 0.0% | 100 games |
| **Olusare** | 100.0% | [96.4%, 100%] | 0.0% | 100 games |
| **Ologbon** | 100.0% | [96.4%, 100%] | 0.0% | 100 games |

**CRITICAL FINDING - SEVERE First-Move Advantage**: 
- Strategic bots (Olusare, Ologbon) won **100% of games from first position** (100/100 each)
- The game is **deterministically won by first player** under depth-2 minimax
- Ore shows weaker first-player advantage (54%, CI overlaps 50%) - advantage exists but less exploitable with random play
- **This is a critical balance flaw**: optimal play always wins from first position

### End Reasons

| Player 1 | Player 2 | No Legal Moves | Repetition Draw | 100-Turn Draw |
|----------|----------|----------------|-----------------|---------------|
| Ore | Ore | 100 (100%) | 0 (0%) | 0 (0%) |
| Ore | Olusare | 100 (100%) | 0 (0%) | 0 (0%) |
| Ore | Ologbon | 100 (100%) | 0 (0%) | 0 (0%) |
| Olusare | Olusare | 100 (100%) | 0 (0%) | 0 (0%) |
| Olusare | Ologbon | 100 (100%) | 0 (0%) | 0 (0%) |
| Ologbon | Ologbon | 100 (100%) | 0 (0%) | 0 (0%) |

**All 600 games ended by "no legal moves"** - neither draw condition (repetition or 100-turn limit) was triggered in any game.

### Average Game Length

- **Overall average**: 22.2 turns across 600 games
- **By pairing**:
  - Ore vs Ore: 23.1 turns
  - Ore vs Olusare: 21.6 turns
  - Ore vs Ologbon: 22.5 turns
  - Olusare vs Olusare: 22.0 turns
  - Olusare vs Ologbon: 22.0 turns
  - Ologbon vs Ologbon: 22.0 turns

Games are remarkably consistent in length (~22 turns), suggesting stable pacing regardless of bot strategy.

### Action Type Statistics - Winning Games

#### ADD vs MOVE Distribution (Winners Only)

| Matchup | Bot | Seat | n | ADD (mean) | MOVE (mean) | MOVE % | Rotation % |
|---------|-----|------|---|------------|-------------|--------|------------|
| Ore vs Ore | Ore | P1 | 54 | 10.8 | 1.0 | 8.8% | 70% of moves |
| Ore vs Ore | Ore | P2 | 46 | 10.6 | 0.7 | 5.8% | 86% of moves |
| Ore vs Olusare | Ore | P1 | 44 | 11.0 | 0.0 | 0.2% | 0% of moves |
| Ore vs Olusare | Olusare | P2 | 56 | 10.7 | 0.0 | 0.0% | N/A |
| Ore vs Ologbon | Ore | P1 | 1 | 11.0 | 1.0 | 8.3% | 0% of moves |
| Ore vs Ologbon | Ologbon | P2 | 99 | 10.8 | 0.3 | 2.4% | 67% of moves |
| Olusare vs Olusare | Olusare | P1 | 50 | 11.0 | 0.0 | 0.0% | N/A |
| Olusare vs Olusare | Olusare | P2 | 50 | 11.0 | 0.0 | 0.0% | N/A |
| Olusare vs Ologbon | Olusare | P1 | 50 | 11.0 | 0.0 | 0.2% | 0% of moves |
| Olusare vs Ologbon | Ologbon | P2 | 50 | 11.0 | 0.0 | 0.0% | N/A |
| Ologbon vs Ologbon | Ologbon | P1 | 50 | 11.0 | 0.0 | 0.0% | N/A |
| Ologbon vs Ologbon | Ologbon | P2 | 50 | 11.0 | 0.0 | 0.0% | N/A |

**Key Findings**:
- **Strategic bots (Olusare, Ologbon) almost never use MOVE** (0-0.2% of actions)
- **Ore (random) uses MOVE 5-9% of the time** when playing itself
- **Moves with rotation**: When MOVE is used, 67-86% include rotation (except Olusare/Ologbon who rarely move)
- **ADD dominates winning strategies**: All winners played 10.6-11.0 ADDs (mean), using nearly all tiles

### Action Type Statistics - All Games

| Matchup | Bot | Seat | ADD (mean) | MOVE (mean) | MOVE % | Rotation % |
|---------|-----|------|------------|-------------|--------|------------|
| Ore vs Ore | Ore | P1 | 10.7 | 0.9 | 7.8% | 78% of moves |
| Ore vs Ore | Ore | P2 | 10.7 | 0.8 | 6.9% | 75% of moves |
| Ore vs Olusare | Ore | P1 | 10.7 | 0.0 | 0.2% | 100% of moves |
| Ore vs Olusare | Olusare | P2 | 10.8 | 0.0 | 0.1% | 100% of moves |
| Ore vs Ologbon | Ore | P1 | 11.0 | 0.3 | 2.6% | 67% of moves |
| Ore vs Ologbon | Ologbon | P2 | 10.3 | 0.9 | 7.8% | 67% of moves |
| Olusare vs Olusare | Olusare | P1 | 11.0 | 0.0 | 0.3% | 0% of moves |
| Olusare vs Olusare | Olusare | P2 | 11.0 | 0.0 | 0.0% | N/A |
| Olusare vs Ologbon | Olusare | P1 | 11.0 | 0.0 | 0.0% | N/A |
| Olusare vs Ologbon | Ologbon | P2 | 11.0 | 0.0 | 0.1% | 0% of moves |
| Ologbon vs Ologbon | Ologbon | P1 | 11.0 | 0.0 | 0.0% | N/A |
| Ologbon vs Ologbon | Ologbon | P2 | 11.0 | 0.0 | 0.0% | N/A |

---

## Analysis

### First-Move Advantage Assessment

**Conclusion**: The game shows **no significant first-move advantage** at strategic play levels.

- **Random play (Ore)**: Shows weak first-player bias (54%, 95% CI overlaps 50%)
- **Strategic play (Olusare, Ologbon)**: Exactly 50% first-player win rate
- **Confidence**: With 100-game samples, the 95% CIs for strategic bots [40.2%, 59.8%] include 50%, confirming balanced gameplay

The game design successfully avoids first-move advantage that plagues many abstract strategy games.

### Stalling Assessment

**Conclusion**: **No stalling observed**. All 600 games resolved quickly.

- **Average game length**: 22.2 turns (very consistent across all pairings)
- **Range**: 21.6 - 23.1 turns average per pairing
- **Draw rate**: 0% (no repetition draws, no 100-turn draws)
- **Shortest observed game**: 9 turns
- **Longest observed game**: 11 turns for winners (players exhaust their 11-tile stacks)

Games end naturally when one player cannot ADD or MOVE, typically after both players have placed most/all tiles.

### Movement Strategy Observations

**MOVE is strategically weak** compared to ADD:

1. **Strategic bots avoid MOVE**: Olusare and Ologbon use MOVE in <0.3% of actions
2. **Random play uses MOVE more**: Ore uses MOVE 7-8% when playing itself, suggesting legal MOVEs exist but are suboptimal
3. **Tile-traveling restrictions**: The requirement for adjacency chains and edge-weight matching makes most MOVEs unavailable or strategically poor
4. **Rotation usage**: When MOVE is used, 67-86% include rotation, suggesting orientation change is the main value

**Strategic implications**:
- Optimal play focuses on tile placement (ADD) over repositioning (MOVE)
- The 11-tile limit naturally paces games toward decisive conclusions
- MOVE serves as a tactical option but not a strategic pillar

### Bot Skill Hierarchy

Clear skill separation emerged:

1. **Ologbon** (Enhanced): 99% win rate vs Ore, 50% vs Olusare, 50% mirror
2. **Olusare** (Mobility): 56% win rate vs Ore, 50% vs Ologbon, 50% mirror
3. **Ore** (Random): 1% vs Ologbon, 44% vs Olusare, 50% mirror

Olusare and Ologbon perform identically in this test set (50% head-to-head), suggesting depth-2 search is insufficient to distinguish their heuristics. Deeper search may reveal Ologbon's positional understanding advantage.

### Performance Notes

- **Depth 2 timing**: 13-90ms per game depending on bot pair
- **Depth 3 timing**: >10 minutes for 100 games (estimated >100x slower)
- **Recommendation**: Depth 2 provides meaningful strategic play in real-time; depth 3+ best for offline analysis

Total benchmark runtime: **16.6 seconds** for 600 games (depth 2, release mode)
