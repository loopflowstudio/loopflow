"""Validate frozen inputs and retain the complete iteration-3 trial ledger."""
import hashlib
import json
from pathlib import Path
import statistics

from iteration3 import ROOT, COMMITS, WORKLOADS, c

OUT = Path(__file__).parent


def summarize():
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    for name, expected in manifest['harness_sha256'].items():
        assert hashlib.sha256((OUT / name).read_bytes()).hexdigest() == expected
    assert hashlib.sha256((c.REPO / 'scratch/loo-822.md').read_bytes()).hexdigest() == manifest['protected_loo822_sha256']
    trials = [json.loads(line) for line in (ROOT / 'trials.jsonl').read_text().splitlines()]
    cohorts = {}
    for sides in (('before', 'after1'), ('after1', 'after2')):
        comparison = '-'.join(sides)
        for workload in WORKLOADS:
            group = {}
            for side in sides:
                rows = [r for r in trials if r['label'].startswith(f'iteration3-{comparison}-') and r['side'] == side and r['workload'] == workload]
                assert len(rows) == 5 and all(r['errors'] == 0 for r in rows)
                assert len({r['full_digest'] for r in rows}) == 1
                assert all(r['warmup']['full_digest'] == r['full_digest'] for r in rows)
                values = [r['ms'] for r in rows]
                group[side] = dict(n=5, median_ms=statistics.median(values), range_ms=[min(values),max(values)], rows=rows[0]['rows'], full_digest=rows[0]['full_digest'], errors=0)
            b, a = [group[s] for s in sides]
            assert b['full_digest'] == a['full_digest']
            verdict = ('better' if a['range_ms'][1] < b['range_ms'][0] else
                       'worse' if b['range_ms'][1] < a['range_ms'][0] else 'no measurable difference')
            cohorts[f'{comparison}-{workload}'] = dict(sides=group, verdict=verdict, reduction_pct=100*(1-a['median_ms']/b['median_ms']))
            print(comparison, workload, verdict, {s: [round(v['median_ms'],1),[round(n,1) for n in v['range_ms']]] for s,v in group.items()})
    counts = [json.loads(line) for line in (ROOT / 'counts.jsonl').read_text().splitlines()] if (ROOT / 'counts.jsonl').exists() else []
    samples = [json.loads(p.read_text()) for p in ROOT.glob('sample-*-receipt.json')]
    attributions = [json.loads(line) for line in (ROOT / 'attribution.jsonl').read_text().splitlines()] if (ROOT / 'attribution.jsonl').exists() else []
    for row in [*counts, *samples, *attributions]:
        expected = cohorts['before-after1-' + row['workload']]['sides']['before']['full_digest']
        assert row['full_digest'] == row['warmup']['full_digest'] == expected
    result = dict(manifest=manifest, cohorts=cohorts, trials=trials, counts=counts, samples=samples, attributions=attributions)
    (OUT / 'iteration3-results.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    summarize()
