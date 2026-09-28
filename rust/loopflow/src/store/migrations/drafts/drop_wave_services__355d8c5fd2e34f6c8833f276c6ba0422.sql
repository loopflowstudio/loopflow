-- name: drop_wave_services
-- id: 355d8c5fd2e34f6c8833f276c6ba0422
-- depends_on: one_flow_driver

DROP TABLE provider_deliveries;
DROP TABLE observation_outbox;

-- Keep supervisor identity and heartbeat evidence; recovery still requires the
-- existing exact process/lock checks. No daemon receives these claims anymore.
UPDATE pr_landings SET supervisor_placement='local', supervisor_home_id=NULL
WHERE supervisor_placement='home';
