"""Progressive tests for lfd flow execution.

Tests are organized by complexity:
1. One-off flows (FLOW type) - single iteration, no scheduler
2. Loop iterations (LOOP type) - multiple iterations, PR limits
3. Scheduler integration - slot coordination
4. Triggers - cron, subscribe

Each level builds on the previous, testing increasingly complex scenarios.
"""

import subprocess
from contextlib import ExitStack
from datetime import datetime
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.flows import resolve_flow
from loopflow.lfd.db import save_loop
from loopflow.lfd.job_runner import (
    _iteration_branch_prefix,
    _scheduler_acquire,
    _scheduler_release,
    run_iteration,
)
from loopflow.lfd.jobs import StartResult, count_outstanding, create_loop, start_loop
from loopflow.lfd.models import Loop, LoopStatus, LoopType

# =============================================================================
# Test Utilities
# =============================================================================


def patch_many(*targets):
    """Patch multiple targets without nesting.

    Usage:
        with patch_many("mod.func1", "mod.func2", ("mod.func3", return_value)):
            ...
    """
    stack = ExitStack()
    mocks = {}
    for target in targets:
        if isinstance(target, tuple):
            path, value = target
            if callable(value):
                m = stack.enter_context(patch(path, side_effect=value))
            else:
                m = stack.enter_context(patch(path, return_value=value))
        else:
            m = stack.enter_context(patch(target))
        mocks[target if isinstance(target, str) else target[0]] = m
    return stack, mocks


# =============================================================================
# Test Fixtures
# =============================================================================


@pytest.fixture
def temp_repo(tmp_path):
    """Create a temporary git repo with minimal structure."""
    repo = tmp_path / "repo"
    repo.mkdir()

    subprocess.run(["git", "init"], cwd=repo, capture_output=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo, capture_output=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, capture_output=True)

    (repo / "README.md").write_text("# Test Repo")
    subprocess.run(["git", "add", "."], cwd=repo, capture_output=True)
    subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=repo, capture_output=True)

    return repo


@pytest.fixture
def temp_db(tmp_path):
    """Create a temporary database path."""
    return tmp_path / "test.db"


@pytest.fixture
def simple_flow(temp_repo):
    """Create a simple one-step flow."""
    flows_dir = temp_repo / ".lf" / "flows"
    flows_dir.mkdir(parents=True)
    (flows_dir / "simple.py").write_text('def flow():\n    return {"steps": ["implement"]}')

    commands_dir = temp_repo / ".claude" / "commands"
    commands_dir.mkdir(parents=True)
    (commands_dir / "implement.md").write_text("Implement the feature.")

    return "simple"


@pytest.fixture
def multi_step_flow(temp_repo):
    """Create a multi-step flow."""
    flows_dir = temp_repo / ".lf" / "flows"
    flows_dir.mkdir(parents=True)
    (flows_dir / "ship.py").write_text(
        'def flow():\n    return {"steps": ["implement", "test", "polish"]}'
    )

    commands_dir = temp_repo / ".claude" / "commands"
    commands_dir.mkdir(parents=True)
    (commands_dir / "implement.md").write_text("Implement the feature.")
    (commands_dir / "test.md").write_text("Write tests.")
    (commands_dir / "polish.md").write_text("Polish the code.")

    return "ship"


@pytest.fixture
def with_goals(temp_repo):
    """Add goals directory to repo."""
    goals_dir = temp_repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "product-engineer.md").write_text("# Product Engineer\nBuild features.")
    return goals_dir


# =============================================================================
# Level 1: One-off Flow Tests (FLOW type)
# =============================================================================


class TestFlowResolution:
    """Test flow loading and resolution."""

    def test_load_simple_flow(self, temp_repo, simple_flow):
        """Simple flow loads and resolves to single step."""
        from loopflow.lf.flows import load_flow

        flow_def = load_flow(simple_flow, temp_repo)
        assert flow_def is not None
        assert flow_def.name == "simple"
        assert len(flow_def.steps) == 1
        assert flow_def.steps[0].step == "implement"

    def test_resolve_simple_flow(self, temp_repo, simple_flow):
        """Simple flow resolves to flat step list."""
        from loopflow.lf.flows import load_flow

        flow_def = load_flow(simple_flow, temp_repo)
        resolved = resolve_flow(flow_def, temp_repo)

        assert len(resolved) == 1
        assert resolved[0].step == "implement"
        assert resolved[0].parallel_group is None

    def test_resolve_multi_step_flow(self, temp_repo, multi_step_flow):
        """Multi-step flow resolves to ordered steps."""
        from loopflow.lf.flows import load_flow

        flow_def = load_flow(multi_step_flow, temp_repo)
        resolved = resolve_flow(flow_def, temp_repo)

        assert len(resolved) == 3
        assert [s.step for s in resolved] == ["implement", "test", "polish"]

    def test_flow_not_found_returns_none(self, temp_repo):
        """Missing flow returns None."""
        from loopflow.lf.flows import load_flow

        assert load_flow("nonexistent", temp_repo) is None


class TestLoopCreation:
    """Test loop creation and persistence."""

    def test_create_flow_loop(self, temp_repo, simple_flow):
        """Create a FLOW type loop."""
        with patch_many(
            ("loopflow.lfd.jobs._branch_exists", False),
            "loopflow.lfd.jobs._create_loop_main_branch",
        )[0]:
            loop = create_loop(LoopType.FLOW, area="src/feature/", repo=temp_repo, flow=simple_flow)

        assert loop.type == LoopType.FLOW
        assert loop.area == "src/feature/"
        assert loop.flow == simple_flow
        assert loop.status == LoopStatus.IDLE
        assert loop.iteration == 0

    def test_create_loop_reuses_existing(self, temp_repo, simple_flow):
        """Creating loop with same area reuses existing."""
        with patch_many(
            ("loopflow.lfd.jobs._branch_exists", False),
            "loopflow.lfd.jobs._create_loop_main_branch",
        )[0]:
            loop1 = create_loop(
                LoopType.FLOW, area="src/feature/", repo=temp_repo, flow=simple_flow
            )
            loop2 = create_loop(
                LoopType.FLOW, area="src/feature/", repo=temp_repo, flow=simple_flow
            )

        assert loop1.id == loop2.id

    def test_create_loop_updates_goals(self, temp_repo, simple_flow):
        """Updating goals on existing loop persists changes."""
        with patch_many(
            ("loopflow.lfd.jobs._branch_exists", False),
            "loopflow.lfd.jobs._create_loop_main_branch",
        )[0]:
            create_loop(
                LoopType.FLOW,
                area="src/feature/",
                repo=temp_repo,
                flow=simple_flow,
                goals=["product-engineer"],
            )
            loop2 = create_loop(
                LoopType.FLOW,
                area="src/feature/",
                repo=temp_repo,
                flow=simple_flow,
                goals=["product-engineer", "security"],
            )

        assert set(loop2.goals) == {"product-engineer", "security"}


class TestRunIteration:
    """Test single iteration execution with mocked side effects."""

    def _make_worktree(self, temp_repo):
        """Helper to create mock worktree."""
        wt_path = temp_repo / "worktree"
        wt_path.mkdir(exist_ok=True)
        return wt_path

    def test_run_iteration_missing_flow_returns_false(self, temp_repo, temp_db):
        """Iteration fails if flow not specified."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=None,
        )
        save_loop(loop, temp_db)

        with patch_many(
            (
                "loopflow.lfd.job_runner.create_worktree",
                lambda *a, **k: self._make_worktree(temp_repo),
            ),
            "loopflow.lfd.job_runner.notify_event",
            "loopflow.lfd.job_runner.save_job_run",
            "loopflow.lfd.job_runner.update_job_run_status",
            "loopflow.lfd.job_runner._cleanup_worktree",
        )[0]:
            result = run_iteration(loop, 1)

        assert result is False

    def test_run_iteration_unknown_flow_returns_false(self, temp_repo, temp_db):
        """Iteration fails if flow doesn't exist."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow="nonexistent",
        )
        save_loop(loop, temp_db)

        with patch_many(
            (
                "loopflow.lfd.job_runner.create_worktree",
                lambda *a, **k: self._make_worktree(temp_repo),
            ),
            "loopflow.lfd.job_runner.notify_event",
            "loopflow.lfd.job_runner.save_job_run",
            "loopflow.lfd.job_runner.update_job_run_status",
            "loopflow.lfd.job_runner._cleanup_worktree",
        )[0]:
            result = run_iteration(loop, 1)

        assert result is False

    def test_run_iteration_worktree_error_returns_false(self, temp_repo, temp_db, simple_flow):
        """Iteration fails if worktree creation fails."""
        from loopflow.lf.worktrees import WorktreeError

        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=simple_flow,
        )
        save_loop(loop, temp_db)

        def raise_error(*a, **k):
            raise WorktreeError("Failed")

        with patch_many(
            ("loopflow.lfd.job_runner.create_worktree", raise_error),
            "loopflow.lfd.job_runner.notify_event",
        )[0]:
            result = run_iteration(loop, 1)

        assert result is False


class TestIterationBranchPrefix:
    """Test branch prefix derivation."""

    def test_strips_main_suffix(self):
        assert _iteration_branch_prefix("feature-main") == "feature"
        assert _iteration_branch_prefix("api-aurora-melody-main") == "api-aurora-melody"

    def test_handles_no_suffix(self):
        assert _iteration_branch_prefix("feature") == "feature"


# =============================================================================
# Level 2: Loop Iteration Tests (LOOP type)
# =============================================================================


class TestStartResult:
    """Test StartResult behavior."""

    def test_truthy_when_ok(self):
        result = StartResult(True)
        assert result.ok is True
        assert result

    def test_falsy_when_not_ok(self):
        result = StartResult(False, "already_running")
        assert result.ok is False
        assert not result

    def test_includes_reason(self):
        result = StartResult(False, "waiting", outstanding=5)
        assert result.reason == "waiting"
        assert result.outstanding == 5


class TestCountOutstanding:
    """Test counting outstanding commits."""

    def test_count_outstanding_no_branch(self, temp_repo):
        """Returns 0 when branch doesn't exist."""
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            area="src/",
            repo=temp_repo,
            loop_main="nonexistent-main",
        )
        assert count_outstanding(loop) == 0

    def test_count_outstanding_with_commits(self, temp_repo):
        """Counts commits ahead of main."""
        loop = Loop(
            id="loop-1", type=LoopType.LOOP, area="src/", repo=temp_repo, loop_main="test-main"
        )

        with patch("subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),  # fetch
                MagicMock(returncode=0, stdout="3\n"),  # rev-list
            ]
            count = count_outstanding(loop)

        assert count == 3


class TestStartLoop:
    """Test loop start logic."""

    def test_start_loop_not_found(self):
        """Returns not_found for missing loop."""
        with patch_many(("loopflow.lfd.jobs.get_job", None))[0]:
            result = start_loop("nonexistent")

        assert not result
        assert result.reason == "not_found"

    def test_start_loop_already_running(self, temp_repo):
        """Returns already_running if process is active."""
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            status=LoopStatus.RUNNING,
            pid=12345,
        )

        with patch_many(
            ("loopflow.lfd.jobs.get_job", loop),
            ("loopflow.lfd.jobs.is_process_running", True),
        )[0]:
            result = start_loop("loop-1")

        assert not result
        assert result.reason == "already_running"

    def test_start_loop_waiting_pr_limit(self, temp_repo):
        """Returns waiting if PR limit reached."""
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            status=LoopStatus.IDLE,
            pr_limit=5,
        )

        with patch_many(
            ("loopflow.lfd.jobs.get_job", loop),
            ("loopflow.lfd.jobs.count_outstanding", 5),
            "loopflow.lfd.jobs.update_job_status",
        )[0]:
            result = start_loop("loop-1")

        assert not result
        assert result.reason == "waiting"
        assert result.outstanding == 5


# =============================================================================
# Level 3: Scheduler Integration Tests
# =============================================================================


class TestSchedulerCalls:
    """Test scheduler RPC calls."""

    def test_scheduler_acquire_no_daemon(self):
        """Returns (True, None) when daemon not running."""
        with patch("loopflow.lfd.job_runner.SOCKET_PATH") as mock_path:
            mock_path.exists.return_value = False
            acquired, reason = _scheduler_acquire("run-1")

        assert acquired is True
        assert reason is None

    def test_scheduler_release_no_daemon(self):
        """Release is safe when daemon not running."""
        with patch("loopflow.lfd.job_runner.SOCKET_PATH") as mock_path:
            mock_path.exists.return_value = False
            _scheduler_release("run-1")  # Should not raise


# =============================================================================
# Level 4: Trigger Tests
# =============================================================================


class TestScheduleTrigger:
    """Test cron schedule evaluation."""

    def test_should_trigger_first_run(self):
        """First run triggers if within grace period."""
        from loopflow.lfd.schedule import should_trigger_cron

        assert should_trigger_cron("* * * * *", None) is True

    def test_should_trigger_after_missed(self):
        """Triggers when last run was before scheduled time."""
        from loopflow.lfd.schedule import should_trigger_cron

        last_run = datetime(2024, 1, 14, 9, 0, 0)
        assert should_trigger_cron("* * * * *", last_run) is True

    def test_skip_beyond_grace_period(self):
        """Skips when beyond grace period."""
        from datetime import timedelta

        from loopflow.lfd.schedule import should_trigger_cron

        assert should_trigger_cron("* * * * *", None, grace_period=timedelta(seconds=0)) is False


class TestSubscribeTrigger:
    """Test file change subscription."""

    def test_check_subscription_no_pathset(self, temp_repo):
        """Returns False when no pathset configured."""
        from loopflow.lfd.subscribe import check_subscription

        loop = Loop(
            id="loop-1",
            type=LoopType.SUBSCRIBE,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            pathset=None,
        )
        assert check_subscription(loop) is False

    def test_check_subscription_first_run(self, temp_repo):
        """First run sets baseline, doesn't trigger."""
        from loopflow.lfd.subscribe import check_subscription

        loop = Loop(
            id="loop-1",
            type=LoopType.SUBSCRIBE,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            pathset="src/**/*.py",
            last_main_sha=None,
        )

        with patch("loopflow.lfd.subscribe.subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="abc123\n"),
            ]
            with patch("loopflow.lfd.subscribe.update_loop_last_sha"):
                result = check_subscription(loop)

        assert result is False

    def test_check_subscription_no_changes(self, temp_repo):
        """No trigger when SHA unchanged."""
        from loopflow.lfd.subscribe import check_subscription

        loop = Loop(
            id="loop-1",
            type=LoopType.SUBSCRIBE,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            pathset="src/**/*.py",
            last_main_sha="abc123",
        )

        with patch("loopflow.lfd.subscribe.subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="abc123\n"),
            ]
            result = check_subscription(loop)

        assert result is False

    def test_check_subscription_with_changes(self, temp_repo):
        """Triggers when SHA changes and pathset matches."""
        from loopflow.lfd.subscribe import check_subscription

        loop = Loop(
            id="loop-1",
            type=LoopType.SUBSCRIBE,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            pathset="src/**/*.py",
            last_main_sha="abc123",
        )

        with patch("loopflow.lfd.subscribe.subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="def456\n"),
                MagicMock(returncode=0, stdout="src/feature.py\n"),
            ]
            with patch("loopflow.lfd.subscribe.update_loop_last_sha"):
                result = check_subscription(loop)

        assert result is True

    def test_check_subscription_no_matching_files(self, temp_repo):
        """No trigger when SHA changes but pathset doesn't match."""
        from loopflow.lfd.subscribe import check_subscription

        loop = Loop(
            id="loop-1",
            type=LoopType.SUBSCRIBE,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            pathset="src/**/*.py",
            last_main_sha="abc123",
        )

        with patch("loopflow.lfd.subscribe.subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="def456\n"),
                MagicMock(returncode=0, stdout=""),
            ]
            with patch("loopflow.lfd.subscribe.update_loop_last_sha"):
                result = check_subscription(loop)

        assert result is False


# =============================================================================
# Level 5: End-to-End Flow Execution Tests
# =============================================================================


class TestFullIterationExecution:
    """Test complete iteration execution with mocked subprocess calls."""

    def _make_worktree(self, temp_repo, branch):
        wt_path = temp_repo / f"worktree-{branch.replace('/', '-')}"
        wt_path.mkdir(exist_ok=True)
        return wt_path

    def _iteration_patches(self, temp_repo, step_fn=None, pr_fn=None):
        """Build patch stack for iteration tests."""
        step_fn = step_fn or (lambda *a, **k: 0)
        pr_fn = pr_fn or (lambda *a, **k: "https://github.com/test/repo/pull/1")

        return patch_many(
            (
                "loopflow.lfd.job_runner.create_worktree",
                lambda repo, branch, **k: self._make_worktree(temp_repo, branch),
            ),
            ("loopflow.lfd.job_runner._run_collector_step", step_fn),
            ("loopflow.lfd.job_runner._create_pr_to_job_main", pr_fn),
            "loopflow.lfd.job_runner._auto_merge_pr",
            "loopflow.lfd.job_runner._cleanup_worktree",
            "loopflow.lfd.job_runner.notify_event",
            "loopflow.lfd.job_runner.save_job_run",
            "loopflow.lfd.job_runner.update_job_run_status",
            "loopflow.lfd.job_runner.update_job_run_step",
            "loopflow.lfd.job_runner.update_job_run_pr",
        )

    def test_iteration_creates_worktree(self, temp_repo, simple_flow, temp_db, with_goals):
        """Iteration creates a worktree for the branch."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-aurora-melody-main",
            flow=simple_flow,
            goals=["product-engineer"],
        )
        save_loop(loop, temp_db)

        worktrees = []
        original_make = self._make_worktree

        def track_worktree(repo, branch, **k):
            worktrees.append(branch)
            return original_make(temp_repo, branch)

        stack, _ = patch_many(
            ("loopflow.lfd.job_runner.create_worktree", track_worktree),
            ("loopflow.lfd.job_runner._run_collector_step", lambda *a, **k: 0),
            ("loopflow.lfd.job_runner._create_pr_to_job_main", None),
            "loopflow.lfd.job_runner._auto_merge_pr",
            "loopflow.lfd.job_runner._cleanup_worktree",
            "loopflow.lfd.job_runner.notify_event",
            "loopflow.lfd.job_runner.save_job_run",
            "loopflow.lfd.job_runner.update_job_run_status",
            "loopflow.lfd.job_runner.update_job_run_step",
            "loopflow.lfd.job_runner.update_job_run_pr",
        )

        with stack:
            run_iteration(loop, 1)

        assert worktrees == ["test-aurora-melody/001"]

    def test_iteration_runs_all_steps(self, temp_repo, multi_step_flow, temp_db, with_goals):
        """Iteration runs all steps in order."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-aurora-melody-main",
            flow=multi_step_flow,
            goals=["product-engineer"],
        )
        save_loop(loop, temp_db)

        steps = []

        def track_step(prompt, wt, backend, model, skip, sess, step_label, **kw):
            steps.append(step_label)
            return 0

        stack, _ = self._iteration_patches(temp_repo, step_fn=track_step)
        with stack:
            result = run_iteration(loop, 1)

        assert result is True
        assert len(steps) == 3
        assert "src/:implement" in steps[0]
        assert "src/:test" in steps[1]
        assert "src/:polish" in steps[2]

    def test_iteration_stops_on_step_failure(self, temp_repo, multi_step_flow, temp_db, with_goals):
        """Iteration stops and returns False when a step fails."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=multi_step_flow,
            goals=["product-engineer"],
        )
        save_loop(loop, temp_db)

        steps = []

        def failing_step(prompt, wt, backend, model, skip, sess, step_label, **kw):
            steps.append(step_label)
            return 1 if "test" in step_label else 0

        stack, _ = self._iteration_patches(temp_repo, step_fn=failing_step)
        with stack:
            result = run_iteration(loop, 1)

        assert result is False
        assert len(steps) == 2

    def test_iteration_creates_pr_on_success(self, temp_repo, simple_flow, temp_db, with_goals):
        """Successful iteration creates PR."""
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=simple_flow,
            goals=["product-engineer"],
        )
        save_loop(loop, temp_db)

        prs = []

        def track_pr(loop, wt, branch, iteration):
            prs.append((branch, iteration))
            return "https://github.com/test/repo/pull/1"

        stack, _ = self._iteration_patches(temp_repo, pr_fn=track_pr)
        with stack:
            result = run_iteration(loop, 1)

        assert result is True
        assert len(prs) == 1
        assert prs[0][1] == 1


class TestForkJoinFlows:
    """Test fork/join flow patterns."""

    @pytest.fixture
    def fork_join_flow(self, temp_repo):
        """Create a flow with fork and join."""
        flows_dir = temp_repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "parallel.py").write_text("""
def flow():
    return {
        "steps": [
            {"fork": ["variant-a", "variant-b"]},
            {"join": {"join": {"step": "synthesize"}}},
            "polish",
        ]
    }
""")

        commands_dir = temp_repo / ".claude" / "commands"
        commands_dir.mkdir(parents=True)
        (commands_dir / "variant-a.md").write_text("Implement variant A.")
        (commands_dir / "variant-b.md").write_text("Implement variant B.")
        (commands_dir / "synthesize.md").write_text("Combine the variants.")
        (commands_dir / "polish.md").write_text("Polish the code.")

        return "parallel"

    def test_resolve_fork_join_flow(self, temp_repo, fork_join_flow):
        """Fork/join flow resolves with parallel groups marked."""
        from loopflow.lf.flows import load_flow

        flow_def = load_flow(fork_join_flow, temp_repo)
        resolved = resolve_flow(flow_def, temp_repo)

        assert len(resolved) == 4
        assert resolved[0].step == "variant-a"
        assert resolved[0].parallel_group == 0
        assert resolved[1].step == "variant-b"
        assert resolved[1].parallel_group == 0
        assert resolved[2].join is not None
        assert resolved[3].step == "polish"
        assert resolved[3].parallel_group is None


class TestLoopIterations:
    """Test continuous loop behavior (LOOP type)."""

    def _loop_patches(self, run_iteration_fn, count_outstanding_fn=None):
        """Build patch stack for loop iteration tests."""
        targets = [
            ("loopflow.lfd.job_runner.run_iteration", run_iteration_fn),
            ("loopflow.lfd.job_runner._scheduler_acquire", (True, None)),
            "loopflow.lfd.job_runner._scheduler_release",
            "loopflow.lfd.job_runner.update_loop_iteration",
            "loopflow.lfd.job_runner.update_loop_status",
            "loopflow.lfd.job_runner.update_loop_pid",
            "loopflow.lfd.job_runner.notify_event",
        ]
        if count_outstanding_fn:
            targets.append(("loopflow.lfd.job_runner.count_outstanding", count_outstanding_fn))
        return patch_many(*targets)

    def test_run_loop_iterations_stops_at_pr_limit(
        self, temp_repo, simple_flow, temp_db, with_goals
    ):
        """Loop stops when PR limit reached."""
        from loopflow.lfd.job_runner import run_loop_iterations

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=simple_flow,
            goals=["product-engineer"],
            pr_limit=2,
            iteration=0,
        )
        save_loop(loop, temp_db)

        iterations = []

        def mock_run(loop, iteration, run_id=None):
            iterations.append(iteration)
            return True

        outstanding = iter([0, 1, 2])
        stack, _ = self._loop_patches(mock_run, lambda _: next(outstanding))

        with stack:
            run_loop_iterations(loop)

        assert len(iterations) == 2

    def test_run_loop_iterations_flow_runs_once(self, temp_repo, simple_flow, temp_db, with_goals):
        """FLOW type runs exactly once."""
        from loopflow.lfd.job_runner import run_loop_iterations

        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            area="src/",
            repo=temp_repo,
            loop_main="test-main",
            flow=simple_flow,
            goals=["product-engineer"],
            iteration=0,
        )
        save_loop(loop, temp_db)

        iterations = []

        def mock_run(loop, iteration, run_id=None):
            iterations.append(iteration)
            return True

        stack, _ = self._loop_patches(mock_run)

        with stack:
            run_loop_iterations(loop)

        assert len(iterations) == 1
