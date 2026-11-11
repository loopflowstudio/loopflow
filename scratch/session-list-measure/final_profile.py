"""Excluded attribution; uses the same callbacks and native sampler as prior runs."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from final_measure import ROOT, COMMITS, c, configure, sha
import profile_iteration2 as sql
import profile_iteration3 as native

# Existing helpers import older roots; override their orchestration only.
sql.ROOT, sql.SIDES = ROOT, tuple(COMMITS)
native.ROOT = ROOT
c.ROOT, c.HOME, c.BIN = ROOT, ROOT / 'home', ROOT / 'lf'


def build():
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    env = {k:v for k,v in os.environ.items() if not k.startswith('LF_')}
    env.update(CARGO_TARGET_DIR=str(c.REPO / 'target'), LOOPFLOW_MIGRATION_ORDER=str(ROOT / 'migration-order.txt'), LOOPFLOW_BUILD_PROVENANCE='development')
    for side in COMMITS:
        source = ROOT / ('source-' + side)
        perf = source / 'rust/loopflow/src/performance.rs'
        text = perf.read_text()
        marker = 'pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {'
        assert text.count(marker) == 1 and 'fn session_list_trace(' not in text
        perf.write_text(text.replace(marker, marker + '\n    session_list_trace(connection);') + '\n' + Path(__file__).with_name('trace_bound.rs').read_text())
        label = side + '-trace'
        print('Building ' + label, flush=True)
        with (ROOT / (label + '-build.log')).open('w') as log:
            subprocess.run(['cargo','build','--release','-p','loopflow','--bin','lf'], cwd=source, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, check=True)
        shutil.copy2(c.REPO / 'target/release/lf', ROOT / label)
        manifest['builds'][label] = dict(commit=COMMITS[side], sha256=sha(ROOT / label), instrumentation='unchanged trace_bound.rs')
        (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
        print('Built ' + label, flush=True)


if __name__ == '__main__':
    if sys.argv[1] == 'build': build()
    elif sys.argv[1] == 'sql': sql.profiles()
    else:
        for side in COMMITS: native.sample(side)
