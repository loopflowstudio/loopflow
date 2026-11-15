-- File cutover is prepared under the migration fence before this SQL runs.
-- settings-cutover.json retains before/after documents for interruption recovery.
ALTER TABLE tasks DROP COLUMN agent;
DROP TABLE scope_settings;
DROP TABLE scope_defaults;
DROP TABLE provider_routes;
DROP TABLE auth_browser_bindings;

-- Local installation identity and observed machine lifetime remain runtime facts.
ALTER TABLE machines ADD COLUMN is_local INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0,1));
UPDATE machines SET is_local=1 WHERE route='local';
DROP INDEX idx_machines_route;
DROP INDEX idx_machines_label;
ALTER TABLE machines DROP COLUMN route;
ALTER TABLE machines DROP COLUMN label;
CREATE UNIQUE INDEX idx_machines_local ON machines(is_local) WHERE is_local=1;

DROP INDEX idx_provider_accounts_login_email;
ALTER TABLE provider_accounts DROP COLUMN login_email;
ALTER TABLE provider_accounts DROP COLUMN routing_state;
ALTER TABLE provider_accounts DROP COLUMN plan;
ALTER TABLE provider_accounts DROP COLUMN paid_through;

-- Capacity declarations move to files; creation receipts and rental history remain.
ALTER TABLE pools DROP COLUMN off;
ALTER TABLE ondemands DROP COLUMN provider;
ALTER TABLE ondemands DROP COLUMN template;
ALTER TABLE ondemands DROP COLUMN max_machines;
ALTER TABLE ondemands DROP COLUMN max_usd_per_hour;
ALTER TABLE ondemands DROP COLUMN hours;
