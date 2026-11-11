"""Keep complete JSON comparisons and timing receipts without private content."""
import json
from pathlib import Path
import statistics
import sys

from final_measure import ROOT, COMMITS, WORKLOADS, c, sha


def differences(a, b, path='$'):
    if type(a) is not type(b):
        return [path]
    if isinstance(a, dict):
        result = [path + '.' + k for k in sorted(a.keys() ^ b.keys())]
        for k in sorted(a.keys() & b.keys()):
            result.extend(differences(a[k], b[k], path + '.' + k))
        return result
    if isinstance(a, list):
        if len(a) != len(b):
            return [path + '.length']
        return [p for i, (left, right) in enumerate(zip(a,b)) for p in differences(left,right,f'{path}[{i}]')]
    return [] if a == b else [path]


def check(mode):
    rows = [json.loads(line) for line in (ROOT / 'trials.jsonl').read_text().splitlines()]
    selected = [r for r in rows if r['label'].startswith(mode + '-')]
    result = dict(mode=mode, trials=selected, comparisons={}, errors=sum(r['errors'] for r in selected))
    for workload in WORKLOADS:
        cohort = [r for r in selected if r['workload'] == workload]
        if not cohort:
            continue
        valid = [r for r in cohort if not r['errors']]
        comparison = dict(summary={}, full_json_differences=[], warmup_mismatches=[])
        reference = json.loads((ROOT / (valid[0]['label'] + '.json')).read_text())
        reference_digest = c.digest(reference)
        for side in COMMITS:
            side_rows = [r for r in valid if r['side'] == side]
            values = [r['ms'] for r in side_rows]
            comparison['summary'][side] = dict(n=len(values), median_ms=statistics.median(values) if values else None,
                range_ms=[min(values),max(values)] if values else None, rows=sorted({r['rows'] for r in side_rows}),
                full_digests=sorted({r['full_digest'] for r in side_rows}))
        for row in valid:
            data = json.loads((ROOT / (row['label'] + '.json')).read_text())
            diff = differences(reference,data)
            if diff:
                comparison['full_json_differences'].append(dict(label=row['label'], paths=diff))
            if row['warmup']['full_digest'] != reference_digest:
                comparison['warmup_mismatches'].append(row['label'])
        result['comparisons'][workload] = comparison
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    result['manifest'] = manifest
    result['sealed_checks'] = dict(
        harness_unchanged=all(sha(Path(__file__).with_name(name)) == value for name,value in manifest['harness_sha256'].items()),
        stores_unchanged=all(sha(ROOT / (side+'.db')) == value for side,value in manifest['store_sha256'].items()),
        frozen_unchanged=sha(ROOT / 'frozen.db') == manifest['frozen_sha256'],
        loo822_unchanged=sha(c.REPO / 'scratch/loo-822.md') == manifest['protected_loo822_sha256'])
    (ROOT / (mode + '-checks.json')).write_text(json.dumps(result,indent=2))
    print(json.dumps({k:v for k,v in result.items() if k not in ('trials','manifest')},indent=2))


if __name__ == '__main__': check(sys.argv[1])
