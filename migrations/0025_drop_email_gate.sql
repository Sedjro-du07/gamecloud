-- Stop making a verified Epitech address a condition of existing.
--
-- The ladder used to start below its own floor: a member sat at
-- `Pending` until they had verified an `@epitech.eu` address, and two
-- CHECK constraints enforced it in the database as well. The effect was
-- that XP accumulated against a rank that could not move — one member
-- reached 14 844 XP while still displayed as `⏳ L'Aspirant`, unable to
-- create a project, review anything, or use the office they had been
-- given.
--
-- The association's own answer is simpler and is the one that matches
-- how it actually works: getting into the Discord server *is* the
-- membership check. The mail column stays — it is still useful to hold
-- one — but nothing depends on it any more.

-- The two constraints that tied rank to verification.
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_above_visitor_requires_verification;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_pending_implies_unverified;

-- `Pending` and `Visitor` are no longer reachable states. They stay in
-- the allowed list because historic rows and the Discord role ladder
-- still name them, and because removing a value from a CHECK is the kind
-- of change that breaks a rollback.

-- Re-derive every rank from the XP that was already earned. Mirrors
-- `GlobalRank::from_xp` exactly; `Initiate` is the floor.
UPDATE users
   SET global_rank = CASE
         WHEN xp_total >= 25000 THEN 'Myth'
         WHEN xp_total >= 10000 THEN 'Legend'
         WHEN xp_total >=  5000 THEN 'Veteran'
         WHEN xp_total >=  2500 THEN 'Expert'
         WHEN xp_total >=  1000 THEN 'SeniorDev'
         WHEN xp_total >=   400 THEN 'JuniorDev'
         WHEN xp_total >=   150 THEN 'Apprentice'
         ELSE 'Initiate'
       END
 WHERE global_rank IN ('Pending', 'Visitor')
    OR global_rank <> CASE
         WHEN xp_total >= 25000 THEN 'Myth'
         WHEN xp_total >= 10000 THEN 'Legend'
         WHEN xp_total >=  5000 THEN 'Veteran'
         WHEN xp_total >=  2500 THEN 'Expert'
         WHEN xp_total >=  1000 THEN 'SeniorDev'
         WHEN xp_total >=   400 THEN 'JuniorDev'
         WHEN xp_total >=   150 THEN 'Apprentice'
         ELSE 'Initiate'
       END;

-- An email that was never verified is not a claim worth keeping: the
-- address was typed to pass a gate that no longer exists.
UPDATE users SET email = NULL WHERE email_verified = false AND email IS NOT NULL;

COMMENT ON COLUMN users.email_verified IS
    'Kept for the optional OTP flow. Gates nothing: Discord membership is the membership check.';
