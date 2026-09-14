-- DraftBot level-ups earned by server members who have no platform
-- account yet.
--
-- They used to be dropped: the sync endpoint answered "ignored" for an
-- unknown Discord id, so everything a member did on the server before
-- signing in was lost. They are now kept here and paid through the
-- normal XP grant the first time the member signs in.

CREATE TABLE IF NOT EXISTS draftbot_pending_levels (
    discord_id  TEXT        NOT NULL,
    level       INTEGER     NOT NULL CHECK (level > 0),
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (discord_id, level)
);
