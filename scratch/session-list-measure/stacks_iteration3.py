"""Inclusive main-thread stack attribution, avoiding nested double-counting."""
import json
from pathlib import Path
import re

from iteration3 import ROOT, COMMITS

FUNCTIONS = ('read_claude_keychain_blob_with_security', 'LaunchCapacity', 'load_planning_transport',
             'resolve_layers', 'check_scope', 'repository_for_task', 'task_for_directory',
             'session_summary', 'listed', 'snapshot_task_details', 'load_config', 'sqlite3_step',
             'RepoId8discover', 'sync_scope', 'deliver_fields', 'resolve_repository_context')

result = {}
for side in COMMITS:
    path = ROOT / f'sample-{side}.txt'
    if not path.exists():
        continue
    text = path.read_text()
    main = text.split('Call graph:\n', 1)[1].splitlines()
    total = int(re.search(r'\d+', main[0]).group())
    lines = []
    for line in main[1:]:
        if re.match(r'^    \d+ Thread_', line) or line.startswith('Total number'):
            break
        match = re.match(r'^([ +!:|`]+)(\d+) (.+)', line)
        if match:
            lines.append((len(match[1]), int(match[2]), match[3]))
    functions = {}
    for name in FUNCTIONS:
        count = 0
        parent = None
        for depth, n, symbol in lines:
            if parent is not None and depth > parent:
                continue
            parent = None
            if name in symbol:
                count += n
                parent = depth
        functions[name] = dict(samples=count, percent=100*count/total)
    result[side] = dict(main_thread_samples=total, functions=functions)
    print(side, total, {k: round(v['percent'],1) for k,v in functions.items() if v['samples']})
(ROOT / 'stack-summary.json').write_text(json.dumps(result, indent=2))
