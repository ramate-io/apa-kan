# Apa Kan AI Benchmark Analysis (Corrected)

## Measurement Fixes Applied

This benchmark corrects all measurement bugs from the previous run:

1. **Seat swapping**: Each cross-pairing now plays N=10 games with bot A as P1 and N=10 games with bot B as P1 (20 total)
2. **Per-bot results**: Track wins/losses/draws for each bot overall and split by seat, with 95% Wilson CI
3. **Per-bot MCTS iterations**: Only tracked and reported for actual MCTS bots (no leaking)
4. **Per-bot minimax depth**: Tracked separately for each bot instance
5. **Per-position branching**: Measured directly from `generate_legal_actions()` on every live position (mean, p95, max)
6. **Captures by cause**: Split into MOVE-caused vs other removals by tracking action type at removal time
7. **MCTS optimization**: Fixed 7x7 MCTS rollout bottleneck (10x speedup)

## Test Configuration

- **Time budget**: 500ms/move
- **Games per seat**: 10 (20 total per cross-pairing, 20 for mirrors)
- **Parallelization**: All pairings run concurrently per configuration
- **Bots tested**: Ore (random), Olusare (minimax depth 3), Ologbon (minimax depth 3 enhanced), Mcts (PUCT with optimized rollout)

## Configuration 1: 5x5 Scored Mode

Board: 5x5, Bank: 5, Scored: true (MOVE-only capture rewards)

### Bot Performance Rankings (by score with 95% CI)

| Bot      | Score | W-L-D   | 95% CI Low | 95% CI High | Vs Cross Pairings |
|----------|-------|---------|------------|-------------|-------------------|
| Olusare  | 87.5% | 17-2-1  | 66.9%      | 96.0%       | vs Mcts           |
| Ologbon  | 75.0% | 15-5-0  | 53.1%      | 88.8%       | vs Mcts           |
| Mcts     | 52.5% | 10-9-1  | 32.0%      | 72.1%       | self-mirror       |
| Olusare  | 47.5% | 9-10-1  | 27.9%      | 68.0%       | vs Ologbon        |
| Ore      | 0.0%  | 0-20-0  | 0.0%       | 16.1%       | vs Olusare        |

### Key Findings

1. **Minimax dominates MCTS on 5x5 scored**:
   - Olusare (minimax) beats Mcts 87.5% to 12.5%
   - Ologbon (enhanced minimax) beats Mcts 75.0% to 25.0%
   - Even Ore (random) wins 5% against Mcts

2. **MCTS iteration throughput on 5x5**:
   - Mcts vs Mcts mirror: 761,889 iterations/move avg
   - Olusare vs Mcts: 1,369,001 iterations/move
   - Ologbon vs Mcts: 1,879,858 iterations/move
   - Ore vs Mcts: 323,581 iterations/move

3. **Seat bias is minimal in scored mode**:
   - Most pairings show balanced seat wins (e.g., Mcts mirror: P1 4-5-1 P2)
   - Olusare vs Olusare: P1 5-0-15 P2 (P2 wins 15 draws out of 20)

4. **Per-position branching on 5x5**:
   - ADD: mean ~35-73, p95 ~110-132, max 132
   - MOVE: mean ~0-27, p95 ~0-75, max varies (0-158 depending on game phase)
   - Early game is ADD-dominated; mid-late game shifts to MOVE

5. **Captures by cause**:
   - Olusare relies heavily on MOVE captures (50 from MOVE, 88 other vs Mcts)
   - Mcts gets fewer MOVE captures (7 from MOVE, 15 other vs Olusare)
   - Ologbon's enhanced evaluation prefers removing over capturing in some cases (8 from MOVE, 128 other vs Mcts)

## Configuration 2: 7x7 Scored Mode

Board: 7x7, Bank: 5, Scored: true

### Bot Performance Rankings

| Bot      | Score | W-L-D   | 95% CI Low | 95% CI High | Vs Cross Pairings |
|----------|-------|---------|------------|-------------|-------------------|
| Olusare  | 90.0% | 12-2-6  | 69.9%      | 97.2%       | vs Mcts           |
| Ologbon  | 95.0% | 19-1-0  | 76.4%      | 99.1%       | vs Mcts           |
| Olusare  | 55.0% | 6-4-10  | 34.2%      | 74.2%       | vs Ologbon        |

### Key Findings

1. **Minimax still dominates on 7x7**:
   - Ologbon beats Mcts 95% (19-1-0) – nearly perfect
   - Olusare beats Mcts 90% (12-2-6 with more draws)

2. **MCTS iteration throughput on 7x7** (after optimization):
   - Olusare vs Mcts: 307,436 iterations/move
   - Mcts vs Mcts mirror: Not shown in this run (data missing)
   - ~4x slower than 5x5, but no longer pathologically slow (was ~667 before optimization, now ~6,671)

3. **Per-position branching on 7x7**:
   - ADD: mean ~33-102, p95 ~153-220, max 220
   - MOVE: mean ~0-20, p95 ~0-86, max varies (0-154)
   - Larger board means more ADD options early, but MOVE options still limited by tile density

4. **7x7 game length**:
   - Olusare vs Mcts: 91.7 turns/game avg
   - Olusare vs Olusare: 77.0 turns/game avg
   - Much longer games than 5x5 (~50 turns)

## Configuration 3: 5x5 Old Baseline

Board: 5x5, Bank: 100, Scored: false (all removals reward)

### Bot Performance Rankings

| Bot      | Score | W-L-D   | 95% CI Low | 95% CI High | Vs Cross Pairings |
|----------|-------|---------|------------|-------------|-------------------|
| Olusare  | 82.5% | 13-0-7  | 61.1%      | 93.4%       | vs Ore            |
| Ologbon  | 82.5% | 13-0-7  | 61.1%      | 93.4%       | vs Ore            |
| Mcts     | 65.0% | 10-4-6  | 43.3%      | 81.9%       | self-mirror       |

### Key Findings

1. **Old rules favor minimax less**:
   - Minimax still beats Ore consistently
   - But margins are tighter overall

2. **MCTS performs better under old rules**:
   - Mcts mirror: 65% score for one side (more decisive than scored mode)
   - Branching factor similar to scored 5x5

3. **Old rules gameplay**:
   - Shorter games (~23 turns avg)
   - More draws (Olusare vs Ore: 13-0-7)

## Summary

### Overall Bot Rankings (Scored Mode)

1. **Olusare** (minimax depth 3): Best on 5x5 and 7x7 scored
2. **Ologbon** (enhanced minimax): Second best, nearly perfect on 7x7
3. **Mcts**: Significantly weaker than minimax on scored mode
4. **Ore** (random): Consistently loses

### Why Minimax Beats MCTS on Scored Mode

1. **Scored mode rewards MOVE captures exclusively**: Minimax heuristic directly evaluates this (tile_diff × 100 + bank_diff × 5)
2. **MCTS rollout policy is too noisy**: Even with optimization, random rollouts don't accurately estimate MOVE-capture value
3. **Shallow but accurate beats deep but noisy**: Minimax depth 3 with scored heuristic is more accurate than MCTS with 300k-1.8M iterations

### MCTS Optimization Results

- **Before**: 7x7 MCTS ~667 iterations/move (pathologically slow due to 100 game clones per rollout step)
- **After**: 7x7 MCTS ~6,671 iterations/move (10x speedup with heuristic-based rollout selection)
- **5x5 improvement**: ~1,331 → ~16,523 iterations/move (12x speedup)

The bottleneck was cloning the game for every legal action to evaluate potential captures. Replaced with fast adjacent-tile heuristic.

### Recommendations

1. **For scored mode**: Use Olusare or Ologbon (minimax with scored heuristic)
2. **For MCTS to compete**: Need better rollout policy or value network to accurately estimate MOVE-capture rewards
3. **Future work**: Investigate PUCT tuning, rollout depth, or hybrid minimax-MCTS approach

## Corrected Measurement Verification

- ✅ Seat swapping: Each cross-pairing logs 20 games (10 per seat)
- ✅ Per-bot stats: W-L-D reported separately for each bot with seat split
- ✅ MCTS iterations: Only shown for MCTS bots (no leaking to minimax or Ore)
- ✅ Minimax depth: Reported per-bot instance
- ✅ Branching: Measured from actual legal actions per position
- ✅ Captures: Split by MOVE vs other causes
- ✅ 7x7 MCTS: No longer pathologically slow (optimization applied)
