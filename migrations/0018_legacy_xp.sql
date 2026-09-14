-- XP carried over from before the platform.
--
-- Members earned activity XP with Kumo before GameCloud OS existed. When
-- progression is reset, that XP is carried over into the member's global
-- XP — and so into their member title — but not into any track, and it
-- grants no office.
--
-- One row per Discord member. Members already on the platform are
-- credited when the snapshot is loaded; everyone else the first time they
-- sign in, at which point `credited_at` is set so it is paid only once.

CREATE TABLE IF NOT EXISTS legacy_xp (
    discord_id  TEXT        PRIMARY KEY,
    xp          INTEGER     NOT NULL CHECK (xp > 0),
    source      TEXT        NOT NULL DEFAULT 'Kumo',
    loaded_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    credited_at TIMESTAMPTZ
);
