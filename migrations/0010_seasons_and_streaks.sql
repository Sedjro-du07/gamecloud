-- 0010_seasons_and_streaks.sql
--
-- Two engagement mechanics that were designed but never wired up.
--
-- **Streaks.** `users.streak_days` was read by the XP multiplier and
-- displayed by the bot, but no code ever wrote it — so the multiplier
-- always evaluated to 1.0x. Computing a streak needs one extra piece
-- of state: the last UTC day on which the user earned XP, so we can
-- tell "yesterday" (continue) from "today" (no-op) from "older"
-- (reset).
--
-- **Seasons.** An all-time leaderboard freezes: founders stay on top
-- and newcomers can never catch up. Seasons scope the competition to a
-- window (an Epitech block, a semester) while total XP and ranks stay
-- cumulative forever.

-- ---------------------------------------------------------------------------
-- Streaks
-- ---------------------------------------------------------------------------

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS last_streak_day DATE,
    ADD COLUMN IF NOT EXISTS longest_streak  INTEGER NOT NULL DEFAULT 0;

ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_longest_streak_nonneg;
ALTER TABLE users
    ADD CONSTRAINT users_longest_streak_nonneg CHECK (longest_streak >= 0);

-- ---------------------------------------------------------------------------
-- Seasons
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS seasons (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    description TEXT,
    starts_at   TIMESTAMPTZ NOT NULL,
    ends_at     TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT seasons_window_valid CHECK (ends_at > starts_at),
    CONSTRAINT seasons_slug_format  CHECK (slug ~ '^[a-z0-9][a-z0-9-]*$')
);

CREATE INDEX IF NOT EXISTS seasons_window_idx ON seasons (starts_at, ends_at);

-- At most one season may be open at any instant. An exclusion
-- constraint over the time range expresses that precisely; it needs
-- btree_gist for the `&&` operator on tstzrange.
CREATE EXTENSION IF NOT EXISTS btree_gist;

ALTER TABLE seasons
    DROP CONSTRAINT IF EXISTS seasons_no_overlap;
ALTER TABLE seasons
    ADD CONSTRAINT seasons_no_overlap
    EXCLUDE USING gist (tstzrange(starts_at, ends_at) WITH &&);

-- Every XP event is stamped with the season that was open when it
-- happened. NULL means "earned outside any season" (pre-seasons data,
-- or a gap between seasons) and simply never shows on a season board.
ALTER TABLE xp_logs
    ADD COLUMN IF NOT EXISTS season_id UUID REFERENCES seasons(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS xp_logs_season_user_idx
    ON xp_logs (season_id, user_id) WHERE season_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Seed: an opening season so the feature is live on first boot.
-- ---------------------------------------------------------------------------
-- Deliberately wide (one academic year). The Bureau can close it early
-- by editing `ends_at`; the exclusion constraint then allows the next
-- one to start.

INSERT INTO seasons (name, slug, description, starts_at, ends_at)
SELECT
    'Saison 1 — La Forge',
    'saison-1-la-forge',
    'Première saison de GameCloud OS. Tout le XP gagné pendant cette fenêtre compte pour le classement saisonnier.',
    date_trunc('day', NOW()),
    date_trunc('day', NOW()) + INTERVAL '1 year'
WHERE NOT EXISTS (SELECT 1 FROM seasons);
