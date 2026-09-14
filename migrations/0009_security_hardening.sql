-- 0009_security_hardening.sql
--
-- Closes three defects found in the May 2026 audit:
--
-- 1. OTP brute-force: `attempts` was reset to 0 every time the user
--    re-requested a code, so the 5-attempt lockout could be bypassed
--    indefinitely by alternating /api/auth/email and /api/auth/verify.
--    We add a *cumulative* counter that survives resends, plus resend
--    bookkeeping so the handler can enforce a cooldown and a ceiling.
--
-- 2. QR tokens were stored in plaintext. A read of `qr_tokens` handed
--    out working bearer tokens. We now store a SHA-256 hash, exactly
--    like `refresh_tokens` already did.
--
-- 3. QR tokens were single-use *globally* (`is_used`), so only the
--    first member to scan an event code was credited. Attendance is
--    now claimed per (token, user): the token stays valid until it
--    expires, but each member can only claim it once.

-- ---------------------------------------------------------------------------
-- 1. OTP hardening
-- ---------------------------------------------------------------------------

ALTER TABLE email_otps
    ADD COLUMN IF NOT EXISTS cumulative_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS resend_count        INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_sent_at        TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE email_otps
    DROP CONSTRAINT IF EXISTS email_otps_cumulative_nonneg;
ALTER TABLE email_otps
    ADD CONSTRAINT email_otps_cumulative_nonneg
    CHECK (cumulative_attempts >= 0 AND resend_count >= 0);

-- ---------------------------------------------------------------------------
-- 2. QR token hashing
-- ---------------------------------------------------------------------------

ALTER TABLE qr_tokens
    ADD COLUMN IF NOT EXISTS token_hash TEXT;

-- Existing rows (dev data only) get the hash of their plaintext token
-- so nothing is orphaned. pgcrypto is already installed by 0001.
UPDATE qr_tokens
   SET token_hash = encode(digest(token, 'sha256'), 'hex')
 WHERE token_hash IS NULL;

ALTER TABLE qr_tokens
    ALTER COLUMN token_hash SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS qr_tokens_token_hash_idx ON qr_tokens (token_hash);

-- The plaintext column becomes dead weight. Keep it nullable so the
-- migration is reversible, but stop writing it.
ALTER TABLE qr_tokens
    ALTER COLUMN token DROP NOT NULL;

COMMENT ON COLUMN qr_tokens.token IS
    'DEPRECATED as of 0009 — plaintext is no longer persisted. Use token_hash.';

-- ---------------------------------------------------------------------------
-- 3. Per-user QR claims
-- ---------------------------------------------------------------------------

ALTER TABLE attendance
    ADD COLUMN IF NOT EXISTS qr_token_id UUID REFERENCES qr_tokens(id) ON DELETE SET NULL;

-- One claim per (member, token). This is the replay defence that
-- `is_used` used to provide, without preventing everybody else at the
-- event from scanning the same code.
CREATE UNIQUE INDEX IF NOT EXISTS attendance_user_token_idx
    ON attendance (user_id, qr_token_id)
    WHERE qr_token_id IS NOT NULL;

-- A token now carries an optional capacity limit. NULL = unlimited
-- until expiry, which is the sensible default for a session QR shown
-- on a projector.
ALTER TABLE qr_tokens
    ADD COLUMN IF NOT EXISTS max_scans INTEGER,
    ADD COLUMN IF NOT EXISTS scan_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE qr_tokens
    DROP CONSTRAINT IF EXISTS qr_tokens_scan_bounds;
ALTER TABLE qr_tokens
    ADD CONSTRAINT qr_tokens_scan_bounds
    CHECK (scan_count >= 0 AND (max_scans IS NULL OR max_scans > 0));

COMMENT ON COLUMN qr_tokens.is_used IS
    'DEPRECATED as of 0009 — superseded by per-user claims in attendance.';

-- The old partial index assumed is_used was meaningful.
DROP INDEX IF EXISTS qr_tokens_active_idx;
CREATE INDEX IF NOT EXISTS qr_tokens_expires_idx ON qr_tokens (expires_at);
