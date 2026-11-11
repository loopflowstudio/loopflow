"""Iteration 2 plan over the unchanged comparison harness."""
import hashlib
import json
from pathlib import Path
import compare as c
from planning_compare import cli as offline_cli

ROOT = Path('/tmp/loo822-process-measure')
SIDES = ('before', 'after')
WORKLOADS = ('waves', 'task', 'cli', 'served', 'board', 'selection')
NORMAL_CLI = c.cli
c.ROOT = ROOT
c.HOME = ROOT / 'home'
c.BIN = ROOT / 'lf'


def configure(workload):
    c.cli = NORMAL_CLI if workload in ('cli', 'easy', 'served') else offline_cli


if __name__ == '__main__':
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    manifest['harness_sha256'] = {
        name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
        for name in ('iteration2.py', 'compare.py', 'planning_compare.py', 'run.py', 'freeze_clock.c')
    }
    manifest['plan'] = dict(sides=SIDES, workloads=WORKLOADS, pairs=5)
    (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    for workload in WORKLOADS:
        configure(workload)
        expected = None
        for pair in range(5):
            for side in (SIDES if pair % 2 == 0 else SIDES[::-1]):
                row = c.run(side, workload, f'iteration2-{workload}-{pair}-{side}')
                assert row['errors'] == 0, row
                if expected is None:
                    expected = row['full_digest']
                assert row['full_digest'] == expected, f'output mismatch: {row["label"]}'
                assert row['warmup']['full_digest'] == expected, f'warmup mismatch: {row["label"]}'
