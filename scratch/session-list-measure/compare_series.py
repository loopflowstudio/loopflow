"""Run a fixed alternating plan; refuse any full-answer mismatch."""
import hashlib
import json
from pathlib import Path
from compare import ROOT, run

PLAN = [
    ('task-probes', ('before', 'after1'), ('cli', 'served')),
    ('smaller-fixes', ('after1', 'after2'), ('cli', 'served')),
    ('planning', ('before', 'after2'), ('task', 'waves', 'board', 'selection')),
]

if __name__ == '__main__':
    source = Path(__file__).parent
    manifest = json.loads((ROOT/'manifest.json').read_text())
    manifest['harness_sha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [source/'compare.py', source/'compare_series.py', source/'freeze_clock.c', source/'run.py']}
    manifest['plan'] = PLAN
    (ROOT/'manifest.json').write_text(json.dumps(manifest, indent=2))
    for name, sides, workloads in PLAN:
        for workload in workloads:
            expected = None
            for pair in range(5):
                for side in (sides if pair%2==0 else sides[::-1]):
                    row = run(side, workload, f'{name}-{workload}-{pair}-{side}')
                    assert row['errors']==0, row
                    if expected is None: expected=row['full_digest']
                    assert row['full_digest']==expected, f'output mismatch: {row["label"]}'
                    assert row['warmup']['full_digest']==expected, f'warmup mismatch: {row["label"]}'
