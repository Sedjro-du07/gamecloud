-- 0013_track_soft_leave.sql
--
-- Leaving a track used to DELETE the membership row — and that row holds
-- `track_xp`, the pool earned in that discipline. So quitting a track
-- silently destroyed the progress made in it, and rejoining started from
-- zero.
--
-- That was tolerable while leaving required a deliberate API call. It is
-- not tolerable now that a Discord role removal propagates to the
-- platform: an admin tidying up roles would wipe members' track XP
-- without anyone realising.
--
-- Membership therefore becomes soft: `left_at` marks the member as no
-- longer in the track, the pool stays, and rejoining clears the mark and
-- restores the standing.

ALTER TABLE track_memberships
    ADD COLUMN IF NOT EXISTS left_at TIMESTAMPTZ;

COMMENT ON COLUMN track_memberships.left_at IS
    'When the member left the track. NULL means active. The row is kept so track_xp survives; rejoining sets this back to NULL.';

-- Active membership is the common case in every query, so index for it.
CREATE INDEX IF NOT EXISTS track_memberships_active_idx
    ON track_memberships (user_id) WHERE left_at IS NULL;

-- The multi-track bonus counts *active* tracks; a dormant one must not
-- inflate anybody's multiplier.
CREATE INDEX IF NOT EXISTS track_memberships_active_recent_idx
    ON track_memberships (user_id, last_active_at) WHERE left_at IS NULL;
