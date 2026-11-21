-- 0028_entrance_applications.sql
-- Pre-registration for entrance tests, without a Discord account.
--
-- Taking a test used to require signing in with Discord. Now anybody
-- pre-registers on the site with their name, e-mail, GitHub and Discord
-- username, and follows their application through a private link. Two
-- requests go to the Bureau: access to the subject, then the verdict on
-- the work handed in. Admission creates the invitation to the Discord
-- server that the applicant's page redirects to.
--
-- The link carries a random token; only its SHA-256 is stored, the way
-- refresh tokens are. The platform sends no e-mail, so the Bureau can
-- issue a fresh link (replacing the hash) for somebody who lost theirs.
--
-- Hand-ins made through the old Discord sign-in stay in
-- entrance_submissions and remain readable and gradable by the Bureau.

CREATE TABLE entrance_applications (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    test_id            UUID NOT NULL REFERENCES entrance_tests(id) ON DELETE CASCADE,
    token_hash         TEXT NOT NULL UNIQUE,

    full_name          TEXT NOT NULL,
    email              TEXT NOT NULL,
    github_username    TEXT NOT NULL,
    discord_username   TEXT NOT NULL,
    motivation         TEXT,

    -- Requested -> Granted | Refused            (Bureau: access to the subject)
    -- Granted   -> HandedIn                      (applicant: work handed in)
    -- HandedIn  -> HandedIn                      (replaced while the test is open)
    -- HandedIn  -> Admitted | Rejected           (Bureau: verdict)
    status             TEXT NOT NULL DEFAULT 'Requested',

    -- The work, kept in TESTS_DIR/submissions under `storage_name`.
    filename           TEXT,
    size_bytes         BIGINT,
    checksum           TEXT,
    storage_name       TEXT,
    comment            TEXT,
    submitted_at       TIMESTAMPTZ,

    access_decided_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    access_decided_at  TIMESTAMPTZ,
    reviewed_by        UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at        TIMESTAMPTZ,
    -- Single-use invitation created on admission.
    discord_invite_url TEXT,

    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT entrance_applications_status_valid CHECK (
        status IN ('Requested', 'Granted', 'Refused', 'HandedIn', 'Admitted', 'Rejected')
    ),
    CONSTRAINT entrance_applications_name_not_blank CHECK (length(btrim(full_name)) > 0),
    CONSTRAINT entrance_applications_work_present CHECK (
        status NOT IN ('HandedIn', 'Admitted', 'Rejected') OR storage_name IS NOT NULL
    ),
    CONSTRAINT entrance_applications_size_max CHECK (size_bytes IS NULL OR size_bytes <= 524288000)
);

-- One application per person per session, recognised by their Discord username.
CREATE UNIQUE INDEX entrance_applications_one_per_discord
    ON entrance_applications (test_id, lower(discord_username));
CREATE INDEX entrance_applications_test_idx ON entrance_applications (test_id, created_at);
