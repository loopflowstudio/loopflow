INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('code','daytona','{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null}',1,'0.40',4.0,1);
INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at)
VALUES('rent_0b5c','code','daytona','lf-rent_0b5c',0,'{"vcpus":4,"memory_gb":8,"disk_gb":10}','0.33','1.32',2,'ended',1);
