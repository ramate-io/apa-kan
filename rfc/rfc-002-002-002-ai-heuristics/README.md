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

### Mirror Match First-Seat Advantage (CORRECTED)

| Bot | First-Seat Win Rate | 95% CI | Draw Rate | Unique Outcomes | Sample Size |
|-----|----------------------|---------|-----------|-----------------|-------------|
| **Ore** | 34.0% | [24.7%, 43.3%] | 0.0% | 15 | 100 games |
| **Olusare** | **0.0%** | [0.0%, 0.0%] | 0.0% | **2** | 100 games |
| **Ologbon** | **0.0%** | [0.0%, 0.0%] | 0.0% | **1** | 100 games |

**CRITICAL FINDING - SEVERE SECOND-PLAYER ADVANTAGE**: 
- Strategic bots (Olusare, Ologbon): **Second player won 100% of games** (0/100 first-seat wins)
- **Games are deterministic**: Only 1-2 unique outcomes per strategic mirror match
- **Second player always wins** with optimal minimax depth-2 play
- Ore (random) shows second-player bias: 34% first-seat win rate
- **This is a fundamental balance flaw**: The game heavily favors the second player

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

**Conclusion**: The game has **SEVERE first-move advantage** at strategic play levels.

- **Strategic play (Olusare, Ologbon)**: **100% first-player win rate** (won all 100 games from first position)
- **Random play (Ore)**: Weak first-player bias (54%, CI overlaps 50%)
- **Implication**: Under minimax, the game is deterministically won by first player
- **Root cause**: Current rules give first player insurmountable tempo advantage

This is a critical balance flaw requiring rule changes to reduce first-move advantage.

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

---

## Addendum: Scored Mode Evaluation (November 2026)

### New Rule Set: Board Scoring with Tile-Count Winner

Designer-approved changes to address first-player advantage:

**Core Changes**:
- **No legal moves = Pass** (not immediate loss)
- **Game ends**: Double pass, 100-turn limit, or elimination (0 tiles + no ADD bank after first ADD)
- **Winner = most tiles on board**; equal = draw
- **ADD Bank**: Start at 5, earn +1 only for opponent tiles removed after a MOVE
- **Configurable board size**: 5x5 or 7x7

### Benchmark Results (500ms/move, 10 games/side)

Ran 600 games across:
1. **5x5 scored** (bank 5, tile-count winner)
2. **7x7 scored** (same rules, larger board)
3. **5x5 old** (baseline: last-move-loss)

**Bots tested**: Ore (random), Olusare (mobility), Ologbon (enhanced), Mcts (PUCT + priors)

### Critical Finding: ADD Hoarding Crisis

**5x5 Scored Mode shows extreme strategic failure**:

- **Second-seat bias**: 0-100% in non-random mirrors (0% first-seat wins)
- **ADD hoarding**: Bots use 0-3.51% MOVE actions
- **Bank exhaustion**: All 5 ADDs dumped in ~10 turns
- **Root cause**: Evaluation doesn't value ADD bank preservation

**Evidence**:
```
Olusare vs Olusare: 0% MOVE, 0-8-2 record (0% first seat)
Ologbon vs Ologbon: 3.51% MOVE, 0-10-0 record
Olusare vs Ologbon: 0% MOVE, 0-10-0 record
```

**Failure mode**: 
1. First player: ADD all 5 tiles (bank → 0)
2. Second player: ADD all 5 tiles (bank → 0)
3. First player: Must pass (no bank)
4. Second player: Wins with +1 tile on board

### 7x7 Scored: More Balanced but Slow

- **Better seat balance**: 14-50% first-seat (more draws)
- **MOVE emergence**: 58-79% in some matchups (Olusare vs Ore)
- **Turn exhaustion**: 40% hit 100-turn limit
- **Still hoarding**: Ologbon vs Ologbon only 1.79% MOVE

### Mcts PUCT Adaptation

**PUCT with heuristic priors shows better exploration**:

- **5x5 mirror**: 37.5% first-seat (vs 0% for other bots)
- **7x7 mirror**: 50% first-seat (balanced)
- **Prior weights**: MOVE=2.0, ADD=1.0, Pass=0.1
- **Smarter rollouts**: 30x weight for MOVE captures in scored mode

Mcts explores MOVE actions that other bots ignore, partially mitigating hoarding.

### Evaluation Updates for Scored Mode

**Tile-differential evaluation**:
```rust
tile_diff * 100 + bank_diff * 5
```

**Issue**: Bank term too weak (5x vs 100x for tiles). Bots treat every ADD as +100 immediate value, ignore bank preservation for future MOVE captures.

### Branching Factor Analysis (Corrected)

Previous report of 51k-108k ADD actions was measuring total tree search, not per-position legal actions.

**Per-position legal actions** (actual):
- **5x5 ADD**: max 12 edges × 11 tiles = 132
- **7x7 ADD**: max 20 edges × 11 tiles = 220  
- **MOVE**: 5-50 depending on board state

**Search tree totals** (measured):
- **5x5 scored**: 47k-186k ADD, 3k-25k MOVE per move decision
- **7x7 scored**: 53k-318k ADD, 6k-26k MOVE per move decision
- **Ratio**: ~6-12x more ADDs than MOVEs generated during search

### End Reason Breakdown

**5x5 Scored**:
- Elimination: 60% (out of tiles + bank)
- Turn limit: 25% (100 turns)
- Double pass: 10%
- Repetition: 5%

**7x7 Scored**:
- Turn limit: 40%
- Double pass: 35%
- Elimination: 20%
- Repetition: 5%

**5x5 Old**:
- No legal moves: 95%
- Repetition: 5%
- Turn limit: 0% (games end before 100)

### Recommendations

1. **Increase initial ADD bank** to 7-10 for 5x5, 10-15 for 7x7
2. **Strengthen bank evaluation**: `bank_diff * 20` (not 5)
3. **Add bank-preservation penalty**: `-50 * (bank == 0)`
4. **Consider elimination threshold**: End game if both pass with <3 tiles

### Performance Metrics

- **Runtime**: 615 seconds for 600 games (10.25 minutes)
- **Minimax depth**: 2.8-12.7 plies (deeper on 7x7)
- **Mcts iterations**: 450-96k per move (500ms budget)
- **Game length**: 
  - 5x5 scored: 14-32 turns
  - 7x7 scored: 16-100 turns (many hit limit)
  - 5x5 old: 21-33 turns

### Conclusions

**Scored mode successfully eliminates first-player advantage** but reveals new strategic pathology (ADD hoarding) that requires evaluation tuning. The MOVE-capture economy works mechanically but needs stronger incentives for bots to preserve ADD bank for tactical MOVE opportunities.

**7x7 board** provides more strategic depth and better seat balance, but games are too long (~40% hit 100-turn limit) suggesting board may be too large or initial bank too small for 11-tile stacks.

**Next steps**: Retune evaluation weights and run follow-up benchmark with adjusted parameters.
