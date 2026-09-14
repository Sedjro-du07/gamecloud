-- 0008_email_regex_relax.sql
-- The original CHECK on `users.email` and `email_otps.email` only
-- accepted `[a-z]+\.[a-z]+@epitech.eu` — i.e. no hyphens, no digits.
-- That rejected legitimate Epitech addresses such as
-- `joachim.goeh-akue@epitech.eu` and `john.doe2@epitech.eu`.
--
-- We drop the strict constraints and replace them with a relaxed
-- version that mirrors the application-level validator in
-- `crates/web/src/services/email_validator.rs`.

ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_email_epitech_format;

ALTER TABLE users
    ADD CONSTRAINT users_email_epitech_format CHECK (
        email IS NULL
        OR email ~ '^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$'
    );

ALTER TABLE email_otps
    DROP CONSTRAINT IF EXISTS email_otps_email_format;

ALTER TABLE email_otps
    ADD CONSTRAINT email_otps_email_format CHECK (
        email ~ '^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$'
    );
