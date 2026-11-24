-- Append-only transaction log, one hash-linked chain per stream
-- (`game/<id>` today; `account/<id>`, `admin/...` later).
-- Rows are signed `apa-kan-tx` envelopes; state is the fold of a stream.
CREATE TABLE IF NOT EXISTS ledger_streams (
  stream_id TEXT PRIMARY KEY NOT NULL,
  next_seq BIGINT NOT NULL CHECK (next_seq >= 1),
  head_hash TEXT NOT NULL,
  created_at_unix BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS ledger_entries (
  stream_id TEXT NOT NULL REFERENCES ledger_streams (stream_id),
  seq BIGINT NOT NULL CHECK (seq >= 0),
  tx_hash TEXT NOT NULL UNIQUE,
  prev_hash TEXT NOT NULL,
  signer TEXT NOT NULL,
  kind TEXT NOT NULL,
  tx_json TEXT NOT NULL,
  committed_at_unix BIGINT NOT NULL,
  PRIMARY KEY (stream_id, seq)
);

CREATE INDEX IF NOT EXISTS ledger_entries_signer ON ledger_entries (signer);
