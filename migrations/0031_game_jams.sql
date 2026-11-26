-- 0031_game_jams.sql
-- Internal game jams, organised on the platform.
--
-- The Bureau opens a jam with a theme (hidden until it starts), three
-- dates and its reward categories. Members form teams, hand in one game
-- per team, then everybody votes 1 to 5 in each category — the Bureau's
-- jury and the members weigh half each (see `gamecloud_shared::jams`).
-- Publishing the results stores the winners and pays the XP and badges.

CREATE TABLE game_jams (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                 TEXT NOT NULL,
    description           TEXT,
    rules                 TEXT,
    theme                 TEXT NOT NULL,
    starts_at             TIMESTAMPTZ NOT NULL,
    ends_at               TIMESTAMPTZ NOT NULL,
    voting_ends_at        TIMESTAMPTZ NOT NULL,
    max_team_size         SMALLINT NOT NULL DEFAULT 4,
    participation_xp      INTEGER NOT NULL DEFAULT 100,
    results_published_at  TIMESTAMPTZ,
    created_by            UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT game_jams_title_len CHECK (char_length(btrim(title)) BETWEEN 1 AND 120),
    CONSTRAINT game_jams_theme_len CHECK (char_length(btrim(theme)) BETWEEN 1 AND 200),
    CONSTRAINT game_jams_dates CHECK (starts_at < ends_at AND ends_at < voting_ends_at),
    CONSTRAINT game_jams_team_size CHECK (max_team_size BETWEEN 1 AND 10),
    CONSTRAINT game_jams_participation_xp CHECK (participation_xp BETWEEN 0 AND 1000)
);
CREATE INDEX game_jams_starts_idx ON game_jams (starts_at DESC);

CREATE TABLE jam_categories (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    jam_id     UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    reward     TEXT,
    xp_reward  INTEGER NOT NULL DEFAULT 200,
    position   SMALLINT NOT NULL,

    CONSTRAINT jam_categories_name_len CHECK (char_length(btrim(name)) BETWEEN 1 AND 60),
    CONSTRAINT jam_categories_xp CHECK (xp_reward BETWEEN 0 AND 2000)
);
CREATE UNIQUE INDEX jam_categories_name_idx ON jam_categories (jam_id, lower(name));

CREATE TABLE jam_teams (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    jam_id      UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT jam_teams_name_len CHECK (char_length(btrim(name)) BETWEEN 1 AND 60)
);
CREATE UNIQUE INDEX jam_teams_name_idx ON jam_teams (jam_id, lower(name));

-- One team per person per jam: `jam_id` is repeated here so the database
-- itself refuses a second team.
CREATE TABLE jam_team_members (
    team_id   UUID NOT NULL REFERENCES jam_teams(id) ON DELETE CASCADE,
    jam_id    UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_id, user_id),
    UNIQUE (jam_id, user_id)
);

-- One game per team, replaceable until submissions close.
CREATE TABLE jam_entries (
    team_id       UUID PRIMARY KEY REFERENCES jam_teams(id) ON DELETE CASCADE,
    jam_id        UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    title         TEXT NOT NULL,
    description   TEXT,
    url           TEXT,
    filename      TEXT,
    size_bytes    BIGINT,
    checksum      TEXT,
    storage_name  TEXT,
    submitted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT jam_entries_title_len CHECK (char_length(btrim(title)) BETWEEN 1 AND 120),
    CONSTRAINT jam_entries_something CHECK (url IS NOT NULL OR storage_name IS NOT NULL),
    CONSTRAINT jam_entries_url_format CHECK (url IS NULL OR url ~ '^https?://'),
    CONSTRAINT jam_entries_size_max CHECK (size_bytes IS NULL OR size_bytes <= 524288000)
);
CREATE INDEX jam_entries_jam_idx ON jam_entries (jam_id, submitted_at);

CREATE TABLE jam_votes (
    category_id  UUID NOT NULL REFERENCES jam_categories(id) ON DELETE CASCADE,
    team_id      UUID NOT NULL REFERENCES jam_teams(id) ON DELETE CASCADE,
    voter_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    jam_id       UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    score        SMALLINT NOT NULL,
    jury         BOOLEAN NOT NULL,
    voted_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (category_id, team_id, voter_id),

    CONSTRAINT jam_votes_score CHECK (score BETWEEN 1 AND 5)
);
CREATE INDEX jam_votes_jam_idx ON jam_votes (jam_id);

-- The winner of each category, fixed when the results are published.
CREATE TABLE jam_awards (
    category_id  UUID PRIMARY KEY REFERENCES jam_categories(id) ON DELETE CASCADE,
    jam_id       UUID NOT NULL REFERENCES game_jams(id) ON DELETE CASCADE,
    team_id      UUID NOT NULL REFERENCES jam_teams(id) ON DELETE CASCADE,
    score        REAL NOT NULL
);
