//! Held provider fixtures shared by Graph and Task lifecycle checks.
/// A Claude stand-in that announces `ready` in LF_HOME, then waits there for
/// `release` before it answers. While `hold-turn` is there it writes `holding`
/// and keeps its turn open.
pub fn holding_claude() -> String {
    super::claude_stand_in(HOLDING_CLAUDE)
}

const HOLDING_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, subprocess, sys, time
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = os.environ['LF_HOME']
streamed = '--input-format' in sys.argv
message = json.loads(sys.stdin.readline()) if streamed else None
with open(os.path.join(home, 'ready'), 'w') as ready:
    ready.write('ready')
if streamed:
    with open(os.path.join(home, 'stream-ready'), 'w') as ready:
        ready.write('ready')
# A fixture that left `tool_turn.py` gets the hooks lf declared, run around a tool.
turn = os.path.join(home, 'tool_turn.py')
if streamed and os.path.exists(turn):
    hooks = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))['hooks']
    subprocess.run([sys.executable, turn, 'stand-in', json.dumps(hooks)], check=True)
# A fixture that left `hold-turn` holds a headless turn open until it is removed.
if streamed and os.path.exists(os.path.join(home, 'hold-turn')):
    # Claude echoes the request and names its conversation before it works.
    print(json.dumps(message | {'session_id': 'stand-in'}), flush=True)
    print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
    with open(os.path.join(home, 'holding'), 'w') as holding:
        holding.write('stand-in')
    while os.path.exists(os.path.join(home, 'hold-turn')):
        time.sleep(0.02)
while not os.path.exists(os.path.join(home, 'release')):
    time.sleep(0.02)
if not streamed:
    sys.exit(0)
message['session_id'] = 'stand-in'
keep_claude_turn(message, 'Held.')
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
if os.path.exists(os.path.join(home, 'ask-question')):
    question = {'name': 'AskUserQuestion', 'type': 'tool_use', 'id': 'question',
                'input': {'questions': [{'question': 'Use the fixture?', 'header': 'Fixture', 'options': [
                    {'label': 'Yes', 'description': 'Use it'}, {'label': 'No', 'description': 'Skip it'}],
                    'multiSelect': False}]}}
    print(json.dumps({'type': 'assistant', 'message': {'content': [question]}}), flush=True)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'Held.'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': 'Held.'}), flush=True)
for line in sys.stdin:
    pass
"#;
