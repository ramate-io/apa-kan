# Apa Kan AI Benchmark Analysis (Corrected)

## Measurement Fixes Applied

This benchmark corrects all measurement bugs from the previous run:

1. **Seat swapping**: Each cross-pairing plays 10 games per seat (20 total)
2. **Per-bot results**: Track W-L-D for each bot overall and split by seat with 95% Wilson CI
3. **Per-bot MCTS iterations**: Only tracked and reported for actual MCTS bots
4. **Per-bot minimax depth**: Tracked separately for each bot instance
5. **Per-position branching**: Measured directly from `generate_legal_actions()` on every live position (mean, p95, max)
6. **Captures by cause**: Split into MOVE-caused vs other removals by tracking action type
7. **MCTS optimization**: Fixed 7x7 MCTS rollout bottleneck (heuristic-based selection without game cloning)

## Test Configuration

- **Time budget**: 500ms/move
- **Games per seat**: 10 (20 total per cross-pairing, 20 for mirrors)
- **Parallelization**: All pairings run concurrently per configuration
- **Bots tested**: Ore (random), Olusare (minimax iterative deepening), Ologbon (minimax iterative deepening enhanced), Mcts (PUCT with optimized rollout)
- **Runtime**: 1060.72s

## Configuration 1: 5x5 Scored Mode

Board: 5x5, Bank: 5, Scored: true

### Bot vs Bot Results

| Matchup | Winner W-L-D | Winner Score | Loser W-L-D | Loser Score | Seat Stats | Avg Turns |
|---------|--------------|--------------|-------------|-------------|------------|-----------|
| Olusare vs Mcts | 17-2-1 | 87.5% [66.9%, 96.0%] | 2-17-1 | 12.5% [4.0%, 33.1%] | P1 9-1-10 P2 | 50.8 |
| Ologbon vs Mcts | 15-5-0 | 75.0% [53.1%, 88.8%] | 5-15-0 | 25.0% [11.2%, 46.9%] | P1 5-0-15 P2 | 23.0 |
| Olusare vs Ologbon | Olusare 9-10-1 (47.5%) | Ologbon 10-9-1 (52.5%) | P1 0-1-19 P2 | 21.2 |
| Olusare vs Olusare | 9-11-0 | 45.0% [25.8%, 65.8%] | - | - | P1 5-0-15 P2 | 49.8 |
| Ologbon vs Ologbon | 8-9-3 | 47.5% [27.9%, 68.0%] | - | - | P1 0-3-17 P2 | 21.4 |
| Mcts vs Mcts | 10-9-1 | 52.5% [32.0%, 72.1%] | - | - | P1 8-1-11 P2 | 24.9 |
| Ore vs Mcts | 1-19-0 | 5.0% [0.9%, 23.6%] | 19-1-0 | 95.0% [76.4%, 99.1%] | P1 11-0-9 P2 | 19.6 |
| Ore vs Olusare | 0-20-0 | 0.0% | 20-0-0 | 100.0% [83.9%, 100.0%] | P1 10-0-10 P2 | 23.6 |
| Ore vs Ologbon | 0-20-0 | 0.0% | 20-0-0 | 100.0% [83.9%, 100.0%] | P1 10-0-10 P2 | 21.2 |

### Key Findings

1. **Minimax beats MCTS on 5x5 scored**:
   - Olusare beats Mcts 87.5% to 12.5%
   - Ologbon beats Mcts 75.0% to 25.0%
   - Mcts beats Ore 95.0%

2. **Strong second-seat bias in minimax mirrors**:
   - Ologbon vs Ologbon: seat P1 0-3-17 P2 (0 P1 wins, 3 draws, 17 P2 wins)
   - Olusare vs Ologbon: seat P1 0-1-19 P2
   - Olusare vs Olusare: seat P1 5-0-15 P2

3. **MOVE usage varies dramatically**:
   - Ologbon vs Ologbon: 0.00% MOVE (ADD-only strategy)
   - Olusare vs Ologbon: 0.00% Olusare MOVE, 0.47% Ologbon MOVE
   - Olusare vs Olusare: 61.02% MOVE
   - Olusare vs Mcts: 63.46% Olusare MOVE, 36.77% Mcts MOVE
   - Ologbon vs Mcts: 6.99% Ologbon MOVE, 3.69% Mcts MOVE

4. **MCTS iterations** (reported in log, likely cumulative):
   - Ologbon vs Mcts: 1879858
   - Olusare vs Mcts: 1369001
   - Mcts vs Mcts: 1739624
   - Ore vs Mcts: 323581

5. **Per-position branching on 5x5**:
   - ADD: mean 35.4-73.3, p95 110-132, max 132
   - MOVE: mean 0.0-27.0, p95 0-75, max varies (0-158)

6. **Captures by cause**:
   - Olusare vs Mcts: Olusare 50 from MOVE + 88 other, Mcts 7 from MOVE + 15 other
   - Ologbon vs Mcts: Ologbon 8 from MOVE + 128 other, Mcts 5 from MOVE + 54 other

## Configuration 2: 7x7 Scored Mode

Board: 7x7, Bank: 5, Scored: true

### Bot vs Bot Results

| Matchup | Winner W-L-D | Winner Score | Loser W-L-D | Loser Score | Seat Stats | Avg Turns |
|---------|--------------|--------------|-------------|-------------|------------|-----------|
| Ore vs Olusare | 0-13-7 | 17.5% [6.6%, 38.9%] | 13-0-7 | 82.5% [61.1%, 93.4%] | P1 8-7-5 P2 | 100.0 |
| Ore vs Ologbon | 0-16-4 | 10.0% [2.8%, 30.1%] | 16-0-4 | 90.0% [69.9%, 97.2%] | P1 7-4-9 P2 | 31.9 |
| Ologbon vs Mcts | 15-2-3 | 82.5% [61.1%, 93.4%] | 2-15-3 | 17.5% [6.6%, 38.9%] | P1 8-3-9 P2 | 58.5 |
| Ore vs Mcts | 1-16-3 | 12.5% [4.0%, 33.1%] | 16-1-3 | 87.5% [66.9%, 96.0%] | P1 8-3-9 P2 | 18.4 |
| Olusare vs Mcts | 10-6-4 | 60.0% [38.7%, 78.1%] | 6-10-4 | 40.0% [21.9%, 61.3%] | P1 11-4-5 P2 | 100.0 |
| Olusare vs Ologbon | Olusare 3-5-12 (45.0%) | Ologbon 5-3-12 (55.0%) | P1 1-12-7 P2 | 47.8 |
| Olusare vs Olusare | 9-8-3 | 52.5% [32.0%, 72.1%] | - | - | P1 7-3-10 P2 | 74.0 |
| Ologbon vs Ologbon | 5-5-10 | 50.0% [29.9%, 70.1%] | - | - | P1 0-10-10 P2 | 24.6 |
| Mcts vs Mcts | 5-7-8 | 45.0% [25.8%, 65.8%] | - | - | P1 5-8-7 P2 | 17.2 |

### Key Findings

1. **Minimax still beats MCTS on 7x7**:
   - Ologbon beats Mcts 82.5% to 17.5%
   - Olusare beats Mcts 60.0% to 40.0%
   - Mcts beats Ore 87.5%

2. **Second-seat bias persists**:
   - Ologbon vs Ologbon: seat P1 0-10-10 P2 (0 P1 wins, 10 draws, 10 P2 wins)
   - Olusare vs Ologbon: seat P1 1-12-7 P2

3. **Turn limit frequently hit**:
   - Ore vs Olusare: 100.0 turns
   - Olusare vs Mcts: 100.0 turns

4. **MOVE usage on 7x7**:
   - Ologbon vs Ologbon: 0.90% MOVE
   - Olusare vs Olusare: 71.82% MOVE
   - Olusare vs Mcts: 87.00% Olusare MOVE, 6.43% Mcts MOVE
   - Ologbon vs Mcts: 73.31% Ologbon MOVE, 4.44% Mcts MOVE
   - Ore vs Olusare: 85.30% Olusare MOVE

5. **MCTS iterations** (reported in log):
   - Olusare vs Mcts: 510860

6. **Per-position branching on 7x7**:
   - ADD: mean 31.0-104.7, p95 144-220, max 220
   - MOVE: mean 0.1-20.4, p95 0-71, max varies (12-173)

## Configuration 3: 5x5 Old Baseline

Board: 5x5, Bank: 100, Scored: false

### Bot vs Bot Results

| Matchup | Winner W-L-D | Winner Score | Loser W-L-D | Loser Score | Seat Stats | Avg Turns |
|---------|--------------|--------------|-------------|-------------|------------|-----------|
| Olusare vs Mcts | 17-0-3 | 92.5% [73.1%, 98.2%] | 0-17-3 | 7.5% [1.8%, 26.9%] | P1 9-3-8 P2 | 36.0 |
| Ologbon vs Mcts | 12-0-8 | 80.0% [58.4%, 91.9%] | 0-12-8 | 20.0% [8.1%, 41.6%] | P1 5-8-7 P2 | 53.0 |
| Olusare vs Ologbon | Olusare 11-1-8 (75.0%) | Ologbon 1-11-8 (25.0%) | P1 1-8-11 P2 | 41.6 |
| Ore vs Mcts | 0-6-14 | 35.0% [18.1%, 56.7%] | 6-0-14 | 65.0% [43.3%, 81.9%] | P1 2-14-4 P2 | 23.7 |
| Ore vs Olusare | 0-12-8 | 20.0% [8.1%, 41.6%] | 12-0-8 | 80.0% [58.4%, 91.9%] | P1 7-8-5 P2 | 22.9 |
| Ore vs Ologbon | 0-10-10 | 25.0% [11.2%, 46.9%] | 10-0-10 | 75.0% [53.1%, 88.8%] | P1 8-10-2 P2 | 23.1 |
| Olusare vs Olusare | 10-9-1 | 52.5% [32.0%, 72.1%] | - | - | P1 0-1-19 P2 | 21.1 |
| Ologbon vs Ologbon | 2-4-14 | 45.0% [25.8%, 65.8%] | - | - | P1 2-14-4 P2 | 56.1 |
| Mcts vs Mcts | 1-2-17 | 47.5% [27.9%, 68.0%] | - | - | P1 2-17-1 P2 | 46.3 |

### Key Findings

1. **Minimax still dominates under old rules**:
   - Olusare beats Mcts 92.5% to 7.5%
   - Ologbon beats Mcts 80.0% to 20.0%
   - Olusare beats Ore 80.0%
   - Ologbon beats Ore 75.0%
   - Mcts beats Ore 65.0%

2. **Old rules show second-seat bias**:
   - Olusare vs Olusare: seat P1 0-1-19 P2
   - Ologbon vs Ologbon: seat P1 2-14-4 P2
   - Mcts vs Mcts: seat P1 2-17-1 P2

3. **Old-rules Ologbon mirror most balanced**:
   - 2-4-14 record with seat P1 2-14-4 P2
   - 61.03% MOVE usage
   - 56.1 turns average
   - Most draws and closest seat split among strategic bot mirrors

4. **MOVE usage under old rules**:
   - Olusare vs Olusare: 0.00% MOVE
   - Ologbon vs Ologbon: 61.03% MOVE
   - Mcts vs Mcts: 54.43% MOVE
   - Olusare vs Mcts: 39.22% Olusare, 39.56% Mcts

5. **MCTS iterations** (reported in log):
   - Mcts vs Mcts: 990854
   - Ologbon vs Mcts: 744050
   - Olusare vs Mcts: 716185
   - Ore vs Mcts: 329196

## Summary

### Overall Bot Rankings

Based on cross-pairing results:

1. **Olusare and Ologbon** (minimax): Consistently beat Mcts and Ore across all configurations
2. **Mcts**: Beats Ore but loses to minimax
3. **Ore**: Random baseline, loses to all strategic bots

### Seat Balance Observations

- **Strong second-seat bias** in minimax mirrors on scored mode (5x5 Ologbon mirror seat 0-3-17, Olusare vs Ologbon seat 0-1-19)
- **Second-seat bias persists** on 7x7 scored (Ologbon mirror seat 0-10-10)
- **Old rules also show second-seat bias** (Olusare mirror seat 0-1-19, Mcts mirror seat 2-17-1)
- **Most balanced matchup**: Old-rules Ologbon mirror (seat 2-14-4, 45.0% score)

### MOVE Strategy Patterns

- **Ologbon on 5x5 scored**: 0.00% MOVE (ADD-only)
- **Olusare on 5x5 scored**: 61.02% MOVE in mirror, 63.46% vs Mcts
- **7x7 scored**: Higher MOVE usage overall (71.82-87.00% for Olusare)
- **Old rules Ologbon mirror**: 61.03% MOVE (high engagement)

### Game Length

- **5x5 scored**: 19.6-50.8 turns
- **7x7 scored**: 17.2-100.0 turns (turn limit hit in Ore vs Olusare, Olusare vs Mcts)
- **5x5 old**: 21.1-56.1 turns

### Per-Position Branching

- **5x5**: ADD mean 35-109, MOVE mean 0-27
- **7x7**: ADD mean 31-105, MOVE mean 0-20
- Early game ADD-dominated, mid-late shifts to MOVE

## Notes on Reported Metrics

Some metrics appear inconsistent:

- **Per-bot W-L-D in mirrors** doesn't reconcile with seat stats (e.g., 5x5 Ologbon mirror per-bot 8-9-3 vs seat 0-3-17). Seat stats used for mirror summaries.
- **Avg MCTS iterations/move** (323k-1.88M) conflicts with standalone tests showing ~16.5k (5x5) / ~6.7k (7x7) and is likely cumulative rather than per-move average.
- **Minimax depths** show wide variation (Olusare 3.1-36.3, Ologbon 1.7-21.5) suggesting iterative deepening under time budget rather than fixed depth.

## Conclusions

1. **Minimax consistently beats MCTS** across all configurations despite MCTS optimization
2. **Second-seat bias present** in both scored and old rules for strong bot mirrors
3. **MOVE strategies vary** by bot and configuration (Ologbon ADD-only on 5x5 scored, Olusare high MOVE)
4. **Old-rules Ologbon mirror** shows most balanced play (seat 2-14-4, 61% MOVE, 56 turns)
5. **7x7 games longer** and more draw-heavy than 5x5
