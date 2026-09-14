-- Who an event is for.
--
-- `track IS NULL` used to mean "the whole association", which left no
-- way to express a third, genuinely different case: a Bureau meeting.
-- Those are not merely unannounced — they must not appear on an ordinary
-- member's calendar at all, which is a visibility rule and not something
-- a nullable column can carry.
--
-- Three values rather than a boolean, because the scope also decides who
-- may edit the event and which Discord channel hears about it.

ALTER TABLE events
    ADD COLUMN IF NOT EXISTS audience TEXT NOT NULL DEFAULT 'Association';

-- Existing rows: anything scoped to a track was a track event, the rest
-- concerned everybody. Nothing was ever Bureau-only before now.
UPDATE events
   SET audience = CASE WHEN track IS NOT NULL THEN 'Track' ELSE 'Association' END
 WHERE audience = 'Association';

ALTER TABLE events DROP CONSTRAINT IF EXISTS events_audience_valid;
ALTER TABLE events
    ADD CONSTRAINT events_audience_valid CHECK (
        audience IN ('Association', 'Track', 'Bureau')
    );

-- A track event without a track is meaningless, and a track on an event
-- that is not track-scoped is a contradiction the calendar would have to
-- guess about.
ALTER TABLE events DROP CONSTRAINT IF EXISTS events_track_matches_audience;
ALTER TABLE events
    ADD CONSTRAINT events_track_matches_audience CHECK (
        (audience = 'Track' AND track IS NOT NULL)
     OR (audience <> 'Track' AND track IS NULL)
    );

CREATE INDEX IF NOT EXISTS events_audience_idx ON events (audience);

COMMENT ON COLUMN events.audience IS
    'Association | Track | Bureau. Drives visibility, edit rights and Discord routing.';
