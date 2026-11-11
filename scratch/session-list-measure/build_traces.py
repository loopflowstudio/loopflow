"""Build historical attribution binaries from private source exports, not worktrees."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path('/tmp/loo822-compare')
REPO=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'trace-source'
COMMITS={'before':'00851d9c6','after1':'d9e3c1f3b','after2':'f8b742d8c'}
paths=json.loads((ROOT/'source-paths.json').read_text())
perf='rust/loopflow/src/performance.rs'
# Exported builds accept the same merged migration order as this checkout.
catalog=next((REPO/'target/release/build').glob('loopflow-*/out/migrations.rs')).read_text()
order=re.findall(r'version: "([^"]+)".*merged: true',catalog)
assert order
(ROOT/'migration-order.txt').write_text('\n'.join(order)+'\n')
SOURCE.mkdir(exist_ok=True)
with (ROOT/'trace-source.tar').open('wb') as archive:
    subprocess.run(['git','archive','f8b742d8c'],cwd=REPO,stdout=archive,check=True)
subprocess.run(['tar','-xf',str(ROOT/'trace-source.tar'),'-C',str(SOURCE)],check=True)
original=(SOURCE/perf).read_text()
env={k:v for k,v in os.environ.items() if not k.startswith('LF_')}
env.update(CARGO_TARGET_DIR=str(REPO/'target'),LOOPFLOW_MIGRATION_ORDER=str(ROOT/'migration-order.txt'),LOOPFLOW_BUILD_PROVENANCE='development')
manifest={}
for side,commit in COMMITS.items():
    for p in paths:
        result=subprocess.run(['git','show',commit+':'+p],cwd=REPO,capture_output=True)
        if result.returncode==0: (SOURCE/p).write_bytes(result.stdout)
        else:
            assert p.endswith('/session_list_indexes.sql') and side=='before'
            (SOURCE/p).unlink(missing_ok=True)
    source=original.replace('pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {','pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {\n    session_list_trace(connection);')
    (SOURCE/perf).write_text(source+'\n'+(REPO/'scratch/session-list-measure/trace_bound.rs').read_text())
    with (ROOT/(side+'-trace-build.log')).open('w') as log:
        subprocess.run(['cargo','build','--release','-p','loopflow','--bin','lf'],cwd=SOURCE,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,check=True)
    dest=ROOT/(side+'-trace');shutil.copy2(REPO/'target/release/lf',dest)
    manifest[side]=dict(commit=commit,sha256=hashlib.sha256(dest.read_bytes()).hexdigest())
    print(json.dumps(manifest[side]),flush=True)
    (ROOT/'trace-manifest.json').write_text(json.dumps(manifest,indent=2))
