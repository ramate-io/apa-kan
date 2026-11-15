CREATE TABLE IF NOT EXISTS accounts (
    account_id BLOB NOT NULL PRIMARY KEY,
    genesis_vk BLOB NOT NULL,
    email TEXT,
    email_verified INTEGER NOT NULL DEFAULT 0,
    recovery_opt_in INTEGER NOT NULL DEFAULT 0,
    approve_via_email INTEGER NOT NULL DEFAULT 0,
    created_at_unix INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS authorized_keys (
    account_id BLOB NOT NULL,
    vk BLOB NOT NULL,
    label TEXT NOT NULL,
    status TEXT NOT NULL,
    next_nonce INTEGER NOT NULL DEFAULT 0,
    added_at_unix INTEGER NOT NULL,
    revoked_at_unix INTEGER,
    usable_at_unix INTEGER,
    PRIMARY KEY (account_id, vk)
);

CREATE TABLE IF NOT EXISTS link_requests (
    request_id BLOB NOT NULL PRIMARY KEY,
    account_id BLOB NOT NULL,
    payload_json TEXT NOT NULL,
    new_sig BLOB NOT NULL,
    status TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL,
    expires_at_unix INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tile_instances (
    instance_id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    account_id BLOB NOT NULL,
    tile_kind_id INTEGER NOT NULL,
    origin TEXT NOT NULL,
    frozen INTEGER NOT NULL DEFAULT 0,
    trade_bind_until_unix INTEGER
);

CREATE TABLE IF NOT EXISTS rating_division_slots (
    account_id BLOB NOT NULL,
    opponent_class TEXT NOT NULL CHECK (opponent_class IN ('human', 'bot')),
    tile_option TEXT,
    time_control TEXT,
    division_id TEXT,
    enrolled_at_unix INTEGER,
    PRIMARY KEY (account_id, opponent_class, tile_option, time_control)
);

CREATE TABLE IF NOT EXISTS ratings (
    account_id BLOB NOT NULL PRIMARY KEY,
    elo_milli INTEGER NOT NULL,
    games INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS recovery_pending (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    account_id BLOB NOT NULL,
    pending_vk BLOB NOT NULL,
    token_hash BLOB NOT NULL,
    kind TEXT NOT NULL,
    started_at_unix INTEGER NOT NULL,
    usable_at_unix INTEGER NOT NULL,
    cancelled_at_unix INTEGER,
    email TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL PRIMARY KEY
);

INSERT INTO schema_version (version) SELECT 1
WHERE NOT EXISTS (SELECT 1 FROM schema_version);