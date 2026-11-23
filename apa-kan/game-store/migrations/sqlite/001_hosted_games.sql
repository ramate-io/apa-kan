-- Hosted games: one row per game (opening record, seat secret hashes, labels)
-- and an append-only log with one row per accepted transaction.
-- `next_seq` is the compare-and-swap cursor: an append must name it exactly.
CREATE TABLE IF NOT EXISTS hosted_games (
    game_id TEXT PRIMARY KEY,
    schema_v INTEGER NOT NULL,
    opening_json TEXT NOT NULL,
    seat1_secret TEXT NOT NULL,
    seat2_secret TEXT NOT NULL,
    seat1_label TEXT NOT NULL,
    seat2_label TEXT NOT NULL,
    next_seq INTEGER NOT NULL DEFAULT 0 CHECK (next_seq >= 0),
    created_at_unix INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS hosted_game_txs (
    game_id TEXT NOT NULL REFERENCES hosted_games(game_id),
    seq INTEGER NOT NULL CHECK (seq >= 0),
    seat INTEGER NOT NULL CHECK (seat IN (1, 2)),
    action_json TEXT NOT NULL,
    -- `seat_token` today. Bearer tokens are never stored; a future signed
    -- variant keeps its public proof in auth_json.
    auth_kind TEXT NOT NULL,
    auth_json TEXT,
    committed_at_unix INTEGER NOT NULL,
    PRIMARY KEY (game_id, seq)
);
