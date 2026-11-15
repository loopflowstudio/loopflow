-- Unassignment appends to the ownership log; it never removes its earlier claim.
INSERT INTO task_ownership(id,task_id,kind,machine_id,writer_machine_id,at,ordered_at,written_here)
SELECT 'own_66666666666666666666666666666666','task_22222222222222222222222222222222',
       'unclaim',id,id,2,2000,1 FROM machines WHERE is_local=1;
