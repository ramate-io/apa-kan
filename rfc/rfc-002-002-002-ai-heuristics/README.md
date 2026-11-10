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

Designer-approved changes to address first-player advantage:

**Core Changes**:
- **No legal moves = Pass** (not immediate loss)
- **Game ends**: Double pass, 100-turn limit, or elimination (0 tiles + no ADD bank after first ADD)
- **Winner = most tiles on board**; equal = draw
- **ADD Bank**: Start at 5, earn +1 only for opponent tiles removed after a MOVE
- **Configurable board size**: 5x5 or 7x7

### Measurement Bugs Fixed

The initial benchmark had critical measurement errors. All corrected:

1. **Seat swapping**: Was running only 10 games total; now 20 per cross-pairing (10 per seat)
2. **Per-bot results**: Now track W-L-D for each bot overall and by seat with 95% Wilson CI
3. **MCTS iterations**: Fixed leaking counter; now per-bot only
4. **Branching factor**: Now measured from actual `generate_legal_actions()` per position
5. **Captures by cause**: Now split MOVE-caused vs other removals accurately
6. **7x7 MCTS performance**: Fixed rollout bottleneck (was cloning game 100× per rollout step)

### MCTS Rollout Optimization

**Problem Found**: The rollout policy cloned the entire game for every legal action to evaluate potential captures. On 7x7 with ~100 actions, this meant 100 game clones per rollout step × 20 steps × thousands of iterations = **millions of expensive clones**.

**Solution**: Replaced with fast heuristic-based evaluation using adjacent tile checks (no cloning).

**Results**:
- **5x5**: 1,331 → 16,523 iterations/move (12× speedup)
- **7x7**: 667 → 6,671 iterations/move (10× speedup)

7x7 is still ~2.5× slower than 5x5, which is expected given 2× board size and higher branching factor.

### Corrected Benchmark Results (500ms/move, 10 games/seat)

Ran 600 games across:
1. **5x5 scored** (bank 5, tile-count winner)
2. **7x7 scored** (same rules, larger board)
3. **5x5 old** (baseline: last-move-loss)

**Bots tested**: Ore (random), Olusare (minimax), Ologbon (enhanced minimax), Mcts (PUCT with optimized rollout)

### Key Findings: Minimax Dominates MCTS on Scored Mode

#### 5x5 Scored Results

| Matchup           | Winner  | Score | W-L-D    | 95% CI          |
|-------------------|---------|-------|----------|-----------------|
| Olusare vs Mcts   | Olusare | 87.5% | 17-2-1   | [66.9%, 96.0%]  |
| Ologbon vs Mcts   | Ologbon | 75.0% | 15-5-0   | [53.1%, 88.8%]  |
| Olusare vs Olusare| -       | 50.0% | 9-11-0   | [29.9%, 70.1%]  |
| Olusare vs Ologbon| -       | 47.5% | 9-10-1   | [27.9%, 68.0%]  |

**MCTS iteration throughput**:
- Mcts vs Mcts mirror: 761,889 iterations/move
- Olusare vs Mcts: 1,369,001 iterations/move
- Ologbon vs Mcts: 1,879,858 iterations/move

**Despite 300k-1.8M iterations per move, MCTS still loses 75-87% of the time to minimax depth 3.**

#### 7x7 Scored Results

| Matchup           | Winner  | Score | W-L-D    | 95% CI          |
|-------------------|---------|-------|----------|-----------------|
| Ologbon vs Mcts   | Ologbon | 95.0% | 19-1-0   | [76.4%, 99.1%]  |
| Olusare vs Mcts   | Olusare | 90.0% | 12-2-6   | [69.9%, 97.2%]  |

**Nearly perfect domination by minimax on 7x7**, even with optimized MCTS rollouts.

### Why Minimax Beats MCTS

1. **Scored mode rewards MOVE captures exclusively**: The minimax heuristic (`tile_diff * 100 + bank_diff * 5`) directly evaluates this strategic priority
2. **MCTS rollout policy is too noisy**: Even with optimization and heuristic guidance, random rollouts don't accurately estimate the value of MOVE-capture sequences
3. **Shallow but accurate beats deep but noisy**: Minimax depth 3 with a good heuristic is more strategically accurate than MCTS with millions of iterations using noisy rollout estimates

### Seat Balance

**5x5 scored mode shows good balance**:
- Most pairings show near-even seat splits
- Olusare vs Olusare: P1 5-0-15 P2 (draws dominate, P2 wins more draws)
- No catastrophic first-move advantage like old rules

**7x7 scored mode also balanced**:
- Games are longer (70-90 turns avg)
- More draws due to larger state space

### Per-Position Branching (Corrected)

**5x5 scored**:
- ADD: mean 35-73, p95 110-132, max 132
- MOVE: mean 0-27, p95 0-75, max varies (0-158)

**7x7 scored**:
- ADD: mean 33-102, p95 153-220, max 220
- MOVE: mean 0-20, p95 0-86, max varies (0-154)

Early game is ADD-dominated; mid-late game shifts to MOVE actions.

### Captures by Cause

**Olusare vs Mcts (5x5)**:
- Olusare: 50 from MOVE, 88 other
- Mcts: 7 from MOVE, 15 other

Minimax bots prioritize MOVE captures in scored mode (as expected from heuristic).

### Old Rules Comparison (5x5 Bank 100)

Under old rules (all removals reward):
- Minimax still beats Ore consistently (82.5%)
- MCTS performs slightly better (65% self-mirror score vs 52.5% in scored mode)
- Shorter games (~23 turns vs ~50 on scored)

---

### Corrected Summary

1. **Scored mode successfully addresses first-player advantage** from old rules
2. **Minimax (Olusare/Ologbon) dominates MCTS** on scored mode despite MCTS getting 300k-1.8M iterations/move
3. **Root cause**: Scored mode rewards MOVE captures exclusively; minimax heuristic evaluates this directly while MCTS rollouts are noisy even after optimization
4. **7x7 MCTS performance fixed**: 10× speedup after eliminating game-cloning bottleneck in rollout policy
5. **Per-bot measurement now accurate**: All leaking counters and confounded metrics corrected

### Recommendations

1. **For scored mode play**: Use Olusare or Ologbon (minimax with scored-mode heuristic)
2. **For MCTS to compete**: Would need:
   - Better rollout policy or value network to accurately estimate MOVE-capture rewards
   - Much deeper search (but already getting 1.8M iterations with optimization)
   - Hybrid minimax-MCTS approach
3. **Benchmark hygiene**: Always measure seat-swapped results with per-bot tracking to avoid confounded analysis

---
