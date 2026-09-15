-- 0021_kumo_relay.sql
-- Messages the bot relayed to Kumo, so Kumo's reply finds its way back.
--
-- A member writes to the GameCloud OS bot in private; the bot posts the
-- message in the relay channel where Kumo reads; when Kumo answers, the
-- bot looks up who asked and sends the answer to them in private.

CREATE TABLE kumo_relays (
    -- The bot's message in the relay channel.
    relay_message_id      TEXT PRIMARY KEY,
    -- Who wrote to the bot.
    requester_discord_id  TEXT NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX kumo_relays_recent_idx ON kumo_relays (created_at DESC);
