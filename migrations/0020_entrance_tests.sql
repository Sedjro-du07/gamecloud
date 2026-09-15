-- 0020_entrance_tests.sql
-- Entrance tests and admission.
--
-- Somebody who is not on the association's Discord server cannot sign up
-- on their own: they take an entrance test first. The Bureau opens a
-- session with a PDF subject and a closing time, candidates hand in their
-- work until it closes, and the Bureau admits or turns each one down.
-- Admission is what opens the sign-up, and it comes with an invitation to
-- the server.

ALTER TABLE users
    -- Not on the server and not yet admitted: may take tests, may not sign up.
    ADD COLUMN candidate          BOOLEAN NOT NULL DEFAULT FALSE,
    -- When the Bureau admitted them, if it did.
    ADD COLUMN admitted_at        TIMESTAMPTZ,
    -- Single-use invitation created on admission.
    ADD COLUMN discord_invite_url TEXT;

CREATE TABLE entrance_tests (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title             TEXT NOT NULL,
    description       TEXT,
    -- The PDF subject, kept in TESTS_DIR/subjects under `subject_storage`.
    subject_filename  TEXT NOT NULL,
    subject_size      BIGINT NOT NULL,
    subject_checksum  TEXT NOT NULL,
    subject_storage   TEXT NOT NULL,
    closes_at         TIMESTAMPTZ NOT NULL,
    created_by        UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT entrance_tests_title_not_blank CHECK (length(btrim(title)) > 0),
    CONSTRAINT entrance_tests_subject_size_max CHECK (subject_size <= 524288000)
);

CREATE INDEX entrance_tests_closes_idx ON entrance_tests (closes_at DESC);

CREATE TABLE entrance_submissions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    test_id       UUID NOT NULL REFERENCES entrance_tests(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The work, kept in TESTS_DIR/submissions under `storage_name`.
    filename      TEXT NOT NULL,
    size_bytes    BIGINT NOT NULL,
    checksum      TEXT NOT NULL,
    storage_name  TEXT NOT NULL,
    comment       TEXT,
    submitted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verdict       TEXT,
    reviewed_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at   TIMESTAMPTZ,

    -- One piece of work per candidate per session, replaced until it closes.
    CONSTRAINT entrance_submissions_one_per_candidate UNIQUE (test_id, user_id),
    CONSTRAINT entrance_submissions_verdict_valid CHECK (
        verdict IS NULL OR verdict IN ('Admitted', 'Rejected')
    ),
    CONSTRAINT entrance_submissions_size_max CHECK (size_bytes <= 524288000)
);

CREATE INDEX entrance_submissions_test_idx ON entrance_submissions (test_id, submitted_at);
CREATE INDEX entrance_submissions_user_idx ON entrance_submissions (user_id);
