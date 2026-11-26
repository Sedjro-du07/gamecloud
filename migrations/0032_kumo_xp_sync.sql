-- Two-way XP sync with Kumo.
--
-- One row per Discord member Kumo has reported on. Keyed by Discord id,
-- not by user: a member who talks on Discord before ever signing in is
-- remembered here and paid at their first login.
--
--   baseline       Kumo XP that is already accounted for and must not be
--                  paid again. For the members of the 2026-09-14 snapshot
--                  it is their `legacy_xp` (already carried over 1:1);
--                  for everybody else it is Kumo's own organic total on
--                  the day the sync was switched on, so the reset that
--                  sent them back to zero is respected.
--   organic_total  The latest cumulative Kumo XP earned by chatting that
--                  Kumo reported. Only ever raised.
--   credited       Platform XP paid so far for the Kumo activity above the
--                  baseline. Also what the platform → Kumo direction
--                  must leave out, so Kumo XP never comes back to Kumo.
CREATE TABLE IF NOT EXISTS kumo_sync (
    discord_id    TEXT        PRIMARY KEY,
    baseline      BIGINT      NOT NULL CHECK (baseline >= 0),
    organic_total BIGINT      NOT NULL CHECK (organic_total >= 0),
    credited      BIGINT      NOT NULL DEFAULT 0 CHECK (credited >= 0),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
