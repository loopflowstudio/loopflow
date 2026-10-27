-- 0.14.17 Fly rows, one rental ended and one still running, beside a Daytona on-demand.
INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('fly1','fly','{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}',2,'0.40',4.0,1);
INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at)
VALUES('rent_0f','fly1','fly','lf-rent-0f',0,'{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}','0.19','0.76',2,'ended',1);
INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at)
VALUES('rent_1a','fly1','fly','lf-rent-1a',0,'{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}','0.19','0.76',2,'running',1);
INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('code','daytona','{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}',1,'0.40',4.0,1);
INSERT INTO scope_defaults(scope_kind,scope_id,tasks_capacity,set_at)
VALUES('wave','infrastructure','fly1',1);
