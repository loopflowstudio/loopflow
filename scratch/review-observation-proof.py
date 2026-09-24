"""Prepare disposable native output for the desktop/CLI review probe."""
from datetime import UTC, datetime
import json
from pathlib import Path
import tempfile

root = Path(tempfile.mkdtemp(prefix='loo293-observation-review-'))
for number, provider in enumerate(['claude', 'codex'], 1):
    run = f'run_{number:032x}'
    directory = root / 'runs' / run[4:6] / run
    directory.mkdir(parents=True)
    path = directory / 'native.jsonl'
    manifest = dict(schema_version=1, run_id=run, parent_run_id=None,
                    created_at=datetime.now(UTC).isoformat(), harness=provider, model=None,
                    surface='tui', cwd='/tmp', repo=None, worktree=None, skill=None,
                    subjects=[dict(selector='task:LOO-293', source='declared')],
                    launch=None, context=None, runtime_path=None, runtime_digest=None,
                    host='review', boot_id=None)
    receipt = dict(schema_version=1, provider_session_id='session', account_id=None,
                   native_source=dict(kind='jsonl', path=str(path)))
    (directory / 'manifest.json').write_text(json.dumps(manifest))
    (directory / 'provider-session.json').write_text(json.dumps(receipt))
    if provider == 'claude':
        initial = [dict(type='assistant', sessionId='session', uuid='call', message=dict(id='assistant', content=[
            dict(type='tool_use', id='call', name='bash', input=dict(command='echo review-proof'))]))]
        arrival = dict(type='user', sessionId='session', uuid='result', message=dict(content=[
            dict(type='tool_result', tool_use_id='call', content='review-proof')]))
    else:
        initial = [dict(type='session_meta', payload=dict(id='session')),
                   dict(type='response_item', payload=dict(id='call', type='function_call', call_id='call',
                        name='bash', arguments='echo review-proof'))]
        arrival = dict(type='response_item', payload=dict(id='result', type='function_call_output',
                       call_id='call', output='review-proof'))
    path.write_text(''.join(json.dumps(row) + '\n' for row in initial))
    (root / f'{provider}-arrival.jsonl').write_text(json.dumps(arrival) + '\n')
for number, provider in enumerate(['claude', 'codex'], 1):
    for ordinal in [1, 2]:
        text = f'{provider} arrival {ordinal}'
        if provider == 'claude':
            row = dict(type='assistant', sessionId='session', uuid=f'prose-{ordinal}',
                       message=dict(id=f'message-{ordinal}', content=[dict(type='text', text=text)]))
        else:
            row = dict(type='response_item', payload=dict(id=f'prose-{ordinal}', type='message',
                       role='assistant', content=[dict(type='output_text', text=text)]))
        (root / f'{provider}-prose-{ordinal}.jsonl').write_text(json.dumps(row) + '\n')
Path('/tmp/loo293-observation-proof-root').write_text(str(root))
print(root)
