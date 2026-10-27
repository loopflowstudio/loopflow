INSERT INTO processes(id,trace_id,started_at,completed_at)
VALUES('dddddddd-dddd-4ddd-8ddd-dddddddddddd','eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',1,2);
INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES('dddddddd-dddd-4ddd-8ddd-dddddddddddd','review',
'{"name":"review","steps":[{"key":0,"id":null,"label":"implement","kind":"skill","human":false,"returns_to":null,"sources":["review"],"paths":[]},{"key":1,"id":null,"label":"route","kind":"xor","human":false,"returns_to":null,"sources":["review"],"paths":[{"name":"ask","description":"Quote: ,\"human\":true","steps":[{"key":2,"id":null,"label":"design","kind":"skill","human":true,"returns_to":null,"sources":["review"],"paths":[]}]}]}],"interactions":{"stages":[],"transitions":[]}}');
