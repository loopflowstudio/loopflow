"""Build both commits and their separate attribution binaries from exact exports."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import time

REPO = Path(__file__).resolve().parents[2]
ROOT = Path('/tmp/loo822-process-measure')
SOURCE = ROOT / 'source'
COMMITS = {'before': '2667788c6', 'after': '761c28056'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ROOT.mkdir(mode=0o700)
    (ROOT / 'home').mkdir()
    SOURCE.mkdir()
    # Read-only online backup; every lf invocation uses the private copy.
    with sqlite3.connect(f'file:{Path.home()}/.lf/loopflow.db?mode=ro', uri=True) as src:
        with sqlite3.connect(ROOT / 'frozen.db') as dst:
            src.backup(dst)
    shutil.copy2('/tmp/loo822-compare/freeze_clock.dylib', ROOT / 'freeze_clock.dylib')
    shutil.copy2('/tmp/loo822-compare/migration-order.txt', ROOT / 'migration-order.txt')
    subprocess.run(['cp', '-cR', '/tmp/loo822-compare/providers', str(ROOT / 'providers')], check=True)
    (ROOT / 'epoch').write_text(str(int(time.time())))
    manifest = dict(commits=COMMITS, epoch=(ROOT / 'epoch').read_text(),
                    frozen_sha256=sha(ROOT / 'frozen.db'), builds={},
                    providers='Unchanged private transcript copies from iteration 1; no credentials copied')
    with sqlite3.connect(ROOT / 'frozen.db') as db:
        manifest['counts'] = {t: db.execute(f'SELECT count(*) FROM {t}').fetchone()[0]
                              for t in ('agent_sessions', 'session_events', 'session_activity', 'processes', 'tasks', 'provider_accounts')}
        manifest['draft_migrations'] = db.execute("SELECT * FROM schema_migrations WHERE version LIKE '%session_list%'").fetchall()
    env = {k: v for k, v in os.environ.items() if not k.startswith('LF_')}
    env.update(CARGO_TARGET_DIR=str(REPO / 'target'),
               LOOPFLOW_MIGRATION_ORDER=str(ROOT / 'migration-order.txt'),
               LOOPFLOW_BUILD_PROVENANCE='development')
    for trace in (False, True):
        for side, commit in COMMITS.items():
            with (ROOT / 'source.tar').open('wb') as archive:
                subprocess.run(['git', 'archive', commit], cwd=REPO, stdout=archive, check=True)
            subprocess.run(['tar', '-xf', str(ROOT / 'source.tar'), '-C', str(SOURCE)], check=True)
            label = side + ('-trace' if trace else '')
            if trace:
                perf = SOURCE / 'rust/loopflow/src/performance.rs'
                source = perf.read_text().replace(
                    'pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {',
                    'pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {\n    session_list_trace(connection);')
                perf.write_text(source + '\n' + (REPO / 'scratch/session-list-measure/trace_bound.rs').read_text())
            print('Building ' + label, flush=True)
            with (ROOT / (label + '-build.log')).open('w') as log:
                subprocess.run(['cargo', 'build', '--release', '-p', 'loopflow', '--bin', 'lf'],
                               cwd=SOURCE, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, check=True)
            shutil.copy2(REPO / 'target/release/lf', ROOT / label)
            manifest['builds'][label] = dict(commit=commit, sha256=sha(ROOT / label))
            (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
            print('Built ' + label, flush=True)


if __name__ == '__main__':
    main()
