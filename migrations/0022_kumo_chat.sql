-- 0022_kumo_chat.sql
-- Talking to Kumo from the platform.
--
-- Somebody who shares no Discord server with the GameCloud OS bot cannot
-- write to it in private, so the platform has its own chat. It needs no
-- account: a conversation is found again by a random token kept in a
-- cookie (only its hash is stored), and tied to the member when they are
-- signed in. The bot relays each message to Kumo and files Kumo's answer
-- back into the conversation.

CREATE TABLE kumo_conversations (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- SHA-256 of the cookie token.
    token_hash       TEXT NOT NULL UNIQUE,
    user_id          UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_message_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX kumo_conversations_user_idx
    ON kumo_conversations (user_id, last_message_at DESC) WHERE user_id IS NOT NULL;

CREATE TABLE kumo_messages (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id  UUID NOT NULL REFERENCES kumo_conversations(id) ON DELETE CASCADE,
    -- FALSE: written on the platform. TRUE: Kumo's answer.
    from_kumo        BOOLEAN NOT NULL,
    body             TEXT NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- When the bot posted it for Kumo.
    relayed_at       TIMESTAMPTZ,
    relay_attempts   INTEGER NOT NULL DEFAULT 0,
    -- Given up after repeated failures.
    relay_failed     BOOLEAN NOT NULL DEFAULT FALSE,

    CONSTRAINT kumo_messages_body_length CHECK (length(body) BETWEEN 1 AND 4000)
);

CREATE INDEX kumo_messages_conversation_idx ON kumo_messages (conversation_id, created_at);
CREATE INDEX kumo_messages_pending_idx
    ON kumo_messages (created_at) WHERE NOT from_kumo AND relayed_at IS NULL AND NOT relay_failed;

-- A relayed message now came either from somebody on Discord or from a
-- platform conversation, never both.
ALTER TABLE kumo_relays
    ALTER COLUMN requester_discord_id DROP NOT NULL,
    ADD COLUMN conversation_id UUID REFERENCES kumo_conversations(id) ON DELETE CASCADE,
    ADD CONSTRAINT kumo_relays_one_requester
        CHECK ((requester_discord_id IS NULL) <> (conversation_id IS NULL));
