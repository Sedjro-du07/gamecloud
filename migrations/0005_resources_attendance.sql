-- 0005_resources_attendance.sql
-- Community resources, event attendance, and QR tokens.

-- ---------------------------------------------------------------------------
-- resources
-- ---------------------------------------------------------------------------

CREATE TABLE resources (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title             TEXT NOT NULL,
    url               TEXT NOT NULL,
    resource_type     TEXT,
    tracks            TEXT[] NOT NULL DEFAULT '{}',
    specializations   TEXT[] NOT NULL DEFAULT '{}',
    level             TEXT,
    submitted_by      UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    validated_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    votes             INTEGER NOT NULL DEFAULT 0,
    is_official       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT resources_type_valid CHECK (
        resource_type IS NULL
        OR resource_type IN ('Tutorial', 'Tool', 'Asset', 'Doc', 'Video')
    ),
    CONSTRAINT resources_level_valid CHECK (
        level IS NULL
        OR level IN ('Initiate', 'Junior', 'Senior', 'Expert')
    ),
    -- URL must be HTTP(S).
    CONSTRAINT resources_url_format CHECK (url ~ '^https?://')
);

CREATE INDEX resources_validated_idx ON resources (validated_by) WHERE validated_by IS NOT NULL;
CREATE INDEX resources_tracks_gin    ON resources USING GIN (tracks);
CREATE INDEX resources_votes_idx     ON resources (votes DESC);

-- ---------------------------------------------------------------------------
-- attendance
-- ---------------------------------------------------------------------------

CREATE TABLE attendance (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_name   TEXT NOT NULL,
    event_type   TEXT NOT NULL,
    xp_rewarded  INTEGER NOT NULL,
    scanned_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT attendance_type_valid CHECK (
        event_type IN ('Session', 'OfficeHours', 'StandUp', 'GameJam', 'Special')
    ),
    CONSTRAINT attendance_xp_nonneg CHECK (xp_rewarded >= 0)
);

CREATE INDEX attendance_user_idx     ON attendance (user_id, scanned_at DESC);
CREATE INDEX attendance_event_idx    ON attendance (event_name);
CREATE INDEX attendance_scanned_idx  ON attendance (scanned_at DESC);

-- ---------------------------------------------------------------------------
-- qr_tokens
-- ---------------------------------------------------------------------------
-- The `token` column stores the JWT itself; the `id` column is the
-- internal PK. We index by token for fast lookup on scan.

CREATE TABLE qr_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token        TEXT NOT NULL UNIQUE,
    event_name   TEXT NOT NULL,
    event_type   TEXT NOT NULL,
    xp_value     INTEGER NOT NULL,
    created_by   UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    expires_at   TIMESTAMPTZ NOT NULL,
    is_used      BOOLEAN NOT NULL DEFAULT FALSE,

    CONSTRAINT qr_tokens_type_valid CHECK (
        event_type IN ('Session', 'OfficeHours', 'StandUp', 'GameJam', 'Special')
    ),
    CONSTRAINT qr_tokens_xp_nonneg CHECK (xp_value >= 0)
);

CREATE INDEX qr_tokens_active_idx ON qr_tokens (expires_at) WHERE is_used = FALSE;
