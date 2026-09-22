-- A member may hold more than one Bureau office.
--
-- `bureau_role` was a single value, so an association where the same
-- person keeps the minutes *and* the accounts — ordinary in a small
-- club — could not be written down: appointing the second office
-- silently replaced the first.
--
-- `offices` is now the one list that is written, kept in protocol order
-- (President first). `bureau_role` survives as a generated column equal
-- to the first — the principal office — so everything that shows one
-- title keeps working unchanged, and nothing can write it out of step
-- with the list.

ALTER TABLE users ADD COLUMN IF NOT EXISTS offices TEXT[] NOT NULL DEFAULT '{}';

-- Only values the platform knows are carried over. Nothing ever
-- constrained the old column, so an unexpected string there must not be
-- allowed to fail the CHECK below and stop the deploy.
UPDATE users SET offices = ARRAY[bureau_role]
 WHERE offices = '{}'
   AND bureau_role IN (
        'President', 'VicePresident', 'Secretary', 'Treasurer', 'VpTech',
        'VpCommunity', 'EventManager', 'AssistantEventManager', 'Archiviste',
        'AssistantArchiviste', 'CommunityManager', 'SocialMediaManager',
        'Moderator', 'AssistantModerator', 'RecruitmentOfficer', 'PrManager',
        'Provisional');

DROP INDEX IF EXISTS users_bureau_role_idx;
ALTER TABLE users DROP COLUMN bureau_role;
ALTER TABLE users ADD COLUMN bureau_role TEXT GENERATED ALWAYS AS (offices[1]) STORED;
CREATE INDEX users_bureau_role_idx ON users (bureau_role) WHERE bureau_role IS NOT NULL;
CREATE INDEX IF NOT EXISTS users_offices_idx ON users USING gin (offices);

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_offices_valid;
ALTER TABLE users ADD CONSTRAINT users_offices_valid CHECK (
    offices <@ ARRAY[
        'President', 'VicePresident', 'Secretary', 'Treasurer', 'VpTech',
        'VpCommunity', 'EventManager', 'AssistantEventManager', 'Archiviste',
        'AssistantArchiviste', 'CommunityManager', 'SocialMediaManager',
        'Moderator', 'AssistantModerator', 'RecruitmentOfficer', 'PrManager',
        'Provisional'
    ]::TEXT[]
);

COMMENT ON COLUMN users.offices IS
    'Bureau offices held, in protocol order. The only office column that is written.';
COMMENT ON COLUMN users.bureau_role IS
    'Generated: the principal office, offices[1]. Read-only.';
