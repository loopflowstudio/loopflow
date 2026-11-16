-- 0.15.0 Waves: a live parent with a child, and a Wave retired with its successor.
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-live','product','/repo',1);
INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES('wave-child','desktop','/repo',2,'wave-live');
INSERT INTO waves(id,name,repo,created_at,retired_at,superseded_by_wave_id,retirement_reason,work_state)
VALUES('wave-old','product','/old',1,5,'wave-live','relocated','abandoned');
