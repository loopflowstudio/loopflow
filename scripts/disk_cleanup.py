#!/usr/bin/env python3
"""Bounded machine cleanup; source pruning remains owned by the installed lf."""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from resource_envelope import _lock_recovery, load_policy

LABEL = "com.loopflow.disk-cleanup"
MARKER = "# loopflow:disk-cleanup"
BUILD_NAMES = {"target", "node_modules", ".venv", ".build", "DerivedData"}
XCODE_CACHE_NAMES = {"ModuleCache.noindex", "SDKStatCaches.noindex", "CompilationCache.noindex"}
DISCOVERY_MAX_ENTRIES = 100_000
BUILDERS = re.compile(
    r"^(cargo|rustc|rustdoc|swift|xcodebuild|clang|gcc|cc$|c\+\+$|ld$|"
    r"ninja|make|cmake|node|npm|npx|yarn|pnpm|uv$|python|go$|java|gradle|bazel)",
    re.IGNORECASE,
)


class CleanupStopped(Exception):
    pass


@dataclass(frozen=True)
class Candidate:
    path: Path
    category: str
    checkout: Path | None = None
    companions: tuple[Path, ...] = ()

    @property
    def paths(self) -> tuple[Path, ...]:
        return (self.path, *self.companions)


@dataclass(frozen=True)
class Activity:
    paths: frozenset[Path]
    builders: frozenset[Path]

    def reason(self, candidate: Candidate) -> str | None:
        if any(_within(held, path) for held in self.paths for path in candidate.paths):
            return "held open"
        checkout = candidate.checkout
        if checkout and any(_within(cwd, checkout) for cwd in self.builders):
            return "builder in checkout"
        return None


def _within(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _owning_checkout(path: Path) -> Path | None:
    return next((parent for parent in (path, *path.parents) if (parent / ".git").exists()), None)


def _stop_group(process: subprocess.Popen) -> None:
    # A timed-out tool may have children even after its leader exits.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


class Cleaner:
    def __init__(self, dry_run: bool, budget_seconds: float) -> None:
        self.home = Path.home().resolve()
        self.src = Path(os.environ.get("LOOPFLOW_SRC", self.home / "src")).resolve()
        self.lf_home = Path(os.environ.get("LF_HOME", self.home / ".lf")).resolve()
        self.lf = self.home / ".local/bin/lf"
        self.dry_run = dry_run
        self.deadline = time.time() + budget_seconds
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
        self.env["LF_HOME"] = str(self.lf_home)
        self.env["UV_LOCK_TIMEOUT"] = "0"
        self.policy = load_policy()
        self.failures: list[str] = []
        self.entries: list[dict] = []
        self.removed_bytes = 0
        self.log = self.home / "Library/Logs/loopflow-disk-cleanup.log"
        self.disk_log = self.lf_home / "logs/disk.log"

    def _check_time(self) -> None:
        if time.time() >= self.deadline:
            raise CleanupStopped("cleanup wall-clock budget exhausted")

    def _note(self, message: str) -> None:
        line = f"{time.strftime('%Y-%m-%d %H:%M:%S')} {message}"
        failures = []
        try:
            print(line, flush=True)
        except OSError as exc:
            failures.append(f"cannot write cleanup output: {exc}")
        try:
            self.log.parent.mkdir(parents=True, exist_ok=True)
            with self.log.open("a") as output:
                output.write(line + "\n")
        except OSError as exc:
            failures.append(f"cannot append {self.log}: {exc}")
        for failure in failures:
            if failure not in self.failures:
                self.failures.append(failure)
                try:
                    print(f"FAILED: {failure}", file=sys.stderr, flush=True)
                except OSError:
                    pass  # launchd's output file can share the full filesystem.

    def _failure(self, message: str) -> None:
        self.failures.append(message)
        self._note(f"FAILED: {message}")

    def _command(
        self,
        args: list[str],
        cwd: Path | None = None,
        timeout: float = 120,
        allowed: tuple[int, ...] = (0,),
    ) -> str:
        self._check_time()
        with subprocess.Popen(
            args,
            cwd=cwd,
            env=self.env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
            text=True,
        ) as process:
            try:
                stdout, stderr = process.communicate(
                    timeout=min(timeout, self.deadline - time.time())
                )
            except subprocess.TimeoutExpired:
                _stop_group(process)
                self._check_time()
                raise
            except BaseException:
                _stop_group(process)
                raise
        if process.returncode not in allowed:
            raise RuntimeError(f"{args[0]} exited {process.returncode}: {stderr.strip()}")
        return stdout

    def _activity(self) -> Activity:
        try:
            output = self._command(["lsof", "-nP", "-Fpcfn"], timeout=30)
            paths: set[Path] = set()
            builders: set[Path] = set()
            command = descriptor = ""
            resolved: dict[str, Path] = {}
            pid = None
            for line in output.splitlines():
                if line.startswith("p"):
                    pid = int(line[1:])
                    command = descriptor = ""
                elif line.startswith("c"):
                    command = line[1:]
                elif line.startswith("f"):
                    descriptor = line[1:]
                elif line.startswith("n/"):
                    raw = line[1:]
                    if raw not in resolved:
                        resolved[raw] = Path(raw).resolve()
                    path = resolved[raw]
                    paths.add(path)
                    if pid != os.getpid() and descriptor == "cwd" and BUILDERS.match(command):
                        builders.add(path)
            if not paths:
                raise RuntimeError("no paths returned")
            return Activity(frozenset(paths), frozenset(builders))
        except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as exc:
            raise CleanupStopped(f"open-file inspection failed; stopping cleanup: {exc}") from exc

    def _measure(self, candidate: Candidate) -> tuple[int, float]:
        allocated = 0
        newest = 0.0
        deadline = time.time() + 30
        for root in candidate.paths:
            if root.is_symlink() or root.resolve() != root:
                raise ValueError("symlink root or ancestor")
            device = root.lstat().st_dev
            pending = [root]
            while pending:
                self._check_time()
                if time.time() >= deadline:
                    raise RuntimeError("content inspection exceeded 30 seconds")
                path = pending.pop()
                if path.name == ".git":
                    raise ValueError("embedded Git checkout")
                info = path.lstat()
                if info.st_dev != device:
                    raise ValueError("nested mount")
                allocated += info.st_blocks * 512
                newest = max(newest, info.st_mtime)
                if stat.S_ISDIR(info.st_mode):
                    pending.extend(path.iterdir())
        return allocated, newest

    def _build_minutes(self) -> int:
        free = shutil.disk_usage(self.home).free
        if free < self.policy.minimum_free_disk_bytes:
            return 15
        if free < self.policy.cleanup_target_free_disk_bytes:
            return 60
        return 240

    def _reason(self, candidate: Candidate, newest: float, activity: Activity) -> str | None:
        held = activity.reason(candidate)
        if held:
            return held
        minutes = (
            self._build_minutes()
            if candidate.category == "build"
            else (30 if candidate.category == "trash" else 7) * 1440
        )
        if newest > time.time() - minutes * 60:
            return f"written in the last {minutes} minutes"
        if candidate.checkout:
            relative = str(candidate.path.relative_to(candidate.checkout))
            # check-ignore's status 1 is normal but cannot authorize deletion.
            ignored = self._command(
                ["git", "check-ignore", "--", relative],
                cwd=candidate.checkout,
                timeout=30,
                allowed=(0, 1),
            )
            if not ignored.strip():
                return "not ignored"
            if self._command(
                ["git", "ls-files", "--", relative],
                cwd=candidate.checkout,
                timeout=30,
            ):
                return "tracked content"
        return None

    def _builds(self) -> list[Candidate]:
        candidates = []
        derived = self.home / "Library/Developer/Xcode/DerivedData"
        pending = [(self.src, True), (derived, False)]
        deadline = time.time() + 30
        inspected = 0
        while pending:
            parent, recursive = pending.pop()
            self._check_time()
            if time.time() >= deadline:
                self._failure("build discovery exceeded 30 seconds")
                return candidates
            if parent.is_symlink() or parent.resolve() != parent:
                continue
            try:
                with os.scandir(parent) as entries:
                    for entry in entries:
                        self._check_time()
                        inspected += 1
                        if inspected > DISCOVERY_MAX_ENTRIES or time.time() >= deadline:
                            self._failure("build discovery exceeded 30 seconds or 100000 entries")
                            return candidates
                        if entry.name == ".git" or not entry.is_dir(follow_symlinks=False):
                            continue
                        path = Path(entry.path)
                        if not recursive:
                            # Never promote an emptied project to a whole candidate:
                            # later runs must still preserve its package source.
                            outputs = (
                                [path]
                                if path.name in XCODE_CACHE_NAMES
                                else [path / "Build", path / "Index.noindex"]
                            )
                            candidates.extend(
                                Candidate(output, "build", _owning_checkout(output))
                                for output in outputs
                                if output.is_dir()
                            )
                        elif path.name in BUILD_NAMES and not (path / ".git").exists():
                            checkout = _owning_checkout(path)
                            if checkout:
                                candidates.append(Candidate(path, "build", checkout))
                        else:
                            pending.append((path, True))
            except FileNotFoundError:
                continue
            except OSError as exc:
                self._failure(f"discovery {parent}: {exc}")
        return candidates

    def _candidates(self) -> list[Candidate]:
        candidates = self._builds()
        for path in self.lf_home.glob("runs/*/*"):
            if path.is_dir():
                candidates.append(Candidate(path, "captures"))
        for name in ("logs", "log"):
            root = self.lf_home / name
            if root.exists():
                for parent, dirs, files in os.walk(root, onerror=_raise_walk_error):
                    dirs[:] = [name for name in dirs if not (Path(parent) / name).is_symlink()]
                    candidates.extend(
                        Candidate(Path(parent) / name, "logs")
                        for name in files
                        if Path(parent) / name != self.disk_log
                    )
        copies = self.lf_home / "copies"
        if copies.exists():
            candidates.extend(
                Candidate(path, "copies") for path in copies.iterdir() if path.is_dir()
            )
        backups: dict[str, list[Path]] = {}
        for path in self.lf_home.glob("*.db.*"):
            if (
                path.is_file()
                and not path.is_symlink()
                and re.search(r"\.db\.(backup|bak|pre)", path.name)
                and not path.name.endswith(("-wal", "-shm", ".lock"))
            ):
                backups.setdefault(path.name.split(".db.")[0], []).append(path)
        for paths in backups.values():
            ordered = sorted(paths, key=lambda path: path.lstat().st_mtime, reverse=True)
            for path in ordered[1:]:
                companions = tuple(
                    Path(f"{path}{suffix}")
                    for suffix in ("-wal", "-shm")
                    if Path(f"{path}{suffix}").exists()
                )
                candidates.append(Candidate(path, "backups", companions=companions))
        for relative, category in (("Library/Caches", "cache"), (".Trash", "trash")):
            root = self.home / relative
            if root.exists():
                try:
                    candidates.extend(Candidate(path, category) for path in root.iterdir())
                except OSError as exc:
                    self._failure(f"{root}: {exc}")
        return candidates

    def _inventory(self, activity: Activity) -> list[tuple[Candidate, int, str | None]]:
        measured = []
        pending = self._candidates()
        while pending:
            candidate = pending.pop()
            try:
                allocated, newest = self._measure(candidate)
                reason = self._reason(candidate, newest, activity)
                # Cargo profiles are independent units. A live release binary
                # must not keep an unused debug profile indefinitely.
                if reason and candidate.checkout and candidate.path.name == "target":
                    profiles = [candidate.path / name for name in ("debug", "release")]
                    pending.extend(
                        Candidate(path, "build", candidate.checkout)
                        for path in profiles
                        if path.is_dir()
                    )
                measured.append((candidate, allocated, reason))
                self._note(
                    f"candidate {candidate.category} {allocated} bytes {candidate.path}: "
                    f"{reason or 'eligible'}"
                )
            except ValueError as exc:
                self._note(f"kept {candidate.path} ({exc})")
                self.entries.append(
                    {"path": str(candidate.path), "status": "retained", "reason": str(exc)}
                )
            except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
                self._failure(f"inspection {candidate.path}: {exc}")
        return sorted(measured, key=lambda item: -item[1])

    def _remove(self, candidate: Candidate, allocated: int) -> None:
        self._command(["/bin/rm", "-rf", "--", *map(str, candidate.paths)])
        self.removed_bytes += allocated
        line = (
            f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} "
            f"removed {candidate.category} {allocated} bytes {candidate.path}\n"
        )
        try:
            self.disk_log.parent.mkdir(parents=True, exist_ok=True)
            with self.disk_log.open("a") as output:
                output.write(line)
        except OSError as exc:
            self._failure(f"cannot append {self.disk_log}: {exc}")
        self._note(line.rstrip())

    def _clean(
        self, candidate: Candidate, allocated: int, reason: str | None, activity: Activity
    ) -> None:
        if not candidate.path.exists():
            return
        try:
            # Recheck eligible output before removal. Retained inventory needs
            # no fresh authority, and dry runs need no second inspection.
            if not reason and not self.dry_run:
                allocated, newest = self._measure(candidate)
                reason = self._reason(candidate, newest, activity)
                if not reason:
                    reason = self._activity().reason(candidate)
            if reason or self.dry_run:
                status = "retained" if reason else "eligible"
                self._note(f"kept {candidate.path} ({reason or 'dry run'})")
            else:
                self._remove(candidate, allocated)
                status = "removed"
        except ValueError as exc:
            status, reason = "retained", str(exc)
            self._note(f"kept {candidate.path} ({reason})")
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
            # A failed removal may be partial; do not credit its allocation.
            status, reason = "failed", str(exc)
            self._failure(f"cleanup {candidate.path}: {exc}")
        self.entries.append(
            {
                "path": str(candidate.path),
                "category": candidate.category,
                "bytes": allocated,
                "status": status,
                "reason": reason,
            }
        )

    def _step(self, name: str, args: list[str], cwd: Path | None = None) -> None:
        if self.dry_run:
            return
        self._note(f"== {name}")
        try:
            output = self._command(args, cwd=cwd)
            if output.strip():
                self._note(output.strip())
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
            self._failure(f"{name}: {exc}")

    def run(self) -> int:
        started = time.time()
        before = shutil.disk_usage(self.home).free
        try:
            for log in (
                self.log,
                self.disk_log,
                self.log.with_name("loopflow-disk-cleanup-launchd.log"),
            ):
                try:
                    if not self.dry_run and log.exists() and log.stat().st_size > 10 * 1024 * 1024:
                        with log.open("rb+") as output:
                            output.seek(-5 * 1024 * 1024, os.SEEK_END)
                            tail = output.read()
                            output.seek(0)
                            output.write(tail)
                            output.truncate()
                except OSError as exc:
                    self._failure(f"cannot rotate {log}: {exc}")
            self._note(
                f"disk cleanup: {before} bytes free; "
                f"build retention {self._build_minutes()} minutes; dry_run={self.dry_run}"
            )
            activity = self._activity()
            for candidate, allocated, reason in self._inventory(activity):
                self._clean(candidate, allocated, reason, activity)
            # Slow native/network maintenance cannot starve generated-output cleanup.
            self._step("lf monitor prune", [str(self.lf), "monitor", "prune"])
            if self.src.exists():
                for repo in sorted(self.src.iterdir()):
                    if repo.is_symlink() or not (repo / ".git").is_dir():
                        continue
                    self._step(f"repository {repo.name}", [str(self.lf), "wt", "prune"], repo)
                    self._step(
                        f"branches {repo.name}", [str(self.lf), "wt", "prune", "--branches"], repo
                    )
            if shutil.which("uv", path=self.env.get("PATH")):
                self._step("uv cache", ["uv", "cache", "prune"])
            if shutil.which("xcrun", path=self.env.get("PATH")):
                self._step("simulators", ["xcrun", "simctl", "delete", "unavailable"])
        except (
            OSError,
            RuntimeError,
            subprocess.TimeoutExpired,
            CleanupStopped,
        ) as exc:
            signal.setitimer(signal.ITIMER_REAL, 0)
            self._failure(str(exc))
        signal.setitimer(signal.ITIMER_REAL, 0)
        after = shutil.disk_usage(self.home).free
        self._note(
            f"{'incomplete' if self.failures else 'done'}: {after} bytes free, "
            f"delta {after - before}; removed {self.removed_bytes} allocated bytes; "
            f"failures={len(self.failures)}"
        )
        receipt = {
            "started_free_bytes": before,
            "finished_free_bytes": after,
            "free_delta_bytes": after - before,
            "target_shortfall_bytes": max(0, self.policy.cleanup_target_free_disk_bytes - after),
            "removed_allocated_bytes": self.removed_bytes,
            "dry_run": self.dry_run,
            "failures": self.failures,
            "candidates": self.entries,
            "started_at": started,
            "finished_at": time.time(),
            "exit_code": int(bool(self.failures)),
        }
        target = self.log.with_suffix(".json")
        temp = target.with_suffix(".json.tmp")
        temp.write_text(json.dumps(receipt, indent=2) + "\n")
        temp.replace(target)
        return receipt["exit_code"]


def _raise_walk_error(error: OSError) -> None:
    raise error


def _interrupt(signum: int, frame: object) -> None:
    raise CleanupStopped(f"cleanup interrupted by signal {signum}")


def _expire(signum: int, frame: object) -> None:
    raise CleanupStopped("cleanup wall-clock budget exhausted")


def _schedule(action: str) -> int:
    home = Path.home()
    target = home / "Library/LaunchAgents" / f"{LABEL}.plist"
    service = f"gui/{os.getuid()}/{LABEL}"
    state = subprocess.run(
        ["launchctl", "print", service], capture_output=True, text=True, timeout=15
    )
    if state.returncode == 0 and re.search(r"\bpid = \d+", state.stdout):
        raise RuntimeError("cleanup is running; leave its job in place until it exits")
    if state.returncode != 0 and "Could not find service" not in state.stderr:
        raise RuntimeError(f"cannot inspect launchd job: {state.stderr.strip()}")
    if action == "install":
        common = subprocess.check_output(
            [
                "git",
                "-C",
                str(Path(__file__).parent),
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ],
            text=True,
            timeout=15,
        ).strip()
        main = Path(common).parent
        if not (main / "scripts/disk_cleanup.py").is_file():
            raise RuntimeError("land the mechanical runner in the main checkout before installing")
        log = home / "Library/Logs/loopflow-disk-cleanup-launchd.log"
        log.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "Label": LABEL,
            "ProgramArguments": [
                "/usr/bin/env",
                "-i",
                f"HOME={home}",
                f"USER={os.environ.get('USER', '')}",
                f"PATH={home}/.local/bin:{home}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                f"LF_HOME={os.environ.get('LF_HOME', home / '.lf')}",
                f"LOOPFLOW_SRC={os.environ.get('LOOPFLOW_SRC', home / 'src')}",
                "TMPDIR=/tmp",
                "LANG=en_US.UTF-8",
                "/bin/bash",
                str(main / "scripts/disk-cleanup.sh"),
                "run",
            ],
            "WorkingDirectory": str(main),
            "StartCalendarInterval": {"Minute": 17},
            "ProcessType": "Background",
            "StandardOutPath": str(log),
            "StandardErrorPath": str(log),
        }
        target.parent.mkdir(parents=True, exist_ok=True)
        previous = target.read_bytes() if target.exists() else None
        temp = target.with_suffix(".plist.tmp")
        temp.write_bytes(plistlib.dumps(payload))
        if state.returncode == 0:
            subprocess.run(["launchctl", "bootout", service], check=True, timeout=15)
        temp.replace(target)
        try:
            subprocess.run(
                ["launchctl", "bootstrap", f"gui/{os.getuid()}", str(target)],
                check=True,
                timeout=15,
            )
        except subprocess.SubprocessError:
            if previous is None:
                target.unlink()
            else:
                target.write_bytes(previous)
                if state.returncode == 0:
                    subprocess.run(
                        ["launchctl", "bootstrap", f"gui/{os.getuid()}", str(target)],
                        check=True,
                        timeout=15,
                    )
            raise
    elif state.returncode == 0:
        subprocess.run(["launchctl", "bootout", service], check=True, timeout=15)
    if action == "uninstall":
        target.unlink(missing_ok=True)
    # Remove only the predecessor's marked entry after launchd is ready.
    cron = subprocess.run(["crontab", "-l"], capture_output=True, text=True, timeout=15)
    if cron.returncode and "no crontab" not in cron.stderr.lower():
        raise RuntimeError(f"cannot inspect old crontab: {cron.stderr.strip()}")
    if MARKER in cron.stdout:
        remaining = "".join(
            line for line in cron.stdout.splitlines(keepends=True) if MARKER not in line
        )
        subprocess.run(["crontab", "-"], input=remaining, text=True, check=True, timeout=15)
    print(f"{action}: {target}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", nargs="?", choices=("run", "install", "uninstall"), default="run")
    parser.add_argument("--dry-run", action="store_true", help="inventory without deletion")
    parser.add_argument("--budget-seconds", type=float, default=1800, help="total wall-clock limit")
    args = parser.parse_args()
    if args.budget_seconds <= 0:
        parser.error("--budget-seconds must be positive")
    try:
        if args.action != "run":
            if args.dry_run:
                parser.error("--dry-run applies to run only")
            return _schedule(args.action)
        lock = _lock_recovery()
        if lock is None:
            print("skipped: another cleanup holds the resource recovery lock", flush=True)
            return 0
        with lock:
            signal.signal(signal.SIGTERM, _interrupt)
            signal.signal(signal.SIGINT, _interrupt)
            signal.signal(signal.SIGALRM, _expire)
            signal.setitimer(signal.ITIMER_REAL, args.budget_seconds)
            try:
                return Cleaner(args.dry_run, args.budget_seconds).run()
            finally:
                signal.setitimer(signal.ITIMER_REAL, 0)
    except (OSError, RuntimeError, subprocess.SubprocessError, CleanupStopped) as exc:
        print(f"disk cleanup failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
