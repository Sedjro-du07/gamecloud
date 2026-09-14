-- Human-readable Discord identity.
--
-- Until now the only name the platform held for a member was the Discord
-- snowflake, so every list, leaderboard row and audit line fell back to
-- showing an eighteen-digit number. That is unreadable, and for a
-- gamified intranet it defeats the point: members should recognise each
-- other by the name they chose and the title they earned.
--
-- Two columns rather than one because Discord has two names: `username`
-- is the globally unique handle, `global_name` the display name the
-- member actually picked. The display name is preferred when set.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS discord_username    TEXT,
    ADD COLUMN IF NOT EXISTS discord_global_name TEXT;

COMMENT ON COLUMN users.discord_username    IS 'Discord handle, e.g. "aevaryn".';
COMMENT ON COLUMN users.discord_global_name IS 'Discord display name, preferred over the handle.';

-- Resolve a member to the best name available, in the order the product
-- wants it: the title they earned, then the name they chose, then their
-- handle, and only as a last resort the raw snowflake. Defined as a
-- function so the precedence lives in exactly one place instead of being
-- copied into a dozen COALESCE chains that can drift apart.
CREATE OR REPLACE FUNCTION member_display_name(
    p_title       TEXT,
    p_global_name TEXT,
    p_username    TEXT,
    p_discord_id  TEXT
) RETURNS TEXT
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT COALESCE(
        NULLIF(btrim(p_title), ''),
        NULLIF(btrim(p_global_name), ''),
        NULLIF(btrim(p_username), ''),
        p_discord_id
    );
$$;
