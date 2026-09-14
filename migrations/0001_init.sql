-- 0001_init.sql
-- Bootstrap: extensions and shared helpers.

-- pgcrypto provides gen_random_uuid() (used as DEFAULT on every PK).
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- citext gives us a case-insensitive text type, useful for usernames
-- and (notably) Epitech emails. We still validate format via CHECK.
CREATE EXTENSION IF NOT EXISTS citext;

-- Helper: generic updated_at trigger function. Plain SQL (no plpgsql)
-- would not allow OLD/NEW access, so we use plpgsql.
CREATE OR REPLACE FUNCTION trg_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
