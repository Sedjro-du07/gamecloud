-- 0029_involvement.sql
-- Involvement levels, credits and the involvement sheet.
--
-- The level itself is never stored: it is computed from what the
-- platform already records (see `gamecloud_shared::involvement`), month
-- by month, so it can neither drift nor be edited by hand. What is stored
-- here is what the platform cannot know:
--
--   * who the member is on paper — first name, last name, promotion — so
--     the sheet handed to the school names them. Each member fills it in
--     themselves; it is shown only to them and to the Bureau;
--   * what the President or the Vice-President decided for a school year:
--     a forced level (with its reason, mandatory) and the remarks printed
--     on the sheet.

ALTER TABLE users
    ADD COLUMN first_name TEXT,
    ADD COLUMN last_name  TEXT,
    ADD COLUMN promotion  TEXT,
    ADD CONSTRAINT users_promotion_valid CHECK (
        promotion IS NULL OR promotion IN ('Tek1', 'Tek2', 'Tek3', 'Tek4', 'Tek5', 'MSc', 'Autre')
    ),
    ADD CONSTRAINT users_first_name_len CHECK (first_name IS NULL OR char_length(btrim(first_name)) BETWEEN 1 AND 80),
    ADD CONSTRAINT users_last_name_len  CHECK (last_name  IS NULL OR char_length(btrim(last_name))  BETWEEN 1 AND 80);

CREATE TABLE involvement_reviews (
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- First year of the school year: 2026 for 2026-2027 (September to August).
    school_year   INTEGER NOT NULL,
    forced_level  SMALLINT,
    force_reason  TEXT,
    remarks       TEXT,
    updated_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, school_year),
    CONSTRAINT involvement_reviews_level_valid CHECK (forced_level IS NULL OR forced_level BETWEEN 1 AND 5),
    -- Forcing a level always says why.
    CONSTRAINT involvement_reviews_force_has_reason CHECK (
        forced_level IS NULL OR (force_reason IS NOT NULL AND char_length(btrim(force_reason)) > 0)
    ),
    CONSTRAINT involvement_reviews_remarks_len CHECK (remarks IS NULL OR char_length(remarks) <= 2000)
);
