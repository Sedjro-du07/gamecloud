-- 0011_engagement.sql
--
-- Supporting tables for the three engagement systems that had schema
-- but no mechanism: resource voting, quest progress, and badges.

-- ---------------------------------------------------------------------------
-- Resource votes
-- ---------------------------------------------------------------------------
-- `resources.votes` was a bare counter, so nothing stopped one member
-- from voting a thousand times. Votes now live in their own table and
-- the counter is a cached aggregate maintained by a trigger.

CREATE TABLE IF NOT EXISTS resource_votes (
    resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id)     ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (resource_id, user_id)
);

CREATE INDEX IF NOT EXISTS resource_votes_user_idx ON resource_votes (user_id);

CREATE OR REPLACE FUNCTION trg_sync_resource_votes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE resources SET votes = votes + 1 WHERE id = NEW.resource_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE resources SET votes = GREATEST(votes - 1, 0) WHERE id = OLD.resource_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS resource_votes_sync ON resource_votes;
CREATE TRIGGER resource_votes_sync
    AFTER INSERT OR DELETE ON resource_votes
    FOR EACH ROW EXECUTE FUNCTION trg_sync_resource_votes();

-- ---------------------------------------------------------------------------
-- Quest progress
-- ---------------------------------------------------------------------------
-- `quest_completions` recorded the finish line but not the run-up, so
-- a member could never see "2 / 3 pushes done". Progress is counted
-- incrementally as XP events arrive.

CREATE TABLE IF NOT EXISTS quest_progress (
    quest_id   UUID NOT NULL REFERENCES quests(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    counter    INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (quest_id, user_id),

    CONSTRAINT quest_progress_counter_nonneg CHECK (counter >= 0)
);

CREATE INDEX IF NOT EXISTS quest_progress_user_idx ON quest_progress (user_id);

-- ---------------------------------------------------------------------------
-- Badges
-- ---------------------------------------------------------------------------
-- `special_badges` already has a unique (user_id, badge_type) index, so
-- awarding is naturally idempotent via ON CONFLICT DO NOTHING. All we
-- add is a lookup index for the "who has this badge" direction.

CREATE INDEX IF NOT EXISTS special_badges_type_idx ON special_badges (badge_type);

-- ---------------------------------------------------------------------------
-- Audit log ergonomics
-- ---------------------------------------------------------------------------
-- `audit_logs.actor_id` was NOT NULL with ON DELETE RESTRICT, which
-- makes system-originated entries (webhook grants, quest awards)
-- impossible to record. Allow a NULL actor meaning "the platform".

ALTER TABLE audit_logs
    ALTER COLUMN actor_id DROP NOT NULL;

COMMENT ON COLUMN audit_logs.actor_id IS
    'NULL means the action was taken by the platform itself (webhook, scheduler) rather than a member.';
