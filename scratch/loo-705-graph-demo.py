"""Exercise an installed-release Graph across a private entry switch."""

import json
import os
from pathlib import Path
import shlex
import sqlite3
import subprocess
import tempfile


OLD = Path('/Users/jack/.lf/bin/lf-5854cc113ff596c596479426a567d305f27d2f48f5332fd57ad94aee1f8d4f44')
NEW = Path(__file__).resolve().parents[1] / 'target/debug/lf'


def _run(args, cwd, env):
    return subprocess.run(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                          capture_output=True, text=True, timeout=45, check=True)


with tempfile.TemporaryDirectory(prefix='loo705-graph-', dir='/tmp') as temporary:
    root = Path(temporary)
    repo, home, remote = [root / name for name in ('repo', 'home', 'remote')]
    for path in (repo, home, remote):
        path.mkdir()
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('LF_', 'LOOPFLOW_', 'CMUX_'))}
    entry = root / 'lf-entry'
    env.update(HOME=str(home), LF_HOME=str(home), LF_BIN=str(entry), NO_COLOR='1')
    for args in (['init', '-b', 'main'], ['config', 'user.name', 'Demo'],
                 ['config', 'user.email', 'demo@example.invalid'],
                 ['commit', '--allow-empty', '-m', 'Demo root']):
        _run(['git', *args], repo, env)
    _run(['git', 'init', '--bare', '-b', 'main'], remote, env)
    _run(['git', 'remote', 'add', 'origin', str(remote)], repo, env)
    _run(['git', 'push', '-u', 'origin', 'main'], repo, env)
    graphs = repo / '.lf/graphs'
    graphs.mkdir(parents=True)
    (graphs / 'upgrade-proof.yaml').write_text('- cmd: sync --plan\n- cmd: sync --plan\n')
    first = root / 'first-step'
    second = root / 'second-step'
    quote = shlex.quote
    second.write_text('#!/bin/sh\nexec ' + quote(str(NEW)) + ' "$@"\n')
    second.chmod(0o755)
    first.write_text('#!/bin/sh\nset -eu\n' + quote(str(OLD)) + ' "$@"\n'
                     + 'ln -s ' + quote(str(second)) + ' ' + quote(str(root / 'next')) + '\n'
                     + 'mv -f ' + quote(str(root / 'next')) + ' ' + quote(str(entry)) + '\n')
    first.chmod(0o755)
    entry.symlink_to(first)
    versions = {name: _run([str(binary), '--version'], repo, env).stdout.strip()
                for name, binary in [('before', OLD), ('after', NEW)]}
    result = subprocess.run([str(OLD), '-b', 'graph', 'upgrade-proof'], cwd=repo,
                            env=env, stdin=subprocess.DEVNULL, capture_output=True,
                            text=True, timeout=45)
    with sqlite3.connect(home / 'loopflow.db') as connection:
        connection.row_factory = sqlite3.Row
        rows = [dict(row) for row in connection.execute(
            "SELECT p.id,p.command,p.outcome,p.exit_code FROM processes p "
            "JOIN graph_process_steps s ON s.lf_process_id=p.id ORDER BY p.started_at,p.rowid")]
        runners = [dict(row) for row in connection.execute(
            "SELECT p.id,p.command,p.outcome FROM processes p "
            "JOIN graph_processes g ON g.lf_process_id=p.id")]
    evidence = dict(versions=versions, returncode=result.returncode,
                    stdout=result.stdout, stderr=result.stderr, runners=runners, steps=rows)
    print(json.dumps(evidence, indent=2))
    assert result.returncode == 0, result.stderr
    assert len(rows) == 2, rows
    assert [json.loads(row['command'])[0] for row in rows] == [str(OLD), str(NEW)], rows
    assert all(row['outcome'] == 'succeeded' for row in rows), rows
    assert len(runners) == 1 and runners[0]['outcome'] == 'succeeded', runners
