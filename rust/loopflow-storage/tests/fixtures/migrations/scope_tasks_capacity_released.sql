-- 0.14.16 RunPod rows: a template naming an image, hardware as a tagged flavor.
INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('pods','runpod','{"gpu":null,"vcpus":8,"memory_gb":16,"disk_gb":null,"image":"runpod/base@sha256:ab","data_center":null}',1,'0.40',4.0,1);
INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at)
VALUES('rent_0a','pods','runpod','lf-rent_0a',0,'{"kind":"cpu","flavor":"cpu3c","vcpus":8}','0.24','0.96',2,'ended',1);
-- A RunPod on-demand in the current shape stays.
INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
VALUES('gpu','runpod','{"vcpus":8,"memory_gb":32,"disk_gb":40,"gpu":"L4"}',1,'1.00',4.0,1);
