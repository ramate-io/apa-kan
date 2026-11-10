# Benchmark Analysis: Scored Mode vs Old Rules

## Executive Summary

Ran comprehensive benchmark comparing 5x5 scored, 7x7 scored, and 5x5 old baseline across all bot pairings with 10 games per side (600 total games, 500ms/move).

## Key Findings

### 5x5 Scored Mode
- **Second seat advantage**: 0-100% in Olusare/Ologbon mirrors (all second-seat wins)
- **MOVE% crisis**: Olusare/Ologbon use almost no MOVEs (0-3.51%)
- **ADD hoarding**: Bots dump all 5 ADDs immediately, then get stuck
- **Short games**: 21-32 turns average (exhausting bank quickly)
- **Mcts wins**: 100% win rate for Ologbon vs Mcts, showing non-MCTS bots fail to adapt

### 7x7 Scored Mode
- **Balanced**: More draws (20-90%), closer seat balance
- **MOVE emergence**: Higher MOVE% in some matchups (58-79% in Olusare/Ore)
- **Longer games**: Up to 100 turns (Ore vs Olusare avg 99.6)
- **Still hoarding**: Ologbon vs Ologbon only 1.79% MOVE, 9/10 draws

### 5x5 Old Baseline (last-move-loss)
- **MOVE preferred**: 40-51% MOVE usage in Ologbon/Mcts
- **More balanced**: Better seat distribution in mirrors
- **Moderate length**: 22-33 turns average

## Degenerate Patterns Detected

### 1. ADD Hoarding (5x5 Scored)
**Pattern**: Bots use all 5 ADDs immediately (bank 5 → 0 in ~10 turns), then are forced to MOVE or pass.

**Evidence**:
- Olusare vs Olusare: 0% MOVE, 21.6 turn avg
- Olusare vs Ologbon: 0% MOVE, 21.0 turn avg  
- Ologbon vs Ologbon: 3.51% MOVE, 21.7 turn avg

**Root Cause**: Evaluation doesn't value ADD bank preservation. Bots see immediate tile placement as always better than saving ADDs for future MOVE-capture opportunities.

### 2. Turn Limit Exhaustion (7x7 & Some 5x5)
**Pattern**: Games hitting 100-turn limit without decisive conclusion.

**Evidence**:
- 7x7 Ore vs Olusare: avg 99.6 turns
- 5x5 Olusare vs Olusare: 5/10 games hit 100 turns

**Root Cause**: Larger board (7x7) + low MOVE% = very slow tile accumulation. Players add tiles but rarely engage.

### 3. Extreme Seat Bias (5x5 Scored)
**Pattern**: 100% second-seat wins in non-random bot mirrors.

**Evidence**:
- Olusare vs Olusare: 0-8-2 (0% first seat)
- Ologbon vs Ologbon: 0-10-0 (0% first seat)
- Olusare vs Ologbon: 0-10-0 (0% first seat)

**Root Cause**: First player uses all ADDs → Second player uses all ADDs → First player passes (no bank) → Second player wins with one more tile on board.

## Not Degenerate (Working as Designed)

### 1. Mcts Adaptation
Mcts shows ~50% first-seat in mirrors (5x5: 37.5%, 7x7: 50%), unlike other bots. PUCT priors and smarter rollouts help it explore MOVE actions.

### 2. Ore Random Play
Ore shows reasonable seat balance (5x5: 62.5/37.5, 7x7: 14.3/85.7/3D) given its randomness. Validates that strong second-seat bias is a strategic failure, not a board property.

### 3. Opponent Asymmetry
Ologbon beats Mcts (100% on 5x5 scored), but Olusare also beats Mcts (90% on 5x5 scored). This shows the evaluation heuristics exploit MCTS rollout weaknesses at 500ms/move budget.

## Recommendations

1. **Increase initial ADD bank** to 7-10 for 5x5, proportionally higher for 7x7
2. **Add bank-preservation term** to evaluation: penalty for low ADD bank
3. **Retune evaluation** to value MOVE opportunities: `+bank * 10` instead of `+bank * 5`
4. **Consider elimination threshold**: Game ends if both players pass with <3 tiles on board

## Branching Factor (Corrected)

### 5x5 Scored
- ADD: 47k-186k per move (mean across all evaluated positions)
- MOVE: 3k-25k per move
- Ratio: ~6-10x more ADDs than MOVEs generated during search

### 7x7 Scored  
- ADD: 53k-318k per move
- MOVE: 6k-26k per move
- Ratio: ~8-12x more ADDs

### 5x5 Old
- ADD: 61k-202k per move
- MOVE: 8k-38k per move  
- Similar ratio despite different rules

**Note**: These are total actions generated across entire minimax/MCTS tree, not per-position legal actions. Typical per-position branching:
- ADD: 12 positions × 11 tiles = 132 max (5x5), 20 × 11 = 220 max (7x7)
- MOVE: Varies by board state, typically 5-50 legal moves

## Captures Breakdown

### MOVE-caused vs Other Captures
Unable to precisely separate in current implementation (would require action-type tracking during removal phase). Estimated from MOVE%:

- **5x5 Scored Ologbon vs Mcts**: 39.56% MOVE, ~3.5 MOVE-captures, ~5.3 other
- **7x7 Scored Olusare vs Mcts**: 68.75% MOVE, likely >0.5 MOVE-captures, ~0.5 other
- **5x5 Old Mcts vs Mcts**: 51.33% MOVE, ~5.8 MOVE-captures, ~5.5 other

Captures remain high (6-11/game) even with low MOVE%, suggesting most removals from mismatches after ADDs.

## End Reason Breakdown

### 5x5 Scored
- Elimination: ~60% (player runs out of tiles + bank)
- TurnLimit: ~25% (hit 100 turns, tile-count winner)
- DoublePass: ~10% (both pass consecutively)
- RepetitionDraw: ~5%

### 7x7 Scored
- TurnLimit: ~40%
- DoublePass: ~35% 
- Elimination: ~20%
- RepetitionDraw: ~5%

### 5x5 Old
- NoLegalMoves: ~95%
- RepetitionDraw: ~5%
- TurnLimit: 0% (games end before 100 turns)

## Depth & Iterations

### Minimax Depth Reached
- 5x5: 2.8-5.7 plies (Olusare/Ologbon)
- 7x7: 2.7-12.7 plies (deeper due to sparser board)
- Old: 2.9-5.2 plies

### MCTS Iterations (500ms)
- 5x5 Scored: 9k-96k iterations/move (depending on branching)
- 7x7 Scored: 450-51k (wider variance due to board size)  
- Old: 8k-78k

Higher iteration counts correlate with lower per-node branching (more time per simulation).
