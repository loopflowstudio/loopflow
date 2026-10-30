-- The release a bug Task regressed in (`v0.15.3`). Open regressions hold `stable`
-- below that release; the release that ships the fix is eligible after its soak.
ALTER TABLE tasks ADD COLUMN regressed_in TEXT;
