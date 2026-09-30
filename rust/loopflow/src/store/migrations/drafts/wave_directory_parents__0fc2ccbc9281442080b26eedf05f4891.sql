-- name: wave_directory_parents
-- id: 0fc2ccbc9281442080b26eedf05f4891
-- depends_on: session_ownership

DROP INDEX idx_waves_active_locator;

-- Expand old full-path names before replacing them with leaf names. Existing
-- IDs, including Projects' Wave references, survive the conversion.
CREATE TEMP TABLE wave_directory_paths AS
WITH RECURSIVE paths(repo, path, rest, name, parent_path) AS (
    SELECT repo, '', name || '/', '', NULL FROM waves
    UNION
    SELECT repo,
           CASE WHEN path='' THEN substr(rest,1,instr(rest,'/')-1)
                ELSE path || '/' || substr(rest,1,instr(rest,'/')-1) END,
           substr(rest,instr(rest,'/')+1), substr(rest,1,instr(rest,'/')-1),
           NULLIF(path,'')
    FROM paths WHERE rest != ''
)
SELECT DISTINCT repo,path,name,parent_path FROM paths WHERE path != '';

INSERT INTO waves(id,name,repo,created_at)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(6))), p.path,p.repo,unixepoch()
FROM wave_directory_paths p
WHERE NOT EXISTS(SELECT 1 FROM waves w WHERE w.repo=p.repo AND w.name=p.path);

-- Old parent links described promotion ancestry. Directory paths now determine
-- parentage; promoted_at remains untouched as historical promotion evidence.
UPDATE waves AS child SET parent_wave_id=(
    SELECT parent.id FROM wave_directory_paths p JOIN waves parent
    ON parent.repo=p.repo AND parent.name=p.parent_path AND parent.retired_at IS NULL
    WHERE p.repo=child.repo AND p.path=child.name
);

UPDATE waves SET name=(SELECT p.name FROM wave_directory_paths p
    WHERE p.repo=waves.repo AND p.path=waves.name);
DROP TABLE wave_directory_paths;

CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(parent_wave_id,''),name) WHERE retired_at IS NULL;

CREATE VIEW wave_addresses AS
WITH RECURSIVE addresses(id,slug) AS (
    SELECT id,name FROM waves WHERE parent_wave_id IS NULL
    UNION ALL
    SELECT w.id,a.slug || '/' || w.name FROM waves w
    JOIN addresses a ON w.parent_wave_id=a.id
)
SELECT w.*,a.slug FROM waves w JOIN addresses a ON a.id=w.id;
