-- No more email at all.
--
-- 0025 stopped the verified address from gating anything. This removes
-- it: signing in with Discord is the whole of membership, the platform
-- sends no mail, and a column that still said `email_verified` was the
-- thread that the old gate kept being rewoven along — it had been
-- adopted, one feature at a time, as the definition of "member" in five
-- pages, the leaderboard, QR attendance and the bot.

DROP TABLE IF EXISTS email_otps;

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_verified_consistency;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_epitech_format;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_key;
ALTER TABLE users DROP COLUMN IF EXISTS email_verified;
ALTER TABLE users DROP COLUMN IF EXISTS email;
