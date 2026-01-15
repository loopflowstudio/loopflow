"""Background agent runner entry point.

This module is spawned as a subprocess to run agent iterations.
Loads agent spec from file and updates lfd.db for runtime state.
"""

import argparse
import sys
from pathlib import Path

from loopflow.lfd.agents import get_agent
from loopflow.lfd.db import get_latest_run, update_run_status
from loopflow.lfd.models import AgentStatus
from loopflow.lfd.runner import run_agent_iteration


def main():
    """Run an agent iteration."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--agent-name", required=True, help="Agent name to run")
    parser.add_argument("--run-id", required=True, help="Run ID for tracking")
    parser.add_argument("--repo-root", required=True, help="Repository root path")
    parser.add_argument("--foreground", action="store_true", help="Show output")

    args = parser.parse_args()

    agent = get_agent(args.agent_name)
    if not agent:
        print(f"Error: Agent '{args.agent_name}' not found", file=sys.stderr)
        update_run_status(args.run_id, AgentStatus.ERROR, error="Agent not found")
        sys.exit(1)

    repo_root = Path(args.repo_root).resolve()
    if not repo_root.exists():
        print(f"Error: Repository not found: {repo_root}", file=sys.stderr)
        update_run_status(args.run_id, AgentStatus.ERROR, error="Repository not found")
        sys.exit(1)

    # Get iteration from the run record
    latest = get_latest_run(args.agent_name)
    iteration = latest.iteration if latest else 1

    try:
        exit_code = run_agent_iteration(
            agent,
            args.run_id,
            iteration,
            repo_root,
            foreground=args.foreground,
        )
    except Exception as e:
        print(f"Error: Agent failed: {e}", file=sys.stderr)
        update_run_status(args.run_id, AgentStatus.ERROR, error=str(e))
        sys.exit(1)

    # Update status based on result
    if exit_code == 0:
        update_run_status(args.run_id, AgentStatus.IDLE)
    else:
        update_run_status(args.run_id, AgentStatus.ERROR)

    sys.exit(exit_code)


if __name__ == "__main__":
    main()
