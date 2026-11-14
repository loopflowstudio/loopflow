INSERT INTO provider_accounts(provider, account_id, credential_state, routing_state, created_at, updated_at)
VALUES ('codex','one','connected','automatic',1,1), ('codex','two','connected','automatic',1,1);
INSERT INTO provider_routes VALUES
('repo','missing/clone','codex',0,'one',1,1), ('repo','missing/clone','codex',1,'two',1,1);

INSERT INTO pools(name,off,created_at) VALUES('workers',1,1);
INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('code','daytona','{"vcpus":2,"memory_gb":4,"disk_gb":20,"gpu":null,"spot":false}',3,'1.50',4,1);
INSERT INTO access_profiles(profile_id,chrome_directory,expected_login,created_at,updated_at)
VALUES('Work','Profile 1',NULL,1,1);
INSERT INTO auth_browser_bindings(provider,account_id,position,profile_id) VALUES('codex','one',0,'Work');
