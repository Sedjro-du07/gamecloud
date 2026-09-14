-- Calendar, and a mark on a track verdict.
--
-- Three gaps this closes.
--
-- 1. Attendance existed but nothing to attend. `qr_tokens` carried a
--    free-text `event_name`, so two QR codes for the same weekly session
--    were unrelated rows and nobody could ask "who came to the Tuesday
--    sessions this month". Events become real, and tokens and
--    attendance hang off them.
--
-- 2. There was no calendar. Sessions and events lived in somebody's
--    head, or in a Discord message that scrolled away.
--
-- 3. `track_validations` recorded Approve/Reject and free-text feedback
--    but no mark, so a track could say "yes" or "no" and nothing in
--    between. A score lets a track grade work rather than merely gate it.

-- ---------------------------------------------------------------------------
-- Events
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS events (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title          TEXT        NOT NULL,
    description    TEXT,
    -- What kind of gathering. `Session` is the recurring working
    -- meeting; the rest are one-offs.
    kind           TEXT        NOT NULL DEFAULT 'Session',
    -- NULL means the whole association is concerned. A track id scopes
    -- the event to that track, which is also what decides who may edit
    -- it: a track Lead owns their own track's sessions.
    track          TEXT,
    starts_at      TIMESTAMPTZ NOT NULL,
    ends_at        TIMESTAMPTZ NOT NULL,
    location       TEXT,
    -- XP handed to whoever scans in. Zero is legitimate: a deadline is
    -- on the calendar to be seen, not to be attended.
    xp_reward      INTEGER     NOT NULL DEFAULT 0,
    created_by     UUID        NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Cancelled rather than deleted: people already scanned in, and the
    -- attendance has to keep pointing somewhere.
    cancelled_at   TIMESTAMPTZ,

    CONSTRAINT events_kind_valid CHECK (
        kind IN ('Session', 'Workshop', 'Jam', 'Meeting', 'Deadline', 'Showcase')
    ),
    CONSTRAINT events_span_valid  CHECK (ends_at >= starts_at),
    CONSTRAINT events_xp_sane     CHECK (xp_reward BETWEEN 0 AND 500),
    CONSTRAINT events_title_len   CHECK (char_length(btrim(title)) BETWEEN 1 AND 120)
);

-- The calendar is always read as "everything between these two
-- instants", so that is the index.
CREATE INDEX IF NOT EXISTS events_starts_at_idx ON events (starts_at);
CREATE INDEX IF NOT EXISTS events_track_idx     ON events (track) WHERE track IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Tie attendance and tokens to events
-- ---------------------------------------------------------------------------

-- Nullable, and no backfill: rows written before events existed keep
-- their free-text `event_name` and simply have no event to point at.
ALTER TABLE qr_tokens  ADD COLUMN IF NOT EXISTS event_id UUID REFERENCES events(id) ON DELETE SET NULL;
ALTER TABLE attendance ADD COLUMN IF NOT EXISTS event_id UUID REFERENCES events(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS attendance_event_idx ON attendance (event_id) WHERE event_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS qr_tokens_event_idx  ON qr_tokens  (event_id) WHERE event_id IS NOT NULL;

-- A member counts once per event however many times they scan.
CREATE UNIQUE INDEX IF NOT EXISTS attendance_one_per_event
    ON attendance (user_id, event_id) WHERE event_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- A mark on the track verdict
-- ---------------------------------------------------------------------------

ALTER TABLE track_validations
    ADD COLUMN IF NOT EXISTS score INTEGER;

ALTER TABLE track_validations
    DROP CONSTRAINT IF EXISTS track_validations_score_range;
ALTER TABLE track_validations
    ADD CONSTRAINT track_validations_score_range
    CHECK (score IS NULL OR score BETWEEN 0 AND 100);

COMMENT ON COLUMN track_validations.score IS
    'Mark out of 100 for this track''s share of the project. NULL when the reviewer only gated.';
