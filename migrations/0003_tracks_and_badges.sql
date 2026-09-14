-- 0003_tracks_and_badges.sql
-- Track memberships (one user × N tracks) and special badges.

-- ---------------------------------------------------------------------------
-- track_memberships
-- ---------------------------------------------------------------------------
-- A user may belong to 1..=8 tracks simultaneously.

CREATE TABLE track_memberships (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track           TEXT NOT NULL,
    specialization  TEXT,
    track_role      TEXT NOT NULL DEFAULT 'Observer',
    track_xp        BIGINT NOT NULL DEFAULT 0,
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at  TIMESTAMPTZ,

    CONSTRAINT track_memberships_track_valid CHECK (
        track IN (
            'Engineering', 'GameDesign', 'Narrative', 'VisualArt',
            'Audio', 'Production', 'QA', 'Marketing'
        )
    ),
    CONSTRAINT track_memberships_role_valid CHECK (
        track_role IN ('Observer', 'Contributor', 'Reviewer', 'Mentor', 'CoLead', 'Lead')
    ),
    CONSTRAINT track_memberships_xp_nonneg CHECK (track_xp >= 0)
);

-- One membership per (user, track).
CREATE UNIQUE INDEX track_memberships_user_track_idx
    ON track_memberships (user_id, track);

-- Lookup all members of a track sorted by XP — common leaderboard query.
CREATE INDEX track_memberships_track_xp_idx
    ON track_memberships (track, track_xp DESC);

-- Lookup leads/co-leads quickly.
CREATE INDEX track_memberships_leads_idx
    ON track_memberships (track, track_role)
    WHERE track_role IN ('Lead', 'CoLead');

-- ---------------------------------------------------------------------------
-- special_badges
-- ---------------------------------------------------------------------------

CREATE TABLE special_badges (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    badge_type  TEXT NOT NULL,
    awarded_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    awarded_by  UUID REFERENCES users(id) ON DELETE SET NULL,

    CONSTRAINT special_badges_type_valid CHECK (
        badge_type IN (
            'FoundingMember', 'Alumni', 'ExternalMentor', 'GameJamWinner',
            'GameJamParticipant', 'BugHunter', 'Contributor', 'Streaker',
            'BlockMaster', 'MultiTracker', 'Validator', 'Mentor',
            'TopContributor', 'NightOwl', 'SpeedRunner'
        )
    )
);

-- A user can own a badge only once. (Multiple Game Jam wins are recorded
-- through metadata in audit_logs, not duplicate rows.)
CREATE UNIQUE INDEX special_badges_user_type_idx
    ON special_badges (user_id, badge_type);

CREATE INDEX special_badges_user_idx ON special_badges (user_id);
