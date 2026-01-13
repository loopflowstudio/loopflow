"""Background agent runner entry point.

This module is spawned as a subprocess to run agent iterations.
"""

import argparse
import sys
from pathlib import Path

from loopflow.maestro.agent import AgentStatus
from loopflow.maestro.db import DEFAULT_DB_PATH, load_agent, update_agent_status
from loopflow.maestro.runner import run_agent_continuous, run_agent_iteration


def main():
    """Run an agent iteration or continuous loop."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--agent-id", required=True, help="Agent ID to run")
    parser.add_argument("--repo-root", required=True, help="Repository root path")
    parser.add_argument("--continuous", action="store_true", help="Run in continuous mode")
    parser.add_argument("--max-iterations", type=int, help="Stop after N iterations")
    parser.add_argument("--check-interval", type=int, default=300, help="Seconds between trigger checks")

    args = parser.parse_args()

    agent = load_agent(DEFAULT_DB_PATH, args.agent_id)
    if not agent:
        print(f"Error: Agent {args.agent_id} not found", file=sys.stderr)
        sys.exit(1)

    repo_root = Path(args.repo_root).resolve()
    if not repo_root.exists():
        print(f"Error: Repository not found: {repo_root}", file=sys.stderr)
        sys.exit(1)

    try:
        if args.continuous:
            exit_code = run_agent_continuous(
                agent,
                repo_root,
                check_interval=args.check_interval,
                max_iterations=args.max_iterations,
            )
        else:
            exit_code = run_agent_iteration(agent, repo_root, foreground=False)
    except Exception as e:
        print(f"Error: Agent failed: {e}", file=sys.stderr)
        update_agent_status(DEFAULT_DB_PATH, args.agent_id, AgentStatus.ERROR)
        sys.exit(1)

    sys.exit(exit_code)


if __name__ == "__main__":
    main()
