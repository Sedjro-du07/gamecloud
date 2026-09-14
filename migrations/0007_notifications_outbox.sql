-- 0007_notifications_outbox.sql
-- Cross-process notification queue.
--
-- The Axum web binary writes rows here when it wants the Discord bot
-- to push an embed (e.g. project Released, rank-up, badge earned).
-- The bot polls this table, processes rows transactionally, and either
-- marks them `Sent` or increments `attempts` on failure.
--
-- This decouples the two binaries' uptime: a temporary bot outage does
-- not lose notifications, and an Axum redeploy does not flush an
-- in-memory queue.

CREATE TABLE notifications_outbox (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID REFERENCES users(id) ON DELETE CASCADE,
    -- Free-form discriminator the bot pattern-matches on.
    -- Examples: 'ProjectReleased', 'RankUp', 'BadgeEarned', 'WeeklyReport'.
    kind          TEXT NOT NULL,
    -- Structured payload; the bot parses it according to `kind`.
    payload       JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- 'Pending', 'Sent', 'Failed'. We retry up to 5 times then mark Failed.
    status        TEXT NOT NULL DEFAULT 'Pending',
    attempts      INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at       TIMESTAMPTZ,

    CONSTRAINT notifications_outbox_status_valid CHECK (
        status IN ('Pending', 'Sent', 'Failed')
    ),
    CONSTRAINT notifications_outbox_attempts_nonneg CHECK (attempts >= 0),
    CONSTRAINT notifications_outbox_sent_consistency CHECK (
        (status = 'Sent' AND sent_at IS NOT NULL)
        OR (status <> 'Sent' AND sent_at IS NULL)
    )
);

-- The bot polls this index every few seconds.
CREATE INDEX notifications_outbox_pending_idx
    ON notifications_outbox (created_at)
    WHERE status = 'Pending';

CREATE INDEX notifications_outbox_user_idx ON notifications_outbox (user_id)
    WHERE user_id IS NOT NULL;
