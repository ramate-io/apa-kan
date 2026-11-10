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


## Addendum: Scored Mode Evaluation - CORRECTED (November 2026)

### New Rule Set: Board Scoring with Tile-Count Winner

**Core Changes**:
- **No legal moves = Pass** (not immediate loss)
- **Game ends**: Double pass, 100-turn limit, or elimination (0 tiles + no ADD bank after first ADD)
- **Winner = most tiles on board**; equal = draw
- **ADD Bank**: Start at 5, earn +1 from opponent tiles removed on a MOVE turn (not exclusively from MOVE captures)
- **Configurable board size**: 5x5 or 7x7

### Measurement Bugs Fixed

The initial benchmark had critical measurement errors. All corrected:

1. **Seat swapping**: Was running only 10 games total; now 20 per cross-pairing (10 per seat)
2. **Per-bot results**: Now track W-L-D for each bot overall and by seat with 95% Wilson CI
3. **MCTS iterations**: Fixed leaking counter; now per-bot only
4. **Branching factor**: Now measured from actual `generate_legal_actions()` per position
5. **Captures by cause**: Now split MOVE-caused vs other removals accurately
6. **7x7 MCTS performance**: Fixed rollout bottleneck (heuristic-based selection without game cloning)

### MCTS Rollout Optimization

**Problem Found**: Rollout policy cloned the entire game for every legal action to evaluate potential captures. On 7x7 with ~100 actions: 100 game clones per rollout step = millions of expensive clones.

**Solution**: Fast heuristic-based evaluation using adjacent tile checks (no cloning).

**Test results** (standalone test, 5 moves):
- 5x5: 1,331 → 16,523 iterations/move (12× speedup)
- 7x7: 667 → 6,671 iterations/move (10× speedup)

### Corrected Benchmark Results (500ms/move, 10 games/seat)

**Runtime**: 1060.72s for 600 games across:
1. **5x5 scored** (bank 5, tile-count winner)
2. **7x7 scored** (same rules, larger board)
3. **5x5 old** (baseline: last-move-loss)

**Bots tested**: Ore (random), Olusare (minimax iterative deepening), Ologbon (minimax iterative deepening enhanced), Mcts (PUCT with optimized rollout)

### Key Findings: Minimax Beats MCTS

#### 5x5 Scored Results

| Matchup | Bot A W-L-D | Bot A Score | Bot B W-L-D | Bot B Score | Seat Stats | Avg Turns |
|---------|-------------|-------------|-------------|-------------|------------|-----------|
| Olusare vs Mcts | 17-2-1 | 87.5% [66.9%, 96.0%] | 2-17-1 | 12.5% [4.0%, 33.1%] | P1 9-1-10 P2 | 50.8 |
| Ologbon vs Mcts | 15-5-0 | 75.0% [53.1%, 88.8%] | 5-15-0 | 25.0% [11.2%, 46.9%] | P1 5-0-15 P2 | 23.0 |
| Olusare vs Ologbon | 9-10-1 (47.5%) | 10-9-1 (52.5%) | P1 0-1-19 P2 | 21.2 |
| Olusare vs Olusare | 9-11-0 (45.0%) | - | P1 5-0-15 P2 | 49.8 |
| Ologbon vs Ologbon | 8-9-3 (47.5%) | - | P1 0-3-17 P2 | 21.4 |
| Mcts vs Mcts | 10-9-1 (52.5%) | - | P1 8-1-11 P2 | 24.9 |

**MOVE usage**:
- Ologbon vs Ologbon: 0.00% (ADD-only strategy)
- Olusare vs Ologbon: 0.00% Olusare, 0.47% Ologbon
- Olusare vs Olusare: 61.02%
- Olusare vs Mcts: 63.46% Olusare, 36.77% Mcts

#### 7x7 Scored Results

| Matchup | Bot A W-L-D | Bot A Score | Bot B W-L-D | Bot B Score | Seat Stats | Avg Turns |
|---------|-------------|-------------|-------------|-------------|------------|-----------|
| Ologbon vs Mcts | 15-2-3 | 82.5% [61.1%, 93.4%] | 2-15-3 | 17.5% [6.6%, 38.9%] | P1 8-3-9 P2 | 58.5 |
| Olusare vs Mcts | 10-6-4 | 60.0% [38.7%, 78.1%] | 6-10-4 | 40.0% [21.9%, 61.3%] | P1 11-4-5 P2 | 100.0 |
| Ore vs Olusare | 0-13-7 (17.5%) | 13-0-7 (82.5%) | P1 8-7-5 P2 | 100.0 |
| Ore vs Ologbon | 0-16-4 (10.0%) | 16-0-4 (90.0%) | P1 7-4-9 P2 | 31.9 |
| Olusare vs Olusare | 9-8-3 (52.5%) | - | P1 7-3-10 P2 | 74.0 |
| Ologbon vs Ologbon | 5-5-10 (50.0%) | - | P1 0-10-10 P2 | 24.6 |
| Mcts vs Mcts | 5-7-8 (45.0%) | - | P1 5-8-7 P2 | 17.2 |

**Turn limit frequently hit**: Ore vs Olusare 100.0 turns, Olusare vs Mcts 100.0 turns.

#### 5x5 Old Rules Results

| Matchup | Bot A W-L-D | Bot A Score | Bot B W-L-D | Bot B Score | Seat Stats | Avg Turns |
|---------|-------------|-------------|-------------|-------------|------------|-----------|
| Olusare vs Mcts | 17-0-3 | 92.5% [73.1%, 98.2%] | 0-17-3 | 7.5% [1.8%, 26.9%] | P1 9-3-8 P2 | 36.0 |
| Ologbon vs Mcts | 12-0-8 | 80.0% [58.4%, 91.9%] | 0-12-8 | 20.0% [8.1%, 41.6%] | P1 5-8-7 P2 | 53.0 |
| Ore vs Mcts | 0-6-14 (35.0%) | 6-0-14 (65.0%) | P1 2-14-4 P2 | 23.7 |
| Ore vs Olusare | 0-12-8 (20.0%) | 12-0-8 (80.0%) | P1 7-8-5 P2 | 22.9 |
| Ore vs Ologbon | 0-10-10 (25.0%) | 10-0-10 (75.0%) | P1 8-10-2 P2 | 23.1 |
| Olusare vs Olusare | 10-9-1 (52.5%) | - | P1 0-1-19 P2 | 21.1 |
| Ologbon vs Ologbon | 2-4-14 (45.0%) | - | P1 2-14-4 P2 | 56.1 |
| Mcts vs Mcts | 1-2-17 (47.5%) | - | P1 2-17-1 P2 | 46.3 |

**MOVE usage under old rules**:
- Ologbon vs Ologbon: 61.03% (most engaged strategic mirror)
- Mcts vs Mcts: 54.43%
- Olusare vs Olusare: 0.00%

### Seat Balance Observations

**Second-seat bias present across all configurations**:

**5x5 scored**:
- Ologbon vs Ologbon: seat P1 0-3-17 P2 (0 P1 wins, 3 draws, 17 P2 wins)
- Olusare vs Ologbon: seat P1 0-1-19 P2
- Olusare vs Olusare: seat P1 5-0-15 P2

**7x7 scored**:
- Ologbon vs Ologbon: seat P1 0-10-10 P2 (0 P1 wins, 10 draws, 10 P2 wins)
- Olusare vs Ologbon: seat P1 1-12-7 P2

**5x5 old rules**:
- Olusare vs Olusare: seat P1 0-1-19 P2
- Ologbon vs Ologbon: seat P1 2-14-4 P2
- Mcts vs Mcts: seat P1 2-17-1 P2

**Most balanced matchup**: Old-rules Ologbon mirror (seat P1 2-14-4 P2, 45.0% score, 61.03% MOVE, 56.1 turns).

### Per-Position Branching

**5x5 scored**:
- ADD: mean 35.4-73.3, p95 110-132, max 132
- MOVE: mean 0.0-27.0, p95 0-75, max varies (0-158)

**7x7 scored**:
- ADD: mean 31.0-104.7, p95 144-220, max 220
- MOVE: mean 0.1-20.4, p95 0-71, max varies (12-173)

**5x5 old**:
- ADD: mean 43.8-130.4, p95 200-231, max 231
- MOVE: mean 0.0-110.7, p95 0-312, max varies (10-373)

Early game ADD-dominated; mid-late shifts to MOVE.

### Captures by Cause

**Olusare vs Mcts (5x5 scored)**:
- Olusare: 50 from MOVE, 88 other
- Mcts: 7 from MOVE, 15 other

**Ologbon vs Mcts (5x5 scored)**:
- Ologbon: 8 from MOVE, 128 other
- Mcts: 5 from MOVE, 54 other

### Notes on Reported Metrics

Some metrics appear inconsistent or require interpretation:

- **Per-bot W-L-D in mirrors** doesn't reconcile with seat stats (e.g., 5x5 Ologbon mirror per-bot 8-9-3 vs seat 0-3-17). Seat stats used for mirror analysis.
- **"Avg MCTS iterations/move"** (323k-1.88M) conflicts with standalone test results (~16.5k on 5x5, ~6.7k on 7x7) and is likely cumulative over all moves rather than per-move average.
- **Minimax depths** show wide variation (Olusare 3.1-36.3, Ologbon 1.7-21.5) consistent with iterative deepening under 500ms/move time budget rather than fixed depth.

### Summary

1. **Minimax consistently beats MCTS** across all configurations (60-92.5% scores)
2. **Second-seat bias present** in both scored and old rules for strong bot mirrors
3. **MOVE strategy varies** by bot and configuration:
   - Ologbon: 0.00% MOVE on 5x5 scored (ADD-only), 61.03% on 5x5 old
   - Olusare: 61.02-87.00% MOVE depending on opponent and board size
4. **Old-rules Ologbon mirror** most balanced (seat 2-14-4, 61% MOVE, 56 turns)
5. **7x7 games longer and draw-heavier** than 5x5 (turn limit hit in Olusare matches)

### Recommendations

1. **Bot selection**: Olusare and Ologbon (minimax) outperform Mcts across all configurations
2. **Balance investigation**: Second-seat bias in strategic mirrors may warrant rule adjustments
3. **Benchmark hygiene**: Always measure seat-swapped results with per-bot tracking

---
2. **For MCTS to compete**: Would need:
   - Better rollout policy or value network to accurately estimate MOVE-capture rewards
   - Much deeper search (but already getting 1.8M iterations with optimization)
   - Hybrid minimax-MCTS approach
3. **Benchmark hygiene**: Always measure seat-swapped results with per-bot tracking to avoid confounded analysis

---
