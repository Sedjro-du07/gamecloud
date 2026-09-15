-- 0023_kumo_chat_answered_by.sql
-- The Bureau may answer in Kumo's place.
--
-- In the relay channel, a Bureau member who replies to a relayed message
-- has that reply delivered like one of Kumo's. The conversation says who
-- actually answered: NULL for Kumo, otherwise the Bureau member's name.

ALTER TABLE kumo_messages ADD COLUMN answered_by TEXT;
