"""Private local planning reads: deny network so task status cannot exchange."""
import hashlib
import json
from pathlib import Path
import subprocess
import time
import compare as c

PLAN=[
    ('planning-offline',('before','after2'),('task','waves','board','selection')),
]


def cli(workload, env):
    # The system launcher strips inherited DYLD variables; env reinstalls the
    # clock shim for the unsigned benchmark binary inside the network sandbox.
    command=['/usr/bin/sandbox-exec','-p','(version 1)(allow default)(deny network*)',
             '/usr/bin/env','DYLD_INSERT_LIBRARIES='+env['DYLD_INSERT_LIBRARIES'],
             str(c.BIN),*c.COMMANDS[workload]]
    start=time.perf_counter()
    p=subprocess.run(command,cwd=c.REPO,env=env,stdin=subprocess.DEVNULL,capture_output=True,timeout=120)
    ms=(time.perf_counter()-start)*1000
    if p.returncode:raise RuntimeError(f'command exit {p.returncode}: {p.stderr.decode()[:300]}')
    return ms,json.loads(p.stdout),p.stderr.decode()


if __name__=='__main__':
    c.cli=cli
    (c.ROOT/'planning-plan.json').write_text(json.dumps(dict(plan=PLAN,script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2))
    for name,sides,workloads in PLAN:
        for workload in workloads:
            expected=None
            for pair in range(5):
                for side in (sides if pair%2==0 else sides[::-1]):
                    row=c.run(side,workload,f'{name}-{workload}-{pair}-{side}')
                    assert row['errors']==0,row
                    if expected is None:expected=row['full_digest']
                    assert row['full_digest']==expected,f'output mismatch: {row["label"]}'
                    assert row['warmup']['full_digest']==expected,f'warmup mismatch: {row["label"]}'
