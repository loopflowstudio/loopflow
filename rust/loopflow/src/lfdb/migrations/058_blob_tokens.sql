-- A git blob's content is immutable, so its line and token counts are too.
-- Walking a month of history re-reads the same blobs thousands of times; cache
-- them by sha and only ever tokenize a file version once.
--
-- Local, derived, and disposable: dropping this table costs time, not truth.
CREATE TABLE IF NOT EXISTS blob_tokens (
    sha TEXT PRIMARY KEY,
    lines BIGINT NOT NULL,
    bytes BIGINT NOT NULL,
    tokens BIGINT NOT NULL
);
