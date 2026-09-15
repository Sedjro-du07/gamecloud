-- Members are named by their Discord pseudo, and by nothing else.
--
-- The platform used to show a member's title as their name, and fell back
-- to the raw Discord snowflake when nothing better was known. The
-- president's rule is now that identifiers never reach the interface: a
-- member is their Discord username, or their Discord display name when the
-- username is missing, and otherwise a neutral word — never a number.
--
-- The signature is unchanged so every caller keeps working. A missing row
-- (a LEFT JOIN that found no member, where the snowflake is NULL too)
-- still yields NULL, so "nobody" stays distinguishable from "a member".
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
    SELECT CASE
        WHEN p_discord_id IS NULL AND p_username IS NULL AND p_global_name IS NULL THEN NULL
        ELSE COALESCE(
            NULLIF(btrim(p_username), ''),
            NULLIF(btrim(p_global_name), ''),
            'Membre sans pseudo'
        )
    END;
$$;
