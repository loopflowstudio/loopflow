"""Prepare and verify private migrated stores before freezing the comparison."""
from contextlib import closing
import hashlib
import json
import sqlite3
import subprocess
from iteration2 import ROOT, SIDES, c, configure


def tables(path):
    with closing(sqlite3.connect(f'file:{path}?mode=ro&immutable=1', uri=True)) as db:
        result = {}
        for (name,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"):
            quoted = '"' + name.replace('"', '""') + '"'
            rows = sorted(repr(row) for row in db.execute('SELECT * FROM ' + quoted))
            result[name] = dict(rows=len(rows), digest=hashlib.sha256('\n'.join(rows).encode()).hexdigest())
        return result


if __name__ == '__main__':
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    preparations = []
    for side in SIDES:
        for suffix in ('', '-wal', '-shm'):
            (ROOT / (side + '.db' + suffix)).unlink(missing_ok=True)
        subprocess.run(['cp', '-c', str(ROOT / 'frozen.db'), str(ROOT / (side + '.db'))], check=True)
        c.activate(side)
        configure('easy')
        ms, data, _ = c.cli('easy', c.environment())
        row = dict(label='prepare-' + side, side=side, ms=ms, errors=0, **c.validate('easy', data))
        preparations.append(row)
        (ROOT / 'preparations.json').write_text(json.dumps(preparations, indent=2))
        print(json.dumps(row), flush=True)
        # Migrate outside timing, then capture the checkpointed database only.
        with closing(sqlite3.connect(c.HOME / 'loopflow.db')) as src:
            with closing(sqlite3.connect(ROOT / (side + '.db'))) as dst:
                src.backup(dst)
                dst.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                dst.execute('PRAGMA journal_mode=DELETE')
        assert not (ROOT / (side + '.db-wal')).exists()
    digests = {side: tables(ROOT / (side + '.db')) for side in SIDES}
    changed = [name for name in digests['before'] if digests['before'][name] != digests['after'][name]]
    assert set(changed) <= {'schema_migrations'}, changed
    (ROOT / 'store-digests.json').write_text(json.dumps(digests, indent=2))
    manifest['store_difference_tables'] = changed
    manifest['store_sha256'] = {side: hashlib.sha256((ROOT / (side + '.db')).read_bytes()).hexdigest() for side in SIDES}
    (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    # Calibration: use the existing one-row and default workloads, no generated load.
    for pair in range(2):
        for workload in (('easy', 'cli') if pair == 0 else ('cli', 'easy')):
            configure(workload)
            row = c.run('before', workload, f'calibration-{pair}-{workload}')
            assert row['errors'] == 0, row
