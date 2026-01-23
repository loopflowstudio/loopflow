"""Export and import triggers to/from YAML files.

Triggers are stored in the database but can be exported for backup,
sharing, or version control. Import adds triggers to the database.
"""

from pathlib import Path

import yaml

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Loop, MergeMode, Schedule, Subscription, TriggerStatus
from loopflow.lfd.runs.loop import get_loop, list_loops, save_loop
from loopflow.lfd.runs.schedule import get_schedule, list_schedules, save_schedule
from loopflow.lfd.runs.subscription import (
    get_subscription,
    list_subscriptions,
    save_subscription,
)


def _loop_to_dict(loop: Loop) -> dict:
    """Convert Loop to exportable dict."""
    return {
        "id": loop.id,
        "flow": loop.flow,
        "area": loop.area,
        "repo": str(loop.repo),
        "goals": loop.goals if loop.goals else None,
        "main_branch": loop.main_branch,
        "pr_limit": loop.pr_limit,
        "merge_mode": loop.merge_mode.value,
    }


def _subscription_to_dict(sub: Subscription) -> dict:
    """Convert Subscription to exportable dict."""
    return {
        "id": sub.id,
        "flow": sub.flow,
        "area": sub.area,
        "repo": str(sub.repo),
        "goals": sub.goals if sub.goals else None,
        "pathset": sub.pathset,
        "main_branch": sub.main_branch,
        "pr_limit": sub.pr_limit,
        "merge_mode": sub.merge_mode.value,
    }


def _schedule_to_dict(sched: Schedule) -> dict:
    """Convert Schedule to exportable dict."""
    return {
        "id": sched.id,
        "flow": sched.flow,
        "area": sched.area,
        "repo": str(sched.repo),
        "goals": sched.goals if sched.goals else None,
        "cron": sched.cron,
        "main_branch": sched.main_branch,
        "pr_limit": sched.pr_limit,
        "merge_mode": sched.merge_mode.value,
    }


def _dict_to_loop(d: dict) -> Loop:
    """Convert dict to Loop."""
    return Loop(
        id=d["id"],
        flow=d["flow"],
        area=d["area"],
        repo=Path(d["repo"]),
        goals=d.get("goals") or [],
        status=TriggerStatus.IDLE,
        iteration=0,
        main_branch=d.get("main_branch", ""),
        pr_limit=d.get("pr_limit", 5),
        merge_mode=MergeMode(d.get("merge_mode", "pr")),
    )


def _dict_to_subscription(d: dict) -> Subscription:
    """Convert dict to Subscription."""
    return Subscription(
        id=d["id"],
        flow=d["flow"],
        area=d["area"],
        repo=Path(d["repo"]),
        goals=d.get("goals") or [],
        pathset=d.get("pathset", ""),
        status=TriggerStatus.IDLE,
        iteration=0,
        main_branch=d.get("main_branch", ""),
        pr_limit=d.get("pr_limit", 5),
        merge_mode=MergeMode(d.get("merge_mode", "pr")),
    )


def _dict_to_schedule(d: dict) -> Schedule:
    """Convert dict to Schedule."""
    return Schedule(
        id=d["id"],
        flow=d["flow"],
        area=d["area"],
        repo=Path(d["repo"]),
        goals=d.get("goals") or [],
        cron=d.get("cron", ""),
        status=TriggerStatus.IDLE,
        iteration=0,
        main_branch=d.get("main_branch", ""),
        pr_limit=d.get("pr_limit", 5),
        merge_mode=MergeMode(d.get("merge_mode", "pr")),
    )


# Export


def export_triggers(
    output_dir: Path,
    repo: Path | None = None,
) -> dict[str, int]:
    """Export triggers to YAML files in output_dir.

    Returns dict with counts: {"loops": N, "subscriptions": N, "schedules": N}
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    counts = {"loops": 0, "subscriptions": 0, "schedules": 0}

    # Export loops
    loops = list_loops(repo)
    if loops:
        data = [_loop_to_dict(loop) for loop in loops]
        (output_dir / "loops.yaml").write_text(yaml.dump(data, default_flow_style=False))
        counts["loops"] = len(loops)

    # Export subscriptions
    subs = list_subscriptions(repo)
    if subs:
        data = [_subscription_to_dict(sub) for sub in subs]
        (output_dir / "watch.yaml").write_text(yaml.dump(data, default_flow_style=False))
        counts["subscriptions"] = len(subs)

    # Export schedules
    scheds = list_schedules(repo)
    if scheds:
        data = [_schedule_to_dict(sched) for sched in scheds]
        (output_dir / "cron.yaml").write_text(yaml.dump(data, default_flow_style=False))
        counts["schedules"] = len(scheds)

    return counts


# Import


def _clear_triggers(trigger_type: str, db_path: Path | None = None) -> int:
    """Delete all triggers of a type. Returns count deleted."""
    table = {"loops": "loops", "subscriptions": "subscriptions", "schedules": "schedules"}[
        trigger_type
    ]
    conn = _get_db(db_path)
    cursor = conn.execute(f"DELETE FROM {table}")
    conn.commit()
    count = cursor.rowcount
    conn.close()
    return count


def import_triggers(
    input_dir: Path,
    replace: bool = False,
    clean: bool = False,
) -> dict[str, dict[str, int]]:
    """Import triggers from YAML files in input_dir.

    Args:
        input_dir: Directory containing loops.yaml, watch.yaml, cron.yaml
        replace: If True, update existing triggers by ID. If False, skip existing.
        clean: If True, delete all existing triggers of each type before import.

    Returns dict with counts per type: {"loops": {"added": N, "skipped": N, "updated": N}}
    """
    results: dict[str, dict[str, int]] = {
        "loops": {"added": 0, "skipped": 0, "updated": 0, "cleared": 0},
        "subscriptions": {"added": 0, "skipped": 0, "updated": 0, "cleared": 0},
        "schedules": {"added": 0, "skipped": 0, "updated": 0, "cleared": 0},
    }

    # Import loops
    loops_file = input_dir / "loops.yaml"
    if loops_file.exists():
        if clean:
            results["loops"]["cleared"] = _clear_triggers("loops")

        data = yaml.safe_load(loops_file.read_text()) or []
        for item in data:
            existing = get_loop(item["id"])
            if existing:
                if replace:
                    loop = _dict_to_loop(item)
                    loop.iteration = existing.iteration
                    loop.status = existing.status
                    save_loop(loop)
                    results["loops"]["updated"] += 1
                else:
                    results["loops"]["skipped"] += 1
            else:
                loop = _dict_to_loop(item)
                save_loop(loop)
                results["loops"]["added"] += 1

    # Import subscriptions (watch.yaml)
    watch_file = input_dir / "watch.yaml"
    if watch_file.exists():
        if clean:
            results["subscriptions"]["cleared"] = _clear_triggers("subscriptions")

        data = yaml.safe_load(watch_file.read_text()) or []
        for item in data:
            existing = get_subscription(item["id"])
            if existing:
                if replace:
                    sub = _dict_to_subscription(item)
                    sub.iteration = existing.iteration
                    sub.status = existing.status
                    sub.last_main_sha = existing.last_main_sha
                    save_subscription(sub)
                    results["subscriptions"]["updated"] += 1
                else:
                    results["subscriptions"]["skipped"] += 1
            else:
                sub = _dict_to_subscription(item)
                save_subscription(sub)
                results["subscriptions"]["added"] += 1

    # Import schedules (cron.yaml)
    cron_file = input_dir / "cron.yaml"
    if cron_file.exists():
        if clean:
            results["schedules"]["cleared"] = _clear_triggers("schedules")

        data = yaml.safe_load(cron_file.read_text()) or []
        for item in data:
            existing = get_schedule(item["id"])
            if existing:
                if replace:
                    sched = _dict_to_schedule(item)
                    sched.iteration = existing.iteration
                    sched.status = existing.status
                    save_schedule(sched)
                    results["schedules"]["updated"] += 1
                else:
                    results["schedules"]["skipped"] += 1
            else:
                sched = _dict_to_schedule(item)
                save_schedule(sched)
                results["schedules"]["added"] += 1

    return results
