-- 0002_users.sql
-- Core user table, email OTPs, refresh tokens.

-- ---------------------------------------------------------------------------
-- users
-- ---------------------------------------------------------------------------
-- One row per platform account. Created on Discord OAuth completion;
-- the email and `email_verified` columns drive the type-state classification
-- in `gamecloud_shared::account`.

CREATE TABLE users (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    discord_id         TEXT NOT NULL UNIQUE,
    github_username    TEXT,
    email              CITEXT UNIQUE,
    email_verified     BOOLEAN NOT NULL DEFAULT FALSE,
    avatar_url         TEXT,
    avatar_custom_url  TEXT,
    xp_total           BIGINT NOT NULL DEFAULT 0,
    level              INTEGER NOT NULL DEFAULT 1,
    global_rank        TEXT NOT NULL DEFAULT 'Pending',
    bureau_role        TEXT,
    current_title      TEXT,
    streak_days        INTEGER NOT NULL DEFAULT 0,
    last_activity_at   TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Domain invariants -----------------------------------------------------

    -- Email, when present, must match the Epitech format. The regex is
    -- intentionally restrictive: lower-case `firstname.lastname@epitech.eu`.
    -- Defensive duplicate of the application-level check.
    CONSTRAINT users_email_epitech_format CHECK (
        email IS NULL
        OR email ~ '^[a-z]+\.[a-z]+@epitech\.eu$'
    ),

    -- email_verified can only be true if email is present.
    CONSTRAINT users_email_verified_consistency CHECK (
        email_verified = FALSE OR email IS NOT NULL
    ),

    -- xp_total is non-negative.
    CONSTRAINT users_xp_total_nonneg CHECK (xp_total >= 0),

    -- streak is non-negative.
    CONSTRAINT users_streak_nonneg CHECK (streak_days >= 0),

    -- Allowed values for global_rank.
    CONSTRAINT users_global_rank_valid CHECK (
        global_rank IN (
            'Pending', 'Visitor', 'Initiate', 'Apprentice', 'JuniorDev',
            'SeniorDev', 'Expert', 'Veteran', 'Legend', 'Myth'
        )
    ),

    -- Pending users must not have email_verified = true.
    -- (Users above Visitor can only exist with verified email — enforced
    --  at the application layer via the type-state pattern; this CHECK is
    --  the cheap last line of defense.)
    CONSTRAINT users_pending_implies_unverified CHECK (
        global_rank <> 'Pending' OR email_verified = FALSE
    ),
    CONSTRAINT users_above_visitor_requires_verification CHECK (
        global_rank IN ('Pending', 'Visitor') OR email_verified = TRUE
    )
);

CREATE INDEX users_xp_total_desc_idx     ON users (xp_total DESC);
CREATE INDEX users_global_rank_idx       ON users (global_rank);
CREATE INDEX users_last_activity_idx     ON users (last_activity_at DESC NULLS LAST);
CREATE INDEX users_bureau_role_idx       ON users (bureau_role) WHERE bureau_role IS NOT NULL;
CREATE INDEX users_github_username_idx   ON users (github_username) WHERE github_username IS NOT NULL;

-- ---------------------------------------------------------------------------
-- email_otps
-- ---------------------------------------------------------------------------
-- One outstanding OTP per user. We always upsert (DELETE + INSERT) so
-- there is at most one row per user_id.

CREATE TABLE email_otps (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email       CITEXT NOT NULL,
    code_hash   TEXT NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT email_otps_email_format CHECK (
        email ~ '^[a-z]+\.[a-z]+@epitech\.eu$'
    ),
    CONSTRAINT email_otps_attempts_nonneg CHECK (attempts >= 0)
);

CREATE UNIQUE INDEX email_otps_one_per_user_idx ON email_otps (user_id);
CREATE INDEX email_otps_expires_idx ON email_otps (expires_at);

-- ---------------------------------------------------------------------------
-- refresh_tokens
-- ---------------------------------------------------------------------------

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked     BOOLEAN NOT NULL DEFAULT FALSE,
    user_agent  TEXT,
    ip_address  INET,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX refresh_tokens_user_idx ON refresh_tokens (user_id) WHERE revoked = FALSE;
CREATE INDEX refresh_tokens_expires_idx ON refresh_tokens (expires_at) WHERE revoked = FALSE;
