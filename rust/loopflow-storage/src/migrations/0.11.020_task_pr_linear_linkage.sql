-- Persist the Linear linkage a published PR carries on its owning Task issue so
-- repeated `lf pr open/submit/land` update the same attachment and comment
-- instead of spamming duplicates.
--
-- linear_attachment_id: id of the first-class Linear attachment (attachmentLinkURL),
--   updated in place via attachmentUpdate on later publishes. NULL until linked.
-- linear_comment_id: the loopflow-managed comment; its presence switches the
--   writeback from commentCreate to commentUpdate. NULL until linked.
-- linear_link_error: NULL when the last writeback linked cleanly; the last error
--   string when Linear writeback degraded (the GitHub publication still succeeded).

ALTER TABLE task_prs ADD COLUMN linear_attachment_id TEXT;
ALTER TABLE task_prs ADD COLUMN linear_comment_id TEXT;
ALTER TABLE task_prs ADD COLUMN linear_link_error TEXT;
