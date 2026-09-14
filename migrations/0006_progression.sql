-- 0006_progression.sql
-- Roadmap, quests, XP logs, audit logs, Hall of Fame.

-- ---------------------------------------------------------------------------
-- roadmap
-- ---------------------------------------------------------------------------

CREATE TABLE roadmap (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    milestone_name  TEXT NOT NULL,
    description     TEXT,
    target_date     DATE,
    status          TEXT NOT NULL DEFAULT 'Todo',
    block_number    INTEGER,
    is_major_boss   BOOLEAN NOT NULL DEFAULT FALSE,
    track           TEXT,
    completed_at    TIMESTAMPTZ,

    CONSTRAINT roadmap_status_valid CHECK (
        status IN ('Todo', 'InProgress', 'Completed')
    ),
    CONSTRAINT roadmap_block_range CHECK (
        block_number IS NULL OR (block_number BETWEEN 1 AND 8)
    ),
    CONSTRAINT roadmap_track_valid CHECK (
        track IS NULL
        OR track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    ),
    CONSTRAINT roadmap_completed_consistency CHECK (
        (status = 'Completed' AND completed_at IS NOT NULL)
        OR (status <> 'Completed' AND completed_at IS NULL)
    )
);

CREATE INDEX roadmap_status_idx ON roadmap (status);
CREATE INDEX roadmap_block_idx  ON roadmap (block_number) WHERE block_number IS NOT NULL;

-- ---------------------------------------------------------------------------
-- quests
-- ---------------------------------------------------------------------------

CREATE TABLE quests (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title            TEXT NOT NULL,
    description      TEXT,
    xp_reward        INTEGER NOT NULL,
    quest_type       TEXT NOT NULL,
    condition_type   TEXT NOT NULL,
    condition_value  INTEGER NOT NULL,
    track            TEXT,
    starts_at        TIMESTAMPTZ NOT NULL,
    ends_at          TIMESTAMPTZ NOT NULL,
    created_by       UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,

    CONSTRAINT quests_type_valid CHECK (
        quest_type IN ('Weekly', 'Special', 'Hidden')
    ),
    CONSTRAINT quests_condition_valid CHECK (
        condition_type IN ('Push', 'Attend', 'Submit', 'Review')
    ),
    CONSTRAINT quests_track_valid CHECK (
        track IS NULL
        OR track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    ),
    CONSTRAINT quests_xp_positive CHECK (xp_reward > 0),
    CONSTRAINT quests_condition_positive CHECK (condition_value > 0),
    CONSTRAINT quests_window_valid CHECK (ends_at > starts_at)
);

CREATE INDEX quests_active_idx ON quests (starts_at, ends_at);

-- ---------------------------------------------------------------------------
-- quest_completions
-- ---------------------------------------------------------------------------

CREATE TABLE quest_completions (
    quest_id      UUID NOT NULL REFERENCES quests(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    completed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (quest_id, user_id)
);

CREATE INDEX quest_completions_user_idx ON quest_completions (user_id);

-- ---------------------------------------------------------------------------
-- xp_logs
-- ---------------------------------------------------------------------------
-- Append-only ledger of XP events. Every write to `users.xp_total` and
-- `track_memberships.track_xp` is mirrored here.

CREATE TABLE xp_logs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount       INTEGER NOT NULL,
    source       TEXT NOT NULL,
    track        TEXT,
    description  TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT xp_logs_source_valid CHECK (
        source IN ('GitHub', 'QR', 'Discord', 'Review', 'Quest', 'Manual', 'Project')
    ),
    CONSTRAINT xp_logs_track_valid CHECK (
        track IS NULL
        OR track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    )
    -- Note: amount can be negative (manual revoke). No CHECK > 0.
);

CREATE INDEX xp_logs_user_created_idx ON xp_logs (user_id, created_at DESC);
CREATE INDEX xp_logs_source_idx       ON xp_logs (source, created_at DESC);
-- Daily cap query: count today's GitHub commits XP for a given user.
CREATE INDEX xp_logs_user_source_day_idx
    ON xp_logs (user_id, source, CAST(created_at AT TIME ZONE 'UTC' AS date));
-- ---------------------------------------------------------------------------
-- audit_logs
-- ---------------------------------------------------------------------------

CREATE TABLE audit_logs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id     UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action       TEXT NOT NULL,
    target_type  TEXT,
    target_id    UUID,
    metadata     JSONB,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX audit_logs_actor_idx   ON audit_logs (actor_id, created_at DESC);
CREATE INDEX audit_logs_action_idx  ON audit_logs (action, created_at DESC);
CREATE INDEX audit_logs_target_idx  ON audit_logs (target_type, target_id);
CREATE INDEX audit_logs_metadata_gin ON audit_logs USING GIN (metadata);

-- ---------------------------------------------------------------------------
-- hall_of_fame
-- ---------------------------------------------------------------------------
-- Auto-generated when a project transitions to Released.

CREATE TABLE hall_of_fame (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL UNIQUE REFERENCES projects(id) ON DELETE CASCADE,
    featured_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    total_xp_distributed  INTEGER NOT NULL DEFAULT 0,
    boss_defeated_by      UUID[] NOT NULL DEFAULT '{}',

    CONSTRAINT hall_of_fame_xp_nonneg CHECK (total_xp_distributed >= 0)
);

CREATE INDEX hall_of_fame_featured_idx ON hall_of_fame (featured_at DESC);
