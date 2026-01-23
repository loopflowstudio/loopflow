"""Subscription entity persistence and operations.

A Subscription watches a pathset on main and spawns Runs when files change.
"""

import json
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import MergeMode, Subscription, TriggerStatus
from loopflow.lfd.runs.loop import (
    StartResult,
    _allocate_main_branch,
    _create_main_branch,
    count_outstanding,
)

# Persistence


def save_subscription(sub: Subscription, db_path: Path | None = None) -> None:
    """Save or update a subscription."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO subscriptions
        (id, flow, area, repo, goals, pathset, last_main_sha,
         status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            sub.id,
            sub.flow,
            sub.area,
            str(sub.repo),
            json.dumps(sub.goals) if sub.goals else None,
            sub.pathset,
            sub.last_main_sha,
            sub.status.value,
            sub.iteration,
            sub.main_branch,
            sub.pr_limit,
            sub.merge_mode.value,
            sub.pid,
            sub.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_subscription(sub_id: str, db_path: Path | None = None) -> Subscription | None:
    """Get a subscription by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM subscriptions WHERE id = ?", (sub_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM subscriptions WHERE id LIKE ?", (f"{sub_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _subscription_from_row(dict(row)) if row else None


def get_subscription_by_area_repo(
    area: str, repo: Path, db_path: Path | None = None
) -> Subscription | None:
    """Get a subscription by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM subscriptions WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _subscription_from_row(dict(row)) if row else None


def list_subscriptions(repo: Path | None = None, db_path: Path | None = None) -> list[Subscription]:
    """List all subscriptions, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM subscriptions WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM subscriptions ORDER BY created_at DESC")

    subs = [_subscription_from_row(dict(row)) for row in cursor]
    conn.close()
    return subs


def update_subscription_status(
    sub_id: str, status: TriggerStatus, db_path: Path | None = None
) -> bool:
    """Update a subscription's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE subscriptions SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, sub_id, f"{sub_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_subscription_sha(sub_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a subscription's last_main_sha."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE subscriptions SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, sub_id, f"{sub_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_subscription(sub_id: str, db_path: Path | None = None) -> bool:
    """Delete a subscription and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM subscriptions WHERE id = ? OR id LIKE ?", (sub_id, f"{sub_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE parent = ?", (f"subscription:{full_id}",))
    cursor = conn.execute("DELETE FROM subscriptions WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _subscription_from_row(row: dict) -> Subscription:
    """Convert database row to Subscription."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Subscription(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        pathset=row.get("pathset", ""),
        last_main_sha=row.get("last_main_sha"),
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Operations


def create_subscription(
    area: str,
    repo: Path,
    flow: str,
    pathset: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Subscription:
    """Create or get an existing subscription for an area+repo combination."""
    goals = goals or []

    existing = get_subscription_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.pathset != pathset:
            existing.pathset = pathset
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_subscription(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    sub = Subscription(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        pathset=pathset,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_subscription(sub)
    return sub


def start_subscription(subscription_id: str) -> StartResult:
    """Trigger a subscription to spawn a Run."""
    sub = get_subscription(subscription_id)
    if not sub:
        return StartResult(False, "not_found")

    if sub.status == TriggerStatus.RUNNING:
        return StartResult(False, "already_running")

    outstanding = count_outstanding(sub)
    if outstanding >= sub.pr_limit:
        update_subscription_status(subscription_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    update_subscription_status(subscription_id, TriggerStatus.RUNNING)
    subprocess.Popen(
        [sys.executable, "-m", "loopflow.lfd.execution.worker", "subscription", subscription_id],
        cwd=sub.repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return StartResult(True)


# Checking (called periodically by daemon)


def check_subscription(sub: Subscription) -> bool:
    """Check if subscription should trigger. Returns True if triggered.

    Updates last_main_sha as a side effect.
    """
    if not sub.pathset:
        return False

    repo = sub.repo

    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)

    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    current_sha = result.stdout.strip()

    if current_sha == sub.last_main_sha:
        return False

    if sub.last_main_sha is None:
        update_subscription_sha(sub.id, current_sha)
        return False

    paths = [p.strip() for p in sub.pathset.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", sub.last_main_sha, current_sha, "--"] + paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        update_subscription_sha(sub.id, current_sha)
        return False

    update_subscription_sha(sub.id, current_sha)
    return True


def run_subscription_check() -> list[str]:
    """Check all subscriptions and trigger as needed.

    Returns list of subscription IDs that were triggered.
    """
    triggered = []
    for sub in list_subscriptions():
        if sub.status == TriggerStatus.RUNNING:
            continue

        if check_subscription(sub):
            result = start_subscription(sub.id)
            if result:
                triggered.append(sub.id)

    return triggered
