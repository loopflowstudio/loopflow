#!/usr/bin/env python3
"""Demo: algedonic signal escalation via repair chain.

Creates a wave with a step that always fails, then verifies:
1. Original run fails
2. Repair runs 1-3 are dispatched with backoff (30s, 60s, 120s)
3. After 3 failed repairs, an algedonic attention item is created
4. Attention item is visible via GET /attention

Total runtime ~4 minutes (backoff delays + step execution time).

Usage:
    uv run python scripts/demo-algedonic.py
    uv run python scripts/demo-algedonic.py --skip-build   # reuse existing binary
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import httpx

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lib.lfd_runtime import LfdRuntime

REPO_ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description="Demo algedonic signal escalation")
    parser.add_argument("--skip-build", action="store_true", help="Skip cargo build")
    args = parser.parse_args()

    print("=== Algedonic Signal Demo ===\n")

    with LfdRuntime(repo_root=REPO_ROOT, build_binary=not args.skip_build) as rt:
        client = httpx.Client(
            base_url=rt.base_url,
            headers={"Authorization": f"Bearer {rt.token}"},
            timeout=30.0,
        )

        # 1. Create wave with a flow that always fails
        print("1. Creating wave with always-failing step...")
        create_failing_step(rt.repo_dir)

        resp = client.post(
            "/waves",
            json={
                "name": "algedonic-demo",
                "repo": str(rt.repo_dir),
                "flow": "always-fail",
                "direction": [],
                "area": [],
                "mode": "manual",
            },
        )
        resp.raise_for_status()
        wave_id = resp.json()["id"]
        print(f"   Wave created: {wave_id}")

        # 2. Run the wave — step will fail
        print("2. Running wave (step will fail)...")
        resp = client.post(f"/waves/{wave_id}/run")
        resp.raise_for_status()

        # 3. Wait for the full repair chain to play out
        # Original failure + 3 repair attempts with backoff (30+60+120 = 210s)
        print("3. Waiting for repair chain (original + 3 repairs with backoff)...")
        print("   This takes ~4 minutes due to backoff delays.\n")

        max_wait = 360  # 6 minutes max
        start = time.time()
        last_run_count = 0

        while time.time() - start < max_wait:
            resp = client.get(f"/waves/{wave_id}/runs")
            if resp.status_code == 200:
                runs = resp.json()
                if len(runs) > last_run_count:
                    last_run_count = len(runs)
                    for run in runs:
                        repair_marker = " (repair)" if run.get("repair_of") else ""
                        print(
                            f"   Run {run['id'][:8]}... "
                            f"status={run['status']}{repair_marker}"
                        )

                # Check for attention items
                attn_resp = client.get("/attention")
                if attn_resp.status_code == 200:
                    items = attn_resp.json()
                    algedonic = [
                        i for i in items if i.get("kind") == "algedonic"
                    ]
                    if algedonic:
                        print(f"\n4. Algedonic attention item created!")
                        item = algedonic[0]
                        print(f"   ID: {item['id']}")
                        print(f"   Title: {item['title']}")
                        print(f"   Summary: {item['summary'][:100]}")
                        print(f"   Status: {item['status']}")

                        # Count total runs
                        all_runs = client.get(f"/waves/{wave_id}/runs").json()
                        repair_runs = [
                            r for r in all_runs if r.get("repair_of")
                        ]
                        print(f"\n5. Summary:")
                        print(f"   Total runs: {len(all_runs)}")
                        print(f"   Repair attempts: {len(repair_runs)}")
                        print(f"   Elapsed: {time.time() - start:.0f}s")
                        print(f"\n=== Demo complete ===")
                        return

            time.sleep(5)

        print(f"\nTimed out after {max_wait}s waiting for algedonic signal.")
        print("Logs:")
        print(rt.logs()[-2000:])
        sys.exit(1)


def create_failing_step(repo_dir: Path) -> None:
    """Create a .lf/steps/always-fail.md step that exits with failure."""
    steps_dir = repo_dir / ".lf" / "steps"
    steps_dir.mkdir(parents=True, exist_ok=True)

    (steps_dir / "always-fail.md").write_text(
        """\
---
requires: none
produces: nothing
---
Exit with an error to test repair escalation.

## Workflow

1. Print a message and exit with failure:
   ```bash
   echo "This step intentionally fails for demo purposes" && exit 1
   ```
"""
    )

    # Create the flow
    flows_dir = repo_dir / ".lf" / "flows"
    flows_dir.mkdir(parents=True, exist_ok=True)

    (flows_dir / "always-fail.yaml").write_text(
        """\
steps:
  - always-fail
"""
    )

    # Commit the step so lfd can find it
    import subprocess

    subprocess.run(
        ["git", "add", "."],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "add always-fail step and flow"],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )


if __name__ == "__main__":
    main()
