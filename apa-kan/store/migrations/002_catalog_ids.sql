CREATE TABLE IF NOT EXISTS pass_pointer (
    id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
    draft_version INTEGER NOT NULL,
    published_version INTEGER
);

INSERT INTO pass_pointer (id, draft_version, published_version)
SELECT 1, 1, NULL
WHERE NOT EXISTS (SELECT 1 FROM pass_pointer WHERE id = 1);

CREATE TABLE IF NOT EXISTS pass_config_revs (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    tau REAL NOT NULL,
    k INTEGER
);

CREATE TABLE IF NOT EXISTS embedding_revs (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    character_id TEXT NOT NULL,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    sparse_json TEXT NOT NULL,
    vector_blob BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embedding_revs_live
    ON embedding_revs (character_id, valid_from);

CREATE TABLE IF NOT EXISTS fallback_revs (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    ids_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS specific_revs (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER,
    maps_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS character_catalog (
    character_id TEXT NOT NULL PRIMARY KEY,
    display_name TEXT NOT NULL,
    image_png BLOB,
    created_at_unix INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS account_roles (
    account_id BLOB NOT NULL,
    role TEXT NOT NULL,
    granted_at_unix INTEGER NOT NULL,
    PRIMARY KEY (account_id, role)
);

CREATE TABLE IF NOT EXISTS core_tiles (
    core_tile_id TEXT NOT NULL PRIMARY KEY,
    rings_json TEXT NOT NULL,
    dots INTEGER NOT NULL,
    capture TEXT NOT NULL,
    emblem TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tile_types (
    tile_type_id TEXT NOT NULL PRIMARY KEY,
    rings_json TEXT NOT NULL,
    dots INTEGER NOT NULL,
    capture TEXT NOT NULL,
    emblem TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tiles_character ON tile_instances (character_id);
CREATE INDEX IF NOT EXISTS idx_tiles_core ON tile_instances (core_tile_id);
CREATE INDEX IF NOT EXISTS idx_tiles_type ON tile_instances (tile_type_id);
