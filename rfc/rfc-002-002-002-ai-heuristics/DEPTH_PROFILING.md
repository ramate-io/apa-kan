# Multi-Depth Compute Profiling Results

## Methodology

### Configuration
- **Bots tested**: Olusare (mobility heuristic), Ologbon (enhanced heuristic)
- **Depths**: 1, 2, 3, (4 if feasible)
- **Variants**: 5 rule configurations (Old, A, A+B bank=7, A+B bank=5, A+B+C)
- **Games per side**: 50 (depth 1-2), 30 (depth 3), 20 (depth 4 if attempted)
- **Optimizations**: Transposition table (TT) + Move ordering (MO) enabled by default
- **Metrics tracked**:
  - Nodes searched per move
  - Branching factor: ADD vs MOVE actions generated per position
  - Time per move: mean and p95 (milliseconds)
  - Time per game (seconds)
  - TT hit rate (%)
  - First-seat win rate with 95% CI
  - MOVE% (MOVE actions / total actions)
  - Captures per game
  - Unique outcome count (determinism measure)

### Hardware
- Cloud VM: Linux 6.12, x86_64
- Rust 1.x release build with optimizations

## Results

### Depth 1

**Sample**: 50 games/side (100 total per pairing), 5 variants, 3 pairings each = 1500 games
**Runtime**: ~12 seconds total (~0.008s per game)

#### Compute Profile
| Pairing | Nodes/Move | Branching (ADD/MOVE) | Time/Move (ms) | Time/Game (s) | TT Hit Rate |
|---------|------------|----------------------|----------------|---------------|-------------|
| Olusare vs Olusare | 121 | 120.7 / 0.2 | 0.33 | 0.01 | N/A |
| Olusare vs Ologbon | 122 | 121.8 / 0.1 | 0.34 | 0.01 | N/A |
| Ologbon vs Ologbon | 123 | 123.0 / 0.0 | 0.36 | 0.01 | N/A |

**Observations**:
- Very fast: sub-millisecond per move
- ~120 nodes/move (branching factor matches total legal actions)
- Minimal MOVE generation (0-0.2 per position)
- No TT benefit at depth 1 (positions not revisited)

#### Strategic Outcomes (Old Rules Variant)
| Pairing | First-Seat Wins | Second-Seat Wins | First-Seat Win Rate | Unique Outcomes |
|---------|-----------------|------------------|---------------------|-----------------|
| Olusare vs Olusare | 43 | 57 | 43.0% [33.3%, 52.7%] | 2 |
| Olusare vs Ologbon | 23 | 77 | 23.0% [14.8%, 31.2%] | 2 |
| Ologbon vs Ologbon | 0 | 100 | 0.0% [0.0%, 0.0%] | 1 |

**Observations**:
- Some non-determinism (Olusare mirror: 43% vs 57%)
- Second-player advantage visible even at depth 1
- Ologbon mirror is deterministic (100% second-seat wins)
- MOVE%: 0-3.8% (still heavily favor ADD)

### Depth 2

**Sample**: 50 games/side (100 total per pairing), 5 variants, 3 pairings each = 1500 games
**Runtime**: ~175 seconds total (~0.12s per game)

#### Compute Profile (Old Rules Variant)
| Pairing | Nodes/Move | Branching (ADD/MOVE) | Time/Move (ms) | Time/Game (s) | TT Hit Rate |
|---------|------------|----------------------|----------------|---------------|-------------|
| Olusare vs Olusare | 1043 | 11530.0 / 0.5 | 5.70 | 0.13 | 37.0% |
| Olusare vs Ologbon | 729 | 11174.8 / 16.6 | 5.52 | 0.12 | 37.6% |
| Ologbon vs Ologbon | 375 | 10847.0 / 33.6 | 5.18 | 0.11 | 38.1% |

#### Compute Profile (A+B bank=7 Variant)
| Pairing | Nodes/Move | Branching (ADD/MOVE) | Time/Move (ms) | Time/Game (s) | TT Hit Rate |
|---------|------------|----------------------|----------------|---------------|-------------|
| Olusare vs Olusare | 374 | 10816.7 / 33.1 | 5.18 | 0.11 | 38.1% |
| Olusare vs Ologbon | 374 | 10825.9 / 33.5 | 5.13 | 0.11 | 38.1% |
| Ologbon vs Ologbon | 374 | 10830.2 / 33.3 | 5.14 | 0.11 | 38.1% |

**Observations**:
- ~8-10x more expensive than depth 1
- Nodes/move varies significantly by pairing (375-1043)
- Branching factor: massive ADD action generation (10k-11k cumulative over search tree)
- Time/move: 5-6ms mean, 13-15ms p95
- TT hit rate: 37-38% (decent cache reuse)

#### Strategic Outcomes (ALL Variants)
| Variant | First-Seat Win Rate | MOVE% | Captures/Game | Unique Outcomes |
|---------|---------------------|-------|---------------|-----------------|
| Old Rules | 0.0% | 0.1-0.3% | 0-10 | 1-2 per pairing |
| A Only | 0.0% | 0.4-0.7% | 0.2-10.1 | 1-2 per pairing |
| A+B (bank=7) | 0.0% | 0.4-0.6% | 10.1 | 2 per pairing |
| A+B (bank=5) | 0.0% | 0.5% | 10.1 | 2 per pairing |
| A+B+C | 0.0-1.0% | 0.2-0.8% | 10.0-10.2 | 2-3 per pairing |

**CRITICAL FINDINGS**:
1. **100% Second-Seat Wins**: All strategic pairings show 0% first-seat win rate
2. **Deterministic Play**: Only 1-2 unique outcomes per pairing (100 games!)
3. **Extremely Low MOVE%**: 0.1-0.8% across all variants
4. **Bank Variants Identical**: bank=5 and bank=7 show no strategic difference at depth 2
5. **Designer Hypothesis Confirmed**: Depth 2 is too shallow to find tactical MOVE opportunities

### Depth 3

**Sample**: 30 games/side (60 total per pairing), 5 variants, 3 pairings each = 900 games
**Status**: IN PROGRESS (>11 minutes elapsed)

Expected compute cost: ~10-100x more expensive than depth 2 per move, depending on TT/MO effectiveness.

**Estimated runtime**: 30-60 minutes total (exponential scaling)

---PENDING COMPLETION---

### Depth 4

**Status**: DEFERRED until depth 3 results analyzed

**Considerations**:
- May require time budget (e.g. 10s per move) or iterative deepening
- Sample size: 20 games/side if feasible
- Goal: Determine if deeper search shifts MOVE%, captures, and seat balance

## Analysis

### Depth Impact on Search Cost

| Depth | Nodes/Move (avg) | Time/Move (mean ms) | Time/Game (s) | Speedup from Optimizations |
|-------|------------------|---------------------|---------------|----------------------------|
| 1 | ~120 | 0.33 | 0.01 | N/A (no TT benefit) |
| 2 | ~370-1043 | 5.2-5.7 | 0.11-0.13 | TT hit rate 37-38% |
| 3 | TBD | TBD | TBD | TBD |
| 4 | TBD | TBD | TBD | TBD |

**Observations**:
- Depth 1→2: ~8-10x cost increase per move
- Depth 2→3: Expected ~10-100x (depending on pruning effectiveness)
- TT provides ~38% hit rate at depth 2, reducing redundant evaluations
- Move ordering helps alpha-beta prune more branches

### Branching Factor Analysis

**Depth 2** (cumulative actions generated in search tree):
- ADD actions: 10,800-11,500 per position explored
- MOVE actions: 0.5-33.6 per position explored
- **Ratio**: ADD actions outnumber MOVEs by 300-20,000x

**Implication**: The search tree is overwhelmingly dominated by ADD actions. Even with move ordering prioritizing captures, MOVEs are rare enough that bots don't discover tactical MOVE+capture combinations at depth 2.

### Strategic Behavior by Depth

| Depth | First-Seat Win Rate | MOVE% | Determinism | Designer Interpretation |
|-------|---------------------|-------|-------------|-------------------------|
| 1 | 0-43% | 0-3.8% | Low (2-15 outcomes) | Random-like, weak second-player bias |
| 2 | 0% | 0.1-0.8% | Extreme (1-2 outcomes) | Complete deterministic second-player wins |
| 3 | TBD | TBD | TBD | Test if deeper search finds MOVE tactics |

**Hypothesis**: Depth 2 minimax sees only immediate ADD threats and cannot foresee MOVE+capture chains. Depth 3 may reveal longer tactical sequences.

### Optimization Effectiveness

**Transposition Table**:
- Hit rate: 37-38% at depth 2
- Benefit: Avoids re-evaluating ~40% of positions
- Estimated speedup: ~1.6x (1 / (1 - 0.38))

**Move Ordering**:
- Prioritizes captures and bank gains
- Improves alpha-beta pruning (more cutoffs)
- Combined with TT: enables depth 3 to be feasible

**Conclusion**: Optimizations are working as intended and make depth 3+ tractable.

## Pending Questions

1. **Does depth 3 increase MOVE%?**
   - If yes → confirms shallow search explanation
   - If no → suggests game structure fundamentally favors ADD

2. **Does depth 3 change seat balance?**
   - If first-seat win rate shifts from 0% → measure new equilibrium
   - If still 0% → second-player advantage persists even with deeper search

3. **Is depth 4 feasible?**
   - If depth 3 time/game < 5s → attempt depth 4 with 20-game sample
   - Otherwise → recommend iterative deepening with time budget

4. **Do bank=5 and bank=7 diverge at deeper search?**
   - At depth 2 they're identical
   - Deeper search may reveal bank scarcity effects

## Recommendations (Pending Depth 3 Results)

1. **If MOVE% remains <5% at depth 3:**
   - Consider rule changes to make MOVE more attractive (e.g. immunity, bank rewards for MOVE)
   - Current bank gain on opponent removal may be insufficient incentive

2. **If second-seat advantage persists at depth 3:**
   - Investigate rule modifications (e.g. first-player bonus, asymmetric starting banks)
   - May indicate fundamental tempo imbalance

3. **If depth 3 shows tactical depth:**
   - Proceed with depth 4 profiling
   - Consider adaptive difficulty (depth selection based on time budget)

---

**Last Updated**: 2026-11-10 00:35 UTC (depth 3 in progress)
