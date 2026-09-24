"""Prove independent continuations through the CLI with disposable source data."""

from datetime import UTC, datetime
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile


def _main() -> None:
    binary = Path(__file__).resolve().parents[1] / 'target/debug/lf'
    database = Path('/Users/jack/.lf/loopflow.db')
    with tempfile.TemporaryDirectory(prefix='loo293-tail-review-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, LF_CONTROL_HOME=str(root), LF_CONTROL_DB_PATH=str(database))
        env.pop('LF_RUN_ID', None)
        now = datetime.now(UTC).isoformat()

        def _source(number: int, provider: str, native: bool) -> Path:
            run = f'run_{number:032x}'
            directory = root / 'runs' / run[4:6] / run
            directory.mkdir(parents=True)
            manifest = dict(schema_version=1, run_id=run, parent_run_id=None,
                            created_at=now, harness=provider, model=None,
                            surface='tui' if native else 'headless', cwd=temporary,
                            repo=None, worktree=None, skill=None,
                            subjects=[dict(selector='task:LOO-293', source='declared')],
                            launch=None, context=None, runtime_path=None, runtime_digest=None,
                            host='review', boot_id=None)
            (directory / 'manifest.json').write_text(json.dumps(manifest))
            path = directory / ('native.db' if provider == 'opencode' else 'events.jsonl')
            if native:
                receipt = dict(schema_version=1, provider_session_id='session', account_id=None,
                               native_source=dict(kind='open_code' if provider == 'opencode' else 'jsonl',
                                                  path=str(path)))
                (directory / 'provider-session.json').write_text(json.dumps(receipt))
            return path

        def _record(provider: str, identity: str, seq: int = 0) -> dict:
            if provider == 'claude':
                return dict(type='assistant', sessionId='session', uuid=identity,
                            message=dict(content=[dict(type='text', text=identity)]))
            if provider == 'codex':
                return dict(type='response_item', payload=dict(id=identity, type='message',
                            role='assistant', content=identity))
            return dict(schema_version=1, seq=seq, observed_at=now, type='conversation',
                        event=dict(type='text_delta', turn_id='turn', content=identity))

        paths = {}
        for number, provider in enumerate(['claude', 'codex', 'journal'], 1):
            path = _source(number, 'codex' if provider == 'journal' else provider, provider != 'journal')
            rows = [_record(provider, f'history-{i}', i) for i in range(400)]
            if provider == 'codex':
                rows.insert(0, dict(type='session_meta', payload=dict(id='session')))
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            paths[provider] = path
        dbpath = _source(4, 'opencode', True)
        with sqlite3.connect(dbpath) as writer:
            writer.executescript('''PRAGMA journal_mode=WAL;
                CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER);
                CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT);
                CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT);
                INSERT INTO session VALUES('session',1);
                INSERT INTO message VALUES('message','session','{"role":"assistant"}');''')
            for i in range(400):
                part = dict(type='text', text=f'history-{i}', time={} if i == 0 else dict(end=i))
                writer.execute('INSERT INTO part VALUES(?,?,?,?,?)', (f'part-{i}', 'message', 'session', i, json.dumps(part)))

        def _read(label: str, cursor: str | None = None, tail: bool = False) -> dict:
            args = [str(binary), 'task', 'output', 'LOO-293', '--json']
            if tail:
                args.append('--tail')
            if cursor:
                path = root / f'{label}.cursor'
                path.write_text(cursor)
                args += ['--cursor', str(path)]
            result = subprocess.run(args, env=env, cwd='/tmp', capture_output=True, text=True, timeout=30)
            result.check_returncode()
            page = json.loads(result.stdout)
            assert not result.stderr, result.stderr
            assert not page['gaps'], page['gaps']
            assert all(s['available'] and not s['gaps'] for s in page['sources']), [(s['provider'],s['available'],s['gaps']) for s in page['sources']]
            return page

        history = _read('history')
        assert all(s['has_more'] for s in history['sources'])
        live = _read('live', tail=True)
        assert [len(s['records']) for s in live['sources']] == [0, 0, 0, 2]
        for provider, path in paths.items():
            with path.open('a') as stream:
                stream.write(json.dumps(_record(provider, 'live-arrival', 400)) + '\n')
        with sqlite3.connect(dbpath) as writer:
            writer.execute('UPDATE part SET data=? WHERE id=?',
                           (json.dumps(dict(type='text', text='live-arrival', time=dict(end=400))), 'part-0'))
        auxiliary = _source(5, 'codex', False)
        auxiliary.write_text(json.dumps(_record('journal', 'auxiliary-first')) + '\n')
        arrivals = _read('arrivals', live['next_cursor'])
        assert [len(s['records']) for s in arrivals['sources']] == [1, 1, 1, 1, 1]
        assert all('live-arrival' in json.dumps(s['records']) for s in arrivals['sources'][:4])
        assert 'auxiliary-first' in json.dumps(arrivals['sources'][4]['records'])
        older = _read('older', history['next_cursor'])
        assert all(s['has_more'] for s in older['sources'][:4])
        assert all('live-arrival' not in json.dumps(s['records']) for s in older['sources'][:4])
        quiet = _read('quiet', arrivals['next_cursor'])
        assert all(not s['records'] for s in quiet['sources'])
        print(json.dumps(dict(sources=5, live_arrivals=5, historical_sources_still_pending=4,
                              quiet_replays=0, gaps=0, provider_processes_started=0)))


if __name__ == '__main__':
    _main()
