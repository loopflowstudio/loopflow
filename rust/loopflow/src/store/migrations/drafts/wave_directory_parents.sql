-- depends_on: session_ownership

DROP INDEX idx_waves_active_locator;

-- Expand old full-path names before replacing them with leaf names. Existing
-- IDs, including Projects' Wave references, survive the conversion.
CREATE TEMP TABLE wave_directory_paths AS
WITH RECURSIVE paths(repo, path, rest, name, parent_path, active, source_id) AS (
    SELECT repo, '', name || '/', '', NULL, retired_at IS NULL, id FROM waves
    UNION
    SELECT repo,
           CASE WHEN path='' THEN substr(rest,1,instr(rest,'/')-1)
                ELSE path || '/' || substr(rest,1,instr(rest,'/')-1) END,
           substr(rest,instr(rest,'/')+1), substr(rest,1,instr(rest,'/')-1),
           NULLIF(path,''), active, source_id
    FROM paths WHERE rest != ''
)
SELECT repo,path,name,parent_path,max(active) AS active,min(source_id) AS source_id
FROM paths WHERE path != '' GROUP BY repo,path,name,parent_path;

-- An active descendant needs an active directory ancestor even when an older
-- registration at that address is retired. Historical-only paths stay retired.
INSERT INTO waves(id,name,repo,created_at,retired_at,superseded_by_wave_id,retirement_reason)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(6))), p.path,p.repo,unixepoch(),
       CASE WHEN p.active=0 THEN source.retired_at END,
       CASE WHEN p.active=0 THEN source.superseded_by_wave_id END,
       CASE WHEN p.active=0 THEN 'Directory ancestor of retired Wave ' || source.id END
FROM wave_directory_paths p JOIN waves source ON source.id=p.source_id
WHERE NOT EXISTS(SELECT 1 FROM waves w WHERE w.repo=p.repo AND w.name=p.path
    AND (p.active=0 OR w.retired_at IS NULL));

-- Old parent links described promotion ancestry. Directory paths now determine
-- parentage; promoted_at remains untouched as historical promotion evidence.
UPDATE waves AS child SET parent_wave_id=(
    SELECT parent.id FROM wave_directory_paths p JOIN waves parent
    ON parent.repo=p.repo AND parent.name=p.parent_path
    WHERE p.repo=child.repo AND p.path=child.name
    ORDER BY parent.retired_at IS NOT NULL, parent.created_at DESC, parent.id LIMIT 1
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
