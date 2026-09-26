from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).parent
home = '/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590'
cli = Path('/Users/jack/src/loopflow.main-view-task/target/debug/lf')
environment = dict(os.environ)
for key in ('LF_RUN_ID', 'LF_RUN_DIR', 'LF_PARENT_RUN_ID', 'LF_PROCESS_ID', 'LF_WORK_ADVANCE_CLAIM', 'LF_WAVE_ID', 'LF_DIRECTIVE_FILE', 'LOOPFLOW_DIRECTIVE_FILE'):
    environment.pop(key, None)
environment.update(LF_HOME=home, LF_CONTROL_HOME=home, LF_DB_PATH=home+'/loopflow.db', LF_CONTROL_DB_PATH=home+'/loopflow.db', LF_BIN=str(cli), LF_CONTROL_BIN=str(cli))
commands = [('roadmap', '--all', '--json'), ('session', 'list', '--json')]
receipt = {'observed_at': datetime.now(timezone.utc).isoformat(), 'home': home, 'cli_sha256': hashlib.sha256(cli.read_bytes()).hexdigest(), 'commands': []}
for args, filename in zip(commands, ('roadmap.json', 'sessions.json')):
    result = subprocess.run([str(cli), *args], env=environment, capture_output=True, timeout=60, check=True)
    json.loads(result.stdout)
    (root/filename).write_bytes(result.stdout)
    receipt['commands'].append({'argv': [str(cli), *args], 'sha256': hashlib.sha256(result.stdout).hexdigest()})
(root/'read-receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
print('PASS: captured current roadmap and Sessions from the configured Home')
