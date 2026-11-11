"""Iteration 3 orchestration; iteration-2 measurement functions stay sealed."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from iteration2 import c, configure
from prepare_iteration2 import tables

ROOT = Path('/tmp/loo822-reads-measure')
PREVIOUS = Path('/tmp/loo822-process-measure')
COMMITS = {'before': '0f39f7be21', 'after1': 'd1f1caa3f7', 'after2': 'a835f03e6f'}
WORKLOADS = ('task', 'cli', 'waves', 'board')
c.ROOT = ROOT
c.HOME = ROOT / 'home'
c.BIN = ROOT / 'lf'


def _sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build():
    ROOT.mkdir(mode=0o700)
    c.HOME.mkdir()
    old = json.loads((PREVIOUS / 'manifest.json').read_text())
    harness = {name: _sha(Path(__file__).with_name(name)) for name in old['harness_sha256']}
    assert harness == old['harness_sha256']
    assert _sha(PREVIOUS / 'after.db') == old['store_sha256']['after']
    for name in ('freeze_clock.dylib', 'epoch', 'migration-order.txt'):
        shutil.copy2(PREVIOUS / name, ROOT / name)
    subprocess.run(['cp', '-cR', str(PREVIOUS / 'providers'), str(ROOT / 'providers')], check=True)
    source = ROOT / 'source'
    source.mkdir()
    manifest = dict(commits=COMMITS, harness_sha256=harness, epoch=(ROOT / 'epoch').read_text(),
                    source_store_sha256=old['store_sha256']['after'], counts=old['counts'],
                    environment=old['environment'], builds={}, store_sha256={},
                    protected_loo822_sha256=_sha(c.REPO / 'scratch/loo-822.md'),
                    claim='Each repeated-read removal reduces network-denied Task-status latency beyond noise with identical full JSON.',
                    plan=dict(workloads=WORKLOADS, pairs=5, comparisons=[['before','after1'],['after1','after2']]))
    env = {k: v for k, v in os.environ.items() if not k.startswith('LF_')}
    env.update(CARGO_TARGET_DIR=str(c.REPO / 'target'),
               LOOPFLOW_MIGRATION_ORDER=str(ROOT / 'migration-order.txt'),
               LOOPFLOW_BUILD_PROVENANCE='development')
    for side, commit in COMMITS.items():
        subprocess.run(['cp', '-c', str(PREVIOUS / 'after.db'), str(ROOT / (side + '.db'))], check=True)
        manifest['store_sha256'][side] = _sha(ROOT / (side + '.db'))
        with (ROOT / 'source.tar').open('wb') as archive:
            subprocess.run(['git', 'archive', commit], cwd=c.REPO, stdout=archive, check=True)
        subprocess.run(['tar', '-xf', str(ROOT / 'source.tar'), '-C', str(source)], check=True)
        print('Building ' + side, flush=True)
        with (ROOT / (side + '-build.log')).open('w') as log:
            subprocess.run(['cargo','build','--release','-p','loopflow','--bin','lf'], cwd=source,
                           env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, check=True)
        shutil.copy2(c.REPO / 'target/release/lf', ROOT / side)
        manifest['builds'][side] = dict(commit=commit, sha256=_sha(ROOT / side))
        (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
        print('Built ' + side, flush=True)
    digests = {side: tables(ROOT / (side + '.db')) for side in COMMITS}
    assert digests['before'] == digests['after1'] == digests['after2']
    (ROOT / 'store-digests.json').write_text(json.dumps(digests, indent=2))


def calibration():
    for pair in range(2):
        for workload in (('easy', 'cli') if pair == 0 else ('cli', 'easy')):
            configure(workload)
            row = c.run('before', workload, f'calibration-{pair}-{workload}')
            assert row['errors'] == 0


def compare(sides):
    name = '-'.join(sides)
    for workload in WORKLOADS:
        configure(workload)
        expected = None
        for pair in range(5):
            for side in (sides if pair % 2 == 0 else sides[::-1]):
                row = c.run(side, workload, f'iteration3-{name}-{workload}-{pair}-{side}')
                assert row['errors'] == 0, row
                expected = expected or row['full_digest']
                assert row['full_digest'] == expected, row['label']
                assert row['warmup']['full_digest'] == expected, row['label']


if __name__ == '__main__':
    if sys.argv[1] == 'build':
        build()
    elif sys.argv[1] == 'calibrate':
        calibration()
    else:
        compare(tuple(sys.argv[1:3]))
