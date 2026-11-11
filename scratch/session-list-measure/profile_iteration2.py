"""Excluded SQL attribution and native stack samples for iteration 2."""
import json
import subprocess
import time
from iteration2 import ROOT, SIDES, WORKLOADS, c, configure


def profiles():
    rows = []
    for side in SIDES:
        for workload in WORKLOADS:
            configure(workload)
            label = f'profile-{side}-{workload}'
            c.activate(side, True)
            env = c.environment()
            if workload != 'served':
                warm_ms, data, _ = c.cli(workload, env)
                warmup = dict(ms=warm_ms, **c.validate(workload, data))
            trace = ROOT / (label + '-trace.jsonl')
            trace.unlink(missing_ok=True)
            env['SESSION_LIST_TRACE'] = str(trace)
            ms, data, extra = c.served(env, label) if workload == 'served' else c.cli(workload, env)
            row = dict(side=side, workload=workload, label=label, errors=0, ms=ms,
                       **c.validate(workload, data))
            row['warmup'] = extra if workload == 'served' else warmup
            records = [json.loads(line) for line in trace.read_text().splitlines()]
            assert all(r['ns'] is not None for r in records)
            row['statements'] = len(records)
            (ROOT / (label + '.json')).write_text(json.dumps(data))
            rows.append(row)
            print(json.dumps(row), flush=True)
            (ROOT / 'profiles.json').write_text(json.dumps(rows, indent=2))


def sample(side):
    c.activate(side)
    env = c.environment()
    configure('task')
    warm_ms, warm_data, _ = c.cli('task', env)
    warmup = dict(ms=warm_ms, **c.validate('task', warm_data))
    command = ['/usr/bin/sandbox-exec', '-p', '(version 1)(allow default)(deny network*)',
               '/usr/bin/env', 'DYLD_INSERT_LIBRARIES=' + env['DYLD_INSERT_LIBRARIES'],
               str(c.BIN), *c.COMMANDS['task']]
    label = 'sample-' + side
    with (ROOT / (label + '.json')).open('w') as out, (ROOT / (label + '.stderr')).open('w') as err:
        start = time.perf_counter()
        proc = subprocess.Popen(command, cwd=c.REPO, env=env, stdin=subprocess.DEVNULL,
                                stdout=out, stderr=err)
        try:
            time.sleep(.15)
            sampler = subprocess.Popen(['/usr/bin/sample', str(proc.pid), '30', '5', '-mayDie', '-file', str(ROOT / (label + '.txt'))],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            telemetry = []
            while proc.poll() is None:
                if time.perf_counter() - start > 120:
                    raise TimeoutError('sampled task status exceeded 120 seconds')
                ps = subprocess.run(['ps', '-axo', 'pid=,ppid=,comm='], capture_output=True, text=True, check=True)
                entries = [line.strip().split(None, 2) for line in ps.stdout.splitlines()]
                parents = {proc.pid}
                selected = []
                while True:
                    children = [e for e in entries if int(e[1]) in parents and int(e[0]) not in parents]
                    if not children:
                        break
                    selected.extend(children)
                    parents.update(int(e[0]) for e in children)
                telemetry.append(dict(ms=(time.perf_counter()-start)*1000, children=selected))
                time.sleep(.1)
            ms = (time.perf_counter()-start)*1000
            sampler.wait(timeout=20)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
    data = json.loads((ROOT / (label + '.json')).read_text())
    result = dict(side=side, ms=ms, errors=0, warmup=warmup, **c.validate('task', data), descendants=telemetry)
    (ROOT / (label + '-receipt.json')).write_text(json.dumps(result, indent=2))
    print(json.dumps({k: v for k, v in result.items() if k != 'descendants'}), flush=True)


if __name__ == '__main__':
    import sys
    if len(sys.argv) > 1:
        sample(sys.argv[1])
    else:
        profiles()
