"""Direct baseline/final comparison and incremental scoped-read attribution."""
import hashlib
import json
from pathlib import Path
from compare import ROOT, run

PLAN = [
    ('combined', ('before', 'after2'), ('cli', 'served')),
    ('planning-increment', ('after1', 'after2'), ('board', 'selection')),
]
if __name__ == '__main__':
    original=json.loads((ROOT/'manifest.json').read_text())
    for name, expected in original['harness_sha256'].items():
        assert hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()==expected
    (ROOT/'followup-plan.json').write_text(json.dumps(dict(plan=PLAN,script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2))
    for name,sides,workloads in PLAN:
        for workload in workloads:
            expected=None
            for pair in range(5):
                for side in (sides if pair%2==0 else sides[::-1]):
                    row=run(side,workload,f'{name}-{workload}-{pair}-{side}')
                    assert row['errors']==0,row
                    if expected is None:expected=row['full_digest']
                    assert row['full_digest']==expected,f'output mismatch: {row["label"]}'
                    assert row['warmup']['full_digest']==expected,f'warmup mismatch: {row["label"]}'
