-- name: record_conversation_connection
-- id: 87b194c675ad48d5af917c2728b3349f
-- depends_on: conversation_mode

-- The provider connection survives replacement of its conversational driver.
-- Endpoint ownership is the provider generation, not an lf process lifetime.
ALTER TABLE sessions ADD COLUMN provider_endpoint TEXT;
ALTER TABLE sessions ADD COLUMN provider_thread TEXT;
