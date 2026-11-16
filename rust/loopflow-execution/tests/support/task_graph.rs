//! Held provider fixtures shared by Graph and Task lifecycle checks.
pub const HOLDING_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys, time
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
while not os.path.exists(os.path.join(home, 'release')):
    time.sleep(0.02)
if not streamed:
    sys.exit(0)
message['session_id'] = 'stand-in'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'Held.'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': 'Held.'}), flush=True)
for line in sys.stdin:
    pass
"#;
