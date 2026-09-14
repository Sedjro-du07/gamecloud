-- 0004_projects.sql
-- Projects, contributors, files, and per-track validations.

-- ---------------------------------------------------------------------------
-- projects
-- ---------------------------------------------------------------------------

CREATE TABLE projects (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                TEXT NOT NULL,
    short_description   TEXT,
    long_description    TEXT,
    primary_track       TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'Draft',
    block_number        INTEGER,
    epitech_level       TEXT,
    rarity              TEXT NOT NULL DEFAULT 'Common',
    thumbnail_url       TEXT,
    banner_url          TEXT,
    -- Postgres TEXT[] preserves insertion order. Capped to 10 by handler.
    screenshots         TEXT[] NOT NULL DEFAULT '{}',
    video_url           TEXT,
    gifs                TEXT[] NOT NULL DEFAULT '{}',
    github_repo_url     TEXT,
    itch_url            TEXT,
    created_by          UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_at         TIMESTAMPTZ,

    CONSTRAINT projects_primary_track_valid CHECK (
        primary_track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    ),
    CONSTRAINT projects_status_valid CHECK (
        status IN ('Draft', 'InReview', 'PartialOK', 'Approved', 'Released', 'Archived', 'Rejected')
    ),
    CONSTRAINT projects_rarity_valid CHECK (
        rarity IN ('Common', 'Rare', 'Epic', 'Legendary', 'Mythic')
    ),
    CONSTRAINT projects_block_range CHECK (
        block_number IS NULL OR (block_number BETWEEN 1 AND 8)
    ),
    CONSTRAINT projects_epitech_level_valid CHECK (
        epitech_level IS NULL OR epitech_level IN ('Tek1', 'Tek2', 'Tek3', 'Master')
    ),
    -- A project can only be `Released` (or later) if it has a release date.
    CONSTRAINT projects_released_consistency CHECK (
        (status NOT IN ('Released', 'Archived')) OR (released_at IS NOT NULL)
    ),
    CONSTRAINT projects_screenshots_max CHECK (cardinality(screenshots) <= 10)
);

CREATE INDEX projects_status_idx        ON projects (status);
CREATE INDEX projects_primary_track_idx ON projects (primary_track);
CREATE INDEX projects_block_idx         ON projects (block_number) WHERE block_number IS NOT NULL;
CREATE INDEX projects_released_at_idx   ON projects (released_at DESC NULLS LAST)
    WHERE status = 'Released';
CREATE INDEX projects_creator_idx       ON projects (created_by);

-- ---------------------------------------------------------------------------
-- project_contributors
-- ---------------------------------------------------------------------------

CREATE TABLE project_contributors (
    project_id        UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id           UUID NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
    track             TEXT NOT NULL,
    role_in_project   TEXT,
    PRIMARY KEY (project_id, user_id, track),

    CONSTRAINT project_contributors_track_valid CHECK (
        track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    )
);

CREATE INDEX project_contributors_user_idx    ON project_contributors (user_id);
CREATE INDEX project_contributors_track_idx   ON project_contributors (track);

-- ---------------------------------------------------------------------------
-- project_files
-- ---------------------------------------------------------------------------

CREATE TABLE project_files (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    filename        TEXT NOT NULL,
    file_type       TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    checksum_sha256 TEXT NOT NULL,
    storage_path    TEXT NOT NULL,
    version         TEXT NOT NULL DEFAULT 'v1.0',
    changelog       TEXT,
    uploaded_by     UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    uploaded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT project_files_type_valid CHECK (
        file_type IN ('Executable', 'Source', 'Asset', 'Doc', 'Audio', 'Video')
    ),
    CONSTRAINT project_files_size_positive CHECK (size_bytes > 0),
    -- Hard cap at 500 MB; the handler also enforces this before upload.
    CONSTRAINT project_files_size_max CHECK (size_bytes <= 524288000),
    CONSTRAINT project_files_checksum_format CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$')
);

CREATE INDEX project_files_project_idx ON project_files (project_id);

-- ---------------------------------------------------------------------------
-- track_validations
-- ---------------------------------------------------------------------------
-- One row per (project, track). Created when a project transitions to
-- `InReview`, one per concerned track.

CREATE TABLE track_validations (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    track        TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'Pending',
    reviewed_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    feedback     TEXT,
    reviewed_at  TIMESTAMPTZ,

    CONSTRAINT track_validations_track_valid CHECK (
        track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    ),
    CONSTRAINT track_validations_status_valid CHECK (
        status IN ('Pending', 'Approved', 'Rejected', 'NotApplicable')
    ),
    -- Rejection requires non-empty feedback.
    CONSTRAINT track_validations_reject_requires_feedback CHECK (
        status <> 'Rejected' OR (feedback IS NOT NULL AND length(feedback) > 0)
    ),
    -- Non-pending statuses require a reviewer + timestamp.
    CONSTRAINT track_validations_review_consistency CHECK (
        status = 'Pending'
        OR (reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX track_validations_project_track_idx
    ON track_validations (project_id, track);
CREATE INDEX track_validations_pending_idx
    ON track_validations (track) WHERE status = 'Pending';
