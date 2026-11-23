CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS accounts (
    account_id BYTEA NOT NULL PRIMARY KEY,
    genesis_vk BYTEA NOT NULL,
    email TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    recovery_opt_in BOOLEAN NOT NULL DEFAULT FALSE,
    approve_via_email BOOLEAN NOT NULL DEFAULT FALSE,
    created_at_unix BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS authorized_keys (
    account_id BYTEA NOT NULL,
    vk BYTEA NOT NULL,
    label TEXT NOT NULL,
    status TEXT NOT NULL,
    next_nonce BIGINT NOT NULL DEFAULT 0,
    added_at_unix BIGINT NOT NULL,
    revoked_at_unix BIGINT,
    usable_at_unix BIGINT,
    PRIMARY KEY (account_id, vk)
);

CREATE TABLE IF NOT EXISTS link_requests (
    request_id BYTEA NOT NULL PRIMARY KEY,
    account_id BYTEA NOT NULL,
    payload_json TEXT NOT NULL,
    new_sig BYTEA NOT NULL,
    status TEXT NOT NULL,
    created_at_unix BIGINT NOT NULL,
    expires_at_unix BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS tile_instances (
    instance_id BIGSERIAL PRIMARY KEY,
    account_id BYTEA NOT NULL,
    tile_kind_id BIGINT NOT NULL,
    origin TEXT NOT NULL,
    frozen BOOLEAN NOT NULL DEFAULT FALSE,
    trade_bind_until_unix BIGINT,
    generation_id BIGINT NOT NULL DEFAULT 0,
    core_tile_id TEXT NOT NULL DEFAULT '',
    tile_type_id TEXT NOT NULL DEFAULT '',
    character_id TEXT NOT NULL DEFAULT '',
    pass_version INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tiles_character ON tile_instances (character_id);
CREATE INDEX IF NOT EXISTS idx_tiles_core ON tile_instances (core_tile_id);
CREATE INDEX IF NOT EXISTS idx_tiles_type ON tile_instances (tile_type_id);
CREATE INDEX IF NOT EXISTS idx_tiles_account ON tile_instances (account_id);

CREATE TABLE IF NOT EXISTS rating_division_slots (
    account_id BYTEA NOT NULL,
    opponent_class TEXT NOT NULL CHECK (opponent_class IN ('human', 'bot')),
    tile_option TEXT,
    time_control TEXT,
    division_id TEXT,
    enrolled_at_unix BIGINT,
    PRIMARY KEY (account_id, opponent_class, tile_option, time_control)
);

CREATE TABLE IF NOT EXISTS ratings (
    account_id BYTEA NOT NULL PRIMARY KEY,
    elo_milli INTEGER NOT NULL,
    games INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS recovery_pending (
    id BIGSERIAL PRIMARY KEY,
    account_id BYTEA NOT NULL,
    pending_vk BYTEA NOT NULL,
    token_hash BYTEA NOT NULL,
    kind TEXT NOT NULL,
    started_at_unix BIGINT NOT NULL,
    usable_at_unix BIGINT NOT NULL,
    cancelled_at_unix BIGINT,
    email TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pass_pointer (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    draft_version INTEGER NOT NULL,
    published_version INTEGER
);

INSERT INTO pass_pointer (id, draft_version, published_version)
VALUES (1, 1, NULL)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pass_config_revs (
    id BIGSERIAL PRIMARY KEY,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    tau DOUBLE PRECISION NOT NULL,
    k INTEGER
);

CREATE TABLE IF NOT EXISTS embedding_revs (
    id BIGSERIAL PRIMARY KEY,
    character_id TEXT NOT NULL,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    sparse_json TEXT NOT NULL,
    vector vector(20) NOT NULL
);

CREATE INDEX IF NOT EXISTS embedding_revs_hnsw
    ON embedding_revs USING hnsw (vector vector_cosine_ops);

CREATE TABLE IF NOT EXISTS fallback_revs (
    id BIGSERIAL PRIMARY KEY,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    ids_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS specific_revs (
    id BIGSERIAL PRIMARY KEY,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    maps_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS character_catalog (
    character_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    image_png BYTEA,
    created_at_unix BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS account_roles (
    account_id BYTEA NOT NULL,
    role TEXT NOT NULL,
    granted_at_unix BIGINT NOT NULL,
    PRIMARY KEY (account_id, role)
);

CREATE TABLE IF NOT EXISTS core_tiles (
    core_tile_id TEXT PRIMARY KEY,
    rings_json TEXT NOT NULL,
    dots INTEGER NOT NULL,
    capture TEXT NOT NULL,
    emblem TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tile_types (
    tile_type_id TEXT PRIMARY KEY,
    rings_json TEXT NOT NULL,
    dots INTEGER NOT NULL,
    capture TEXT NOT NULL,
    emblem TEXT NOT NULL
);
