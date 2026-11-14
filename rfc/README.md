# RFCs

This directory contains Request for Comments (RFCs) for Apa Kan game features and technical designs.

## Structure

Each RFC should be placed in its own directory with the naming convention:
```
rfc-NNN-NNN-NNN-descriptive-name/
```

Each RFC directory should contain:
- `README.md` - The main RFC document
- Supporting files, diagrams, and assets as needed

## Creating an RFC

1. Create a new directory following the naming convention
2. Copy the RFC template or create a README.md with:
   - Problem statement
   - Proposed solution
   - Implementation details
   - Alternatives considered
   - Open questions
3. Submit for review via pull request

## Index

- [rfc-001-001-001-rosette-redesign](./rfc-001-001-001-rosette-redesign/README.md) — 8-petal rosette board, color overwhelm, bench/bag, exact-`d` MOVE, stall-prevention RuleSet options
- [rfc-001-001-002-independent-channels](./rfc-001-001-002-independent-channels/README.md) — independent additive normal / zag / braid / bead edge channels; accepted default: channels + flat-1 failed braids + 11-tile public bench with spawn tokens + doubles allowed
- [rfc-001-001-003-deck-building](./rfc-001-001-003-deck-building/README.md) — 27-tile collections, 7+4 public bench, reserves never usable, 6-ring-per-type cap (zag, braid, bead; braids count by slots)
- [rfc-001-001-004-beads](./rfc-001-001-004-beads/README.md) — fourth ring type: beads as a bomb against non-matching colors; unmatched bead rings +1 each, single match −1 per edge
- [rfc-001-001-005-range](./rfc-001-001-005-range/README.md) — per-ring **range** modifier (not a type): effects extend along uninterrupted 2-arc walks through empty intermediates; inner↔center cartesian product; both tiles charged
