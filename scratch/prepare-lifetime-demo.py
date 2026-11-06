"""Prepare private LOO-717 review data using a complete copied candidate."""
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1]
ROOT = Path('/Users/jack/.lf/demos/loo717-500425244c')
DURABLE = ROOT / 'durable'
RUNTIME = ROOT / 'runtime'
sys.path.insert(0, str(SOURCE / 'scripts'))
from app_bundle import install_app_executable


def main() -> None:
    manifest_path = DURABLE / 'manifest.json'
    if manifest_path.exists():
        raise RuntimeError('Existing demo manifest; preserve it instead of reprovisioning')
    bundle = RUNTIME / 'Loopflow LOO-717 Demo.app'
    contents = bundle / 'Contents'
    resources = contents / 'Resources'
    resources.mkdir(parents=True, exist_ok=True)
    plist = plistlib.loads((SOURCE / 'swift/LoopflowMac/Info.plist').read_bytes())
    plist.update(CFBundleIdentifier='com.loopflow.mac.demo.loo717.500425244c',
                 CFBundleExecutable='Loopflow LOO-717 Demo', CFBundleName='Loopflow LOO-717 Demo',
                 CFBundleDisplayName='Loopflow LOO-717 Demo')
    (contents / 'Info.plist').write_bytes(plistlib.dumps(plist))
    executable = install_app_executable(SOURCE / 'swift/.build/debug', contents / 'Info.plist', contents)
    cli = contents / 'MacOS/lf'
    shutil.copy2(SOURCE / 'target/debug/lf', cli)
    for name in ('LoopflowSwift_Loopflow.bundle', 'LoopflowSwift_LoopflowMac.bundle'):
        destination = resources / name
        if not destination.exists():
            shutil.copytree(SOURCE / 'swift/.build/debug' / name, destination, symlinks=True)
    shutil.copy2(SOURCE / 'swift/LoopflowMac/AppIcon.icns', resources)
    subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(bundle)], check=True)
    repo = DURABLE / 'repo'
    repo.mkdir(exist_ok=True)
    machine = DURABLE / 'machine'
    machine.mkdir(exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith(('LF_', 'LOOPFLOW_UI_TEST_'))}
    env.update(LF_HOME=str(machine), LF_BIN=str(cli), LF_USER_NAME='Jack Heart')

    def run(args: list[str]) -> str:
        result = subprocess.run(args, cwd=repo, env=env, stdin=subprocess.DEVNULL,
                                text=True, capture_output=True, check=True)
        return result.stdout

    (repo / 'README.md').write_text('# LOO-717 disposable demo\n')
    (repo / 'AGENTS.md').write_text('Disposable demonstration data. Respond to the explicit demo request only.\nNever launch Graphs, implement features, claim work, or operate other Tasks.\n')
    for args in (['init', '-b', 'main'], ['config', 'user.name', 'Jack Heart'],
                 ['config', 'user.email', 'loo717-demo@example.invalid'], ['add', '.'],
                 ['commit', '-m', 'Disposable shell and viewer demo']):
        run(['git', *args])
    run(['git', 'init', '--bare', str(DURABLE / 'origin.git')])
    run(['git', 'remote', 'add', 'origin', str(DURABLE / 'origin.git')])
    run(['git', 'push', '-q', 'origin', 'main'])
    tasks = []
    for title in ('Viewer retains checkout', 'Shell continuity'):
        task = json.loads(run([str(cli), 'task', 'create', '--title', title, '--notes',
            'Disposable demo only. No implementation or Graph work. Wait for explicit review input.', '--json']))
        run([str(cli), 'checkout', task['id'], '--json'])
        plan = json.loads(run([str(cli), 'desktop', 'plan', '--task', task['id'], '--json']))
        tasks.append({'id': task['id'], 'title': title, 'plan': plan})
    manifest = {'source': run(['git', '-C', str(SOURCE), 'rev-parse', 'HEAD']).strip(),
                'durable_root': str(DURABLE), 'runtime_root': str(RUNTIME), 'machine': str(machine),
                'repository': str(repo), 'bundle': str(bundle), 'executable': str(executable),
                'cli': str(cli), 'tasks': tasks, 'owners': [], 'human_input': 'none yet'}
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    for name, args in [('launch', [str(executable), '--repo', str(repo)]), ('lf', [str(cli), '"$@"'])]:
        script = DURABLE / (name + '.sh')
        command = ' '.join('"$@"' if a == '"$@"' else shlex.quote(a) for a in args)
        script.write_text('#!/bin/zsh\ncd ' + shlex.quote(str(repo)) + '\nexec env $(env | grep \'^LF_\' | cut -d= -f1 | sed \'s/^/-u /\') LF_HOME=' + shlex.quote(str(machine)) + ' LF_BIN=' + shlex.quote(str(cli)) + ' ' + command + '\n')
        script.chmod(0o700)
    print(json.dumps({'manifest': str(manifest_path), 'tasks': [{'id': t['id'], 'title': t['title']} for t in tasks]}))


if __name__ == '__main__':
    main()
