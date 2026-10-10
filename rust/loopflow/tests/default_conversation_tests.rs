use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn bare_lf_launches_a_conversational_prompt_with_an_agent_option() {
    for batch in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let claude = bin.join("claude");
        fs::write(
            &claude,
            r#"#!/usr/bin/python3
import json, os, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
args = '\n'.join(sys.argv[1:]) + '\n'
open(os.environ['TEST_ARGS'], 'w').write(args)
context = open(sys.argv[sys.argv.index('--append-system-prompt-file') + 1]).read()
open(os.environ['TEST_CONTEXT'], 'w').write(context)
if '--input-format' in sys.argv:
    message = json.loads(sys.stdin.readline())
    message['session_id'] = 'fixture'
    print(json.dumps(message), flush=True)
    print(json.dumps({'type': 'result', 'session_id': 'fixture', 'subtype': 'success', 'result': 'hello'}), flush=True)
else:
    print('hello')
"#,
        )
        .unwrap();
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();

        let repo = temp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        for args in [
            &["init", "--quiet"][..],
            &["commit", "--quiet", "--allow-empty", "-m", "start"][..],
        ] {
            let status = Command::new("git")
                .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap();
            assert!(status.success());
        }

        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        if batch {
            command.arg("-b");
        }
        let output = command
            .args(["-a", "claude:sonnet"])
            .current_dir(&repo)
            .env_clear()
            .env("HOME", temp.path())
            .env("LF_HOME", temp.path().join("home"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("TEST_ARGS", temp.path().join("received-args"))
            .env("TEST_CONTEXT", temp.path().join("received-context"))
            .env("PATH", path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let args = fs::read_to_string(temp.path().join("received-args")).unwrap();
        assert!(args.contains("--model\nsonnet\n"), "{args}");
        assert_eq!(
            args.lines().any(|arg| arg == "--input-format"),
            batch,
            "{args}"
        );
        let prompt = fs::read_to_string(temp.path().join("received-context")).unwrap();
        assert!(prompt.contains("<lf:skill:default>"), "{prompt}");
        assert!(!prompt.contains("<lf:skill:repo/operate>"), "{prompt}");
        assert!(!prompt.contains("lf session list --json"), "{prompt}");
    }
}
