"""Excluded native stack samples and exact security spawn counts; no credentials."""
import json
from pathlib import Path
import subprocess
import sys
import time

from iteration3 import ROOT, COMMITS, WORKLOADS, c, configure


def counts():
    subprocess.run(['cc', '-dynamiclib', '-O2', str(Path(__file__).with_name('count_security.c')),
                    '-o', str(ROOT / 'count_security.dylib')], check=True)
    for side in COMMITS:
        for workload in WORKLOADS:
            configure(workload)
            for trial in range(5):
                c.activate(side)
                env = c.environment()
                ms, data, _ = c.cli(workload, env)
                warmup = dict(ms=ms, **c.validate(workload, data))
                label = f'count-{side}-{workload}-{trial}'
                log = ROOT / (label + '.spawns')
                log.unlink(missing_ok=True)
                env['DYLD_INSERT_LIBRARIES'] = str(ROOT / 'freeze_clock.dylib') + ':' + str(ROOT / 'count_security.dylib')
                env['MEASURE_SECURITY_SPAWNS'] = str(log)
                ms, data, _ = c.cli(workload, env)
                row = dict(label=label, side=side, workload=workload, ms=ms, errors=0,
                           security_spawns=len(set(log.read_text().splitlines())) if log.exists() else 0,
                           warmup=warmup, **c.validate(workload, data))
                assert row['full_digest'] == warmup['full_digest']
                (ROOT / (label + '.json')).write_text(json.dumps(data))
                with (ROOT / 'counts.jsonl').open('a') as out:
                    out.write(json.dumps(row) + '\n')
                print(json.dumps(row), flush=True)


def sample(side):
    configure('task')
    c.activate(side)
    env = c.environment()
    ms, data, _ = c.cli('task', env)
    warmup = dict(ms=ms, **c.validate('task', data))
    label = 'sample-' + side
    command = ['/usr/bin/sandbox-exec', '-p', '(version 1)(allow default)(deny network*)',
               '/usr/bin/env', 'DYLD_INSERT_LIBRARIES=' + env['DYLD_INSERT_LIBRARIES'],
               str(c.BIN), *c.COMMANDS['task']]
    sampler = None
    with (ROOT / (label + '.json')).open('w') as out, (ROOT / (label + '.stderr')).open('w') as err:
        start = time.perf_counter()
        proc = subprocess.Popen(command, cwd=c.REPO, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=err)
        try:
            time.sleep(.05)
            sampler = subprocess.Popen(['/usr/bin/sample', str(proc.pid), '30', '1', '-mayDie',
                                        '-file', str(ROOT / (label + '.txt'))],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            assert proc.wait(timeout=120) == 0
            ms = (time.perf_counter() - start) * 1000
            sampler.wait(timeout=35)
        finally:
            for child in (proc, sampler):
                if child is not None and child.poll() is None:
                    child.kill()
                    child.wait()
    data = json.loads((ROOT / (label + '.json')).read_text())
    row = dict(label=label, side=side, workload='task', ms=ms, errors=0, warmup=warmup, **c.validate('task', data))
    assert row['full_digest'] == warmup['full_digest']
    (ROOT / (label + '-receipt.json')).write_text(json.dumps(row, indent=2))
    print(json.dumps(row), flush=True)


if __name__ == '__main__':
    if sys.argv[1] == 'counts':
        counts()
    else:
        sample(sys.argv[1])
