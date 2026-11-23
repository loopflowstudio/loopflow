//! Held provider fixtures shared by Graph and Task lifecycle checks.
use std::fs;
use std::path::Path;
use std::process::Command;

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
if streamed and '--settings' in sys.argv and os.path.exists(turn):
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
    if not os.path.isdir(home):
        sys.exit(0)
    time.sleep(0.02)
if not streamed:
    sys.exit(0)
message['session_id'] = 'stand-in'
keep_claude_turn(message, 'Held.')
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'Held.'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': 'Held.'}), flush=True)
for line in sys.stdin:
    pass
"#;

/// Give the held worker a reparented build and a detached service. Dropping
/// the writer releases both, including after a failed assertion.
pub fn hold_build_jobs(home: &Path) -> fs::File {
    let pipe = home.join("build-input");
    assert!(Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap()
        .success());
    // Closing the test's writer releases the job even if an assertion fails.
    let writer = fs::File::options()
        .read(true)
        .write(true)
        .open(&pipe)
        .unwrap();
    fs::write(home.join("release"), "").unwrap();
    fs::write(home.join("hold-turn"), "").unwrap();
    fs::write(
        home.join("tool_turn.py"),
        r#"import os, signal, subprocess
from pathlib import Path
home = Path(os.environ['LF_HOME'])
# Both processes ignore TERM and stay alive until the test closes its writer.
signal.signal(signal.SIGTERM, signal.SIG_IGN)
with (home / 'build-input').open('rb') as pipe:
    def start(**group):
        return subprocess.Popen(['/bin/cat'], stdin=pipe, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL, **group).pid
    (home / 'build').write_text(str(start(preexec_fn=os.setpgrp)))
    (home / 'daemon').write_text(str(start(start_new_session=True)))
"#,
    )
    .unwrap();

    writer
}

/// Read the identity the tool recorded before the provider announced its held turn.
pub fn worker_job(home: &Path, name: &str) -> crate::process::OsProcess {
    let pid = fs::read_to_string(home.join(name))
        .unwrap()
        .parse()
        .unwrap();
    crate::process::OsProcess::read(pid).unwrap().unwrap()
}
