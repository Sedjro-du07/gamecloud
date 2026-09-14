-- 0012_real_logout.sql
--
-- Logging out did not actually log you out.
--
-- Two defects, found by testing the running server:
--
-- 1. The cookies were removed with `Cookie::from(NAME)`, which carries no
--    `Path`. A cookie is identified by (name, domain, path), so a removal
--    cookie without a path defaults to the request's directory
--    (`/api/auth`) and never matches the session cookie, which was set
--    with `Path=/`. The browser therefore kept the session: the member
--    saw themselves logged out in the UI while every request stayed
--    authenticated.
--
-- 2. Even with the cookies gone, the access JWT stayed valid until its
--    `exp` — up to an hour. Logout revoked the *refresh* token but had no
--    way to invalidate an access token already issued, so a copied token
--    (or a restored cookie) kept working.
--
-- This migration addresses (2). A single timestamp per member acts as a
-- cheap revocation epoch: any access token issued before it is refused.
-- The auth extractor already loads the member's row on every request, so
-- enforcing it costs no extra query.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS sessions_valid_from TIMESTAMPTZ;

COMMENT ON COLUMN users.sessions_valid_from IS
    'Access tokens issued (iat) before this instant are rejected. Set by logout, and by any future "sign out everywhere" or credential-reset action. NULL means no revocation has ever happened.';
