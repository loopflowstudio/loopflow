# /// script
# requires-python = ">=3.10"
# dependencies = ["websockets>=15,<16"]
# ///
"""What a running Codex turn does when its home's auth.json changes underneath it."""
import json, os, runpy, shutil, signal, subprocess, sys, tempfile, threading, time
from pathlib import Path
fx = runpy.run_path(sys.argv[1], run_name="fixture_lib")
Responses, Client, login = fx["Responses"], fx["Client"], fx["_login"]
codex = shutil.which("codex"); N = int(sys.argv[2]); variants = sys.argv[3:]

def once(variant: str, delay: float) -> str:
    server = Responses(); server.command = "true"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    root = Path(tempfile.mkdtemp(prefix="lf-repro-", dir="/tmp")); home = root / ".codex"; work = root / "work"
    home.mkdir(); work.mkdir()
    (home / "config.toml").write_text(f'''model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
[features]
shell_snapshot = false
[model_providers.fixture]
name = "Local synthetic fixture"
base_url = "http://127.0.0.1:{server.server_port}/v1"
wire_api = "responses"
requires_openai_auth = false
[analytics]
enabled = false
[feedback]
enabled = false
''')
    auth = home / "auth.json"; auth.write_text(login("first"))
    env = {k: os.environ[k] for k in ("PATH", "TMPDIR", "LANG") if k in os.environ}; env["HOME"] = str(root)
    endpoint = root / "engine.sock"
    log = (root / "engine.log").open("w")
    engine = subprocess.Popen([codex, "app-server", "--listen", f"unix://{endpoint}"], cwd=work, env=env,
                              stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 10
        while not endpoint.exists() and time.monotonic() < deadline: time.sleep(0.05)
        client = Client(endpoint)
        thread = client.call("thread/start", {"cwd": str(work), "model": "gpt-5.4", "modelProvider": "fixture",
                                               "approvalPolicy": "never", "sandbox": "danger-full-access"})["thread"]["id"]
        if variant.startswith("idle"):
            # Switch with no turn in flight, wait, then run a turn; then a second.
            staged = home / "auth.json.new"; staged.write_text(login("second")); os.replace(staged, auth)
            time.sleep(2.0)
            out = []
            for _ in range(2):
                t = client.start_turn(thread, "plain turn")
                deadline = time.monotonic() + 25
                while time.monotonic() < deadline:
                    event = client.pending.get(timeout=max(0.1, deadline - time.monotonic()))
                    if event.get("method") == "turn/completed" and event["params"]["turn"]["id"] == t:
                        tt = event["params"]["turn"]; out.append(tt["status"] + (": " + tt["error"]["message"] if tt.get("error") else "")); break
            return " | ".join(out)
        turn = client.start_turn(thread, "held conversation")
        assert server.held.wait(20), "never reached the provider"
        time.sleep(delay)
        def install(text):
            staged = home / "auth.json.new"; staged.write_text(text); os.replace(staged, auth)
        if variant == "same-bytes": install(auth.read_text())
        elif variant == "rotated": install(login("first", "rotated"))
        elif variant in ("other-account", "recover"): install(login("second"))
        elif variant == "in-place-other": auth.write_text(login("second"))
        elif variant == "deleted": auth.unlink()
        time.sleep(delay)
        server.release.set()
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            event = client.pending.get(timeout=max(0.1, deadline - time.monotonic()))
            if event.get("method") == "turn/completed" and event["params"]["turn"]["id"] == turn:
                t = event["params"]["turn"]
                first = t["status"] + (": " + t["error"]["message"][13:60] if t.get("error") else "")
                if variant != "recover": return first
                # After a killed turn, does the same engine and thread take another?
                t2 = client.start_turn(thread, "plain turn")
                while time.monotonic() < deadline:
                    event = client.pending.get(timeout=max(0.1, deadline - time.monotonic()))
                    if event.get("method") == "turn/completed" and event["params"]["turn"]["id"] == t2:
                        tt = event["params"]["turn"]; return first + " -> next turn " + tt["status"] + (": " + tt["error"]["message"][13:60] if tt.get("error") else "")
                return first + " -> next turn timeout"
        return "timeout"
    except Exception as error:
        return f"harness error: {type(error).__name__}: {str(error)[:120]}"
    finally:
        server.release.set()
        try: os.killpg(engine.pid, signal.SIGTERM); engine.wait(timeout=10)
        except Exception: pass
        server.shutdown(); shutil.rmtree(root, ignore_errors=True)

for variant in variants:
    for delay in (0.0, 0.5):
        results = {}
        for _ in range(N):
            r = once(variant, delay); results[r] = results.get(r, 0) + 1
        print(f"{variant:15} delay={delay}: {results}", flush=True)
