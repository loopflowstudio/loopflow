"""Final orchestration over the unchanged, sealed measurement functions."""
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import time

from iteration2 import c, configure, WORKLOADS
from prepare_iteration2 import tables

ROOT = Path('/tmp/loo822-final-measure')
COMMITS = {'before': '00851d9c6', 'after': 'b8fd92ab6b'}
c.ROOT, c.HOME, c.BIN = ROOT, ROOT / 'home', ROOT / 'lf'
HARNESS = ('iteration2.py', 'compare.py', 'planning_compare.py', 'run.py', 'freeze_clock.c')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build():
    ROOT.mkdir(mode=0o700)
    c.HOME.mkdir()
    # The only real-store access is a read-only SQLite online backup. All lf
    # invocations, migrations, profiles and measurements use this private copy.
    with closing(sqlite3.connect(f'file:{Path.home()}/.lf/loopflow.db?mode=ro', uri=True)) as src:
        with closing(sqlite3.connect(ROOT / 'frozen.db')) as dst:
            src.backup(dst)
            dst.execute('PRAGMA wal_checkpoint(TRUNCATE)')
            dst.execute('PRAGMA journal_mode=DELETE')
    for name in ('freeze_clock.dylib', 'migration-order.txt'):
        shutil.copy2(Path('/tmp/loo822-process-measure') / name, ROOT / name)
    subprocess.run(['cp', '-cR', '/tmp/loo822-process-measure/providers', str(ROOT / 'providers')], check=True)
    (ROOT / 'epoch').write_text(str(int(time.time())))
    manifest = dict(commits=COMMITS, claim='The kept Session-list, Wave-list and Task-status latency gains survive compress and sync onto main, with full JSON unchanged except intentional changes from main.',
                    harness_sha256={n: sha(Path(__file__).with_name(n)) for n in HARNESS},
                    protected_loo822_sha256=sha(c.REPO / 'scratch/loo-822.md'),
                    frozen_sha256=sha(ROOT / 'frozen.db'), epoch=(ROOT / 'epoch').read_text(),
                    providers='Unchanged credential-free private transcript copies from iteration 2',
                    plan=dict(workloads=WORKLOADS, pairs=5), builds={})
    with closing(sqlite3.connect(ROOT / 'frozen.db')) as db:
        manifest['counts'] = {t: db.execute(f'SELECT count(*) FROM {t}').fetchone()[0] for t in ('agent_sessions','session_events','session_activity','processes','tasks','provider_accounts')}
        manifest['source_optimization_indexes'] = db.execute("SELECT name FROM sqlite_master WHERE type='index' AND name IN ('tasks_checkout_root','tasks_primary_session','agent_process_unfinished','processes_cwd')").fetchall()
    manifest['environment'] = {key: subprocess.check_output(cmd, text=True).strip() for key, cmd in {'rust':['rustc','--version'], 'os':['sw_vers','-productVersion'], 'arch':['uname','-m']}.items()}
    (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    env = {k:v for k,v in os.environ.items() if not k.startswith('LF_')}
    env.update(CARGO_TARGET_DIR=str(c.REPO / 'target'), LOOPFLOW_MIGRATION_ORDER=str(ROOT / 'migration-order.txt'), LOOPFLOW_BUILD_PROVENANCE='development')
    for side, commit in COMMITS.items():
        source = ROOT / ('source-' + side)
        source.mkdir()
        with (ROOT / 'source.tar').open('wb') as out:
            subprocess.run(['git','archive',commit], cwd=c.REPO, stdout=out, check=True)
        subprocess.run(['tar','-xf',str(ROOT / 'source.tar'),'-C',str(source)], check=True)
        print('Building ' + side, flush=True)
        with (ROOT / (side + '-build.log')).open('w') as log:
            subprocess.run(['cargo','build','--release','-p','loopflow','--bin','lf'], cwd=source, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, check=True)
        shutil.copy2(c.REPO / 'target/release/lf', ROOT / side)
        manifest['builds'][side] = dict(commit=commit, sha256=sha(ROOT / side))
        (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
        print('Built ' + side, flush=True)


def prepare():
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    assert not manifest['source_optimization_indexes'], 'Fresh store already has optimization indexes; prepare a faithful baseline before timing'
    for side in COMMITS:
        subprocess.run(['cp','-c',str(ROOT / 'frozen.db'),str(ROOT / (side + '.db'))], check=True)
        c.activate(side)
        configure('easy')
        ms, data, _ = c.cli('easy', c.environment())
        print(json.dumps(dict(side=side, ms=ms, **c.validate('easy',data))), flush=True)
        with closing(sqlite3.connect(c.HOME / 'loopflow.db')) as src:
            with closing(sqlite3.connect(ROOT / (side + '.db'))) as dst:
                src.backup(dst)
                dst.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                dst.execute('PRAGMA journal_mode=DELETE')
        assert not (ROOT / (side + '.db-wal')).exists()
    digests = {side:tables(ROOT / (side + '.db')) for side in COMMITS}
    changed = [name for name in digests['before'] if digests['before'][name] != digests['after'][name]]
    assert set(changed) <= {'schema_migrations'}, changed
    manifest['store_difference_tables'] = changed
    manifest['store_sha256'] = {side:sha(ROOT / (side + '.db')) for side in COMMITS}
    (ROOT / 'store-digests.json').write_text(json.dumps(digests, indent=2))
    (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))


def run(mode):
    for workload in (('easy','cli','cli','easy') if mode == 'calibrate' else WORKLOADS):
        configure(workload)
        for pair in range(5 if mode == 'compare' else 1):
            for side in (tuple(COMMITS) if pair % 2 == 0 else tuple(COMMITS)[::-1]):
                row = c.run(side, workload, f'{mode}-{workload}-{pair}-{side}-{time.monotonic_ns()}')
                assert row['errors'] == 0, row


if __name__ == '__main__':
    mode = sys.argv[1]
    if mode == 'build': build()
    elif mode == 'prepare': prepare()
    else: run(mode)
