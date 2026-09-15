-- 0019_shares.sql
-- Member shares: scripts, lore, games and assets anyone can post and
-- every member can download.
--
-- A share is either a file kept by the platform or a link to somewhere
-- else (itch.io, Drive, GitHub…), never both. Both kinds go through the
-- platform on download, so the counter means the same thing for each.

CREATE TABLE shares (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title            TEXT NOT NULL,
    description      TEXT,
    kind             TEXT NOT NULL,
    -- File shares: original name, size, checksum, and the name the bytes
    -- are stored under in SHARES_DIR.
    filename         TEXT,
    size_bytes       BIGINT,
    checksum_sha256  TEXT,
    storage_name     TEXT,
    -- Link shares.
    url              TEXT,
    uploaded_by      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    download_count   BIGINT NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT shares_title_not_blank CHECK (length(btrim(title)) > 0),
    CONSTRAINT shares_kind_valid CHECK (
        kind IN ('Script', 'Lore', 'Game', 'Asset', 'Other')
    ),
    CONSTRAINT shares_file_or_link CHECK (
        (url IS NULL AND filename IS NOT NULL AND size_bytes IS NOT NULL
             AND checksum_sha256 IS NOT NULL AND storage_name IS NOT NULL)
        OR
        (url IS NOT NULL AND filename IS NULL AND size_bytes IS NULL
             AND checksum_sha256 IS NULL AND storage_name IS NULL)
    ),
    CONSTRAINT shares_url_format CHECK (url IS NULL OR url ~ '^https?://'),
    CONSTRAINT shares_size_max CHECK (size_bytes IS NULL OR size_bytes <= 524288000),
    CONSTRAINT shares_downloads_nonneg CHECK (download_count >= 0)
);

CREATE INDEX shares_created_idx ON shares (created_at DESC);
CREATE INDEX shares_kind_idx    ON shares (kind, created_at DESC);
