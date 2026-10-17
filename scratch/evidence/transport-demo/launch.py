import os
import sys
from pathlib import Path


repo = Path(__file__).resolve().parents[3]
binary = repo / "target/debug/lf"
env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
env["LF_BIN"] = str(binary)
env["LF_HOME"] = (Path(__file__).parent / "home.txt").read_text().strip()
os.chdir(repo)
if len(sys.argv) > 1 and sys.argv[1] == "--lf":
    args = sys.argv[2:]
elif len(sys.argv) > 1:
    args = ["session", "attach", sys.argv[1]]
else:
    args = [
        "--interactive", "--agent", "claude", ":",
        "This is a bounded LOO-415 transport demonstration, not an implementation "
        "assignment. Do not edit files, run shell commands, launch agents or Flows, "
        "or operate this Task. Immediately use AskUserQuestion to ask: "
        "Which word should survive reconnect? Offer Orchid and Quartz. Wait for "
        "the answer. After the answer, reply with exactly DEMO_COMPLETE:<chosen word>. "
        "Keep this native conversation open for the demonstration.",
    ]
os.execve(binary, [str(binary), *args], env)
