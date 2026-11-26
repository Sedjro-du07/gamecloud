-- 0030_involvement_max.sql
-- Guaranteed maximal involvement.
--
-- The President and the Vice-President, by their office, and the members
-- they name, are always shown at maximal involvement (level 5, 3 credits
-- recommended), whatever happens. Their analysis — score, axes, months —
-- is still computed and shown as it is; only the level that counts and
-- the credits are pinned. The President or the Vice-President grant or
-- lift it from the Bureau page; it is written to the journal.

ALTER TABLE users ADD COLUMN involvement_max BOOLEAN NOT NULL DEFAULT FALSE;

-- Named by the President, November 2026.
UPDATE users SET involvement_max = TRUE
 WHERE lower(discord_username) IN ('yange4legend', 'dragon710538', 'y4n1s_79131');
