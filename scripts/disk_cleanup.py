#!/usr/bin/env python3
"""Opt-in machine maintenance for disposable coding output, with private accounting."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from collections import deque
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from cleanup_evidence import evidence_outcome
from resource_envelope import _lock_recovery, load_policy

LABEL = "com.loopflow.disk-cleanup"
MARKER = "# loopflow:disk-cleanup"
BUILD_NAMES = {"target", "node_modules", ".venv", ".build", "DerivedData"}
XCODE_CACHE_NAMES = {"ModuleCache.noindex", "SDKStatCaches.noindex", "CompilationCache.noindex"}
DISCOVERY_MAX_ENTRIES = 100_000
DISCOVERY_SECONDS = 120
OWNER_GRACE_SECONDS = 5
HISTORY_LIMIT = 168
HISTORY_BYTES = 4 * 1024 * 1024
RETENTIONS = {"keep_forever", "emergency_reducible", "mixed", "unclassified", "project_owned"}
CONTENT_TYPES = {"code/development", "media", "entertainment", "desktop apps", "system/other"}
EVIDENCE_CATEGORIES = {"pytest", "coding-temp", "demo", "test-evidence"}
TEMP_PREFIXES = ("lf-worlds-", "ghostty-shaders-", "swift-test-")
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


@dataclass(frozen=True)
class Process:
    pid: int
    parent: int
    uid: int
    started: str
    command: str

    @property
    def identity(self) -> tuple[int, int, str, str]:
        return self.pid, self.uid, self.started, self.command


@dataclass(frozen=True)
class OpenPath:
    pid: int
    descriptor: str
    path: Path


@dataclass(frozen=True)
class Activity:
    processes: dict[int, Process]
    files: tuple[OpenPath, ...]

    def owners(self, candidate: Candidate) -> set[int]:
        return {file.pid for file in self.files if _within(file.path, candidate.path)}

    def reason(self, candidate: Candidate) -> str | None:
        if self.owners(candidate):
            return "held open"
        checkout = candidate.checkout
        if checkout and any(
            file.descriptor == "cwd"
            and _within(file.path, checkout)
            and file.pid in self.processes
            and BUILDERS.match(Path(self.processes[file.pid].command).name)
            for file in self.files
        ):
            return "builder in checkout"
        return None


def _within(path: Path, root: Path) -> bool:
    return path.parts[: len(root.parts)] == root.parts


def _owning_checkout(path: Path) -> Path | None:
    return next((parent for parent in (path, *path.parents) if (parent / ".git").exists()), None)


def _temporary_roots() -> set[Path]:
    roots = {Path(tempfile.gettempdir()).resolve(), Path("/tmp").resolve()}
    if sys.platform == "darwin":
        roots.add(
            Path(
                subprocess.check_output(
                    ["getconf", "DARWIN_USER_TEMP_DIR"], text=True, timeout=5
                ).strip()
            ).resolve()
        )
    return roots


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
        self.state = self.home / ".lf-machine/maintenance/disk-cleanup"
        config = self.state / "config.json"
        self.config = json.loads(config.read_text()) if config.exists() else {}
        self.usage_labels = {}
        self.uv_caches: set[Path] = set()
        self.cache_caps = {
            "uv_bytes": 8 * 1024**3,
            "sccache_bytes": 10 * 1024**3,
            "docker_build_bytes": 8 * 1024**3,
            **self.config.get("cache_caps", {}),
        }
        if any(type(cap) is not int or cap <= 0 for cap in self.cache_caps.values()):
            raise ValueError("cache caps must be positive integer byte counts")
        for entry in self.config.get("usage_roots", []):
            if entry["retention"] not in RETENTIONS or entry["content_type"] not in CONTENT_TYPES:
                raise ValueError("invalid usage retention or content type")
            path = Path(entry["path"]).expanduser().resolve()
            self.usage_labels[path] = (entry["retention"], entry["content_type"])
        self.temp_roots = _temporary_roots()
        self.dry_run = dry_run
        self.deadline = time.time() + budget_seconds
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
        self.env["LF_HOME"] = str(self.lf_home)
        self.env["UV_LOCK_TIMEOUT"] = "0"
        self.policy = load_policy()
        self.failures: list[str] = []
        self.entries: list[dict] = []
        self.removed_bytes = 0
        self.stopped: list[dict] = []
        self.maintenance: list[dict] = []
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
        input_text: str | None = None,
    ) -> str:
        self._check_time()
        with subprocess.Popen(
            args,
            cwd=cwd,
            env=self.env,
            stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
            text=True,
        ) as process:
            try:
                stdout, stderr = process.communicate(
                    input=input_text, timeout=min(timeout, self.deadline - time.time())
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

    def _processes(self, pids: set[int] | None = None) -> dict[int, Process]:
        selection = ["-p", ",".join(map(str, sorted(pids)))] if pids else ["-ax"]
        output = self._command(
            ["ps", *selection, "-o", "pid=,ppid=,uid=,lstart=,stat=,comm="],
            allowed=(0, 1) if pids else (0,),
        )
        processes = {}
        for line in output.splitlines():
            fields = line.split(maxsplit=9)
            if len(fields) != 10:
                raise RuntimeError("incomplete process identity")
            pid, parent, uid = map(int, fields[:3])
            if fields[8].startswith("Z"):
                continue
            processes[pid] = Process(pid, parent, uid, " ".join(fields[3:8]), fields[9])
        if not processes and not pids:
            raise RuntimeError("empty process inventory")
        return processes

    def _activity(self) -> Activity:
        try:
            processes = self._processes()
            output = self._command(["lsof", "-nP", "-Fpcfn"])
            files = []
            descriptor = ""
            resolved: dict[str, Path] = {}
            pid = None
            for line in output.splitlines():
                if line.startswith("p"):
                    pid = int(line[1:])
                    descriptor = ""
                elif line.startswith("f"):
                    descriptor = line[1:]
                elif line.startswith("n/"):
                    raw = line[1:]
                    if raw not in resolved:
                        path = Path(raw)
                        parent = str(path.parent)
                        if parent not in resolved:
                            resolved[parent] = path.parent.resolve()
                        path = resolved[parent] / path.name
                        try:
                            resolved[raw] = path.resolve() if path.is_symlink() else path
                        except PermissionError:
                            # Older system Python raises from is_symlink for
                            # protected OS files. Keep lsof's evidence via the
                            # same non-strict resolution used for parent paths.
                            resolved[raw] = path.resolve()
                    path = resolved[raw]
                    if pid is not None:
                        files.append(OpenPath(pid, descriptor, path))
            if not files:
                raise RuntimeError("no paths returned")
            return Activity(processes, tuple(files))
        except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as exc:
            raise CleanupStopped(f"open-file inspection failed; stopping cleanup: {exc}") from exc

    def _measure(self, candidate: Candidate) -> tuple[int, float]:
        allocated = 0
        newest = 0.0
        deadline = time.time() + 30
        root = candidate.path
        if root.is_symlink() or root.resolve() != root:
            raise ValueError("symlink root or ancestor")
        device = root.lstat().st_dev
        pending = [root]
        while pending:
            self._check_time()
            if time.time() >= deadline:
                raise RuntimeError("content inspection exceeded 30 seconds")
            path = pending.pop()
            if path.name == ".cleanup-keep" and candidate.category in EVIDENCE_CATEGORIES:
                raise ValueError("explicitly saved evidence")
            if path.name == ".git" and candidate.category not in EVIDENCE_CATEGORIES:
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

    def _reservation(self, root: Path) -> str | None:
        for path, labels in self.usage_labels.items():
            if labels[0] in {"keep_forever", "project_owned"} and (
                _within(root, path) or _within(path, root)
            ):
                return (
                    "explicit keep forever"
                    if labels[0] == "keep_forever"
                    else "project-owned/deferred"
                )
        return None

    def _reason(self, candidate: Candidate, newest: float, activity: Activity) -> str | None:
        reservation = self._reservation(candidate.path)
        if reservation:
            return reservation
        held = activity.reason(candidate)
        terminable = (
            candidate.category == "build" and candidate.path.name not in {".venv", "node_modules"}
        ) or candidate.category in EVIDENCE_CATEGORIES
        if candidate.category in EVIDENCE_CATEGORIES:
            outcome = evidence_outcome(candidate.path)
            if outcome == "saved":
                return "explicitly saved evidence"
            minutes = 1440 if outcome == "passed" else 7 * 1440
        else:
            minutes = 1440 if held or candidate.category != "build" else self._build_minutes()
        if newest > time.time() - minutes * 60:
            return f"written in the last {minutes} minutes"
        if held and (not terminable or held != "held open"):
            return held
        if candidate.category == "chrome":
            copies = self._chrome_copies(candidate.path.parent)
            if not copies or candidate.path == max(copies, key=lambda p: p.stat().st_mtime):
                return "newest Chrome copy"
            if {path.name for path in candidate.path.iterdir()} != {"Google Chrome.app.bundle"}:
                return "unexpected Chrome copy contents"
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

    def _stop_owners(self, candidate: Candidate, activity: Activity) -> None:
        owners = activity.owners(candidate)
        if not owners:
            return
        processes = activity.processes
        protected = {1, os.getpid()}
        ancestor = os.getpid()
        while ancestor in processes and processes[ancestor].parent not in protected:
            ancestor = processes[ancestor].parent
            protected.add(ancestor)
        selected = set(owners)
        while True:
            children = {p.pid for p in processes.values() if p.parent in selected}
            if children <= selected:
                break
            selected |= children
        for pid in selected:
            process = processes.get(pid)
            if process is None or process.uid != os.getuid() or pid in protected:
                raise RuntimeError(f"cannot verify a stoppable owner: pid {pid}")
            if "chrome" in process.command.lower():
                raise RuntimeError(f"Chrome owner preserved: pid {pid}")
            if any(
                labels[0] == "project_owned"
                and any(file.pid == pid and _within(file.path, path) for file in activity.files)
                for path, labels in self.usage_labels.items()
            ):
                raise RuntimeError(f"owner uses project-owned/deferred state: pid {pid}")
        identities = {pid: processes[pid] for pid in selected}
        # Poll only this verified tree; unrelated machine activity must not make
        # every grace-period check rescan the entire process table.
        for sig in (signal.SIGTERM, signal.SIGKILL):
            current = self._processes(selected)
            for pid, original in identities.items():
                process = current.get(pid)
                if process is None:
                    continue
                if process.identity != original.identity:
                    raise RuntimeError(f"owner identity changed: pid {pid}")
                self._check_time()
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    continue
                self.stopped.append(
                    {
                        "pid": pid,
                        "started": original.started,
                        "signal": sig.name,
                        "path": str(candidate.path),
                    }
                )
            until = time.time() + (OWNER_GRACE_SECONDS if sig == signal.SIGTERM else 1)
            while time.time() < until:
                self._check_time()
                current = self._processes(selected)
                if not any(
                    pid in current and current[pid].identity == original.identity
                    for pid, original in identities.items()
                ):
                    break
                time.sleep(0.1)
        current = self._processes(selected)
        if any(
            pid in current and current[pid].identity == original.identity
            for pid, original in identities.items()
        ):
            raise RuntimeError("owner tree survived teardown")
        if self._activity().owners(candidate):
            raise RuntimeError("output still has owners after teardown")

    def _ignored_directories(self, checkout: Path) -> set[Path]:
        # Discovery collects these roots without walking their contents. Give
        # Git the same boundary, including when output is not gitignored.
        # _reason still requires the checkout's real ignore rules for deletion.
        output = self._command(
            [
                "git",
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
                "-z",
                *(f"--exclude={name}/" for name in sorted(BUILD_NAMES | {".lf"})),
            ],
            cwd=checkout,
            timeout=10,
        )
        directories = [entry for entry in output.split("\0") if entry.endswith("/")]
        if not directories:
            return set()
        # ls-files also reports empty untracked parents. Only actual ignore
        # matches may prune traversal into nested output.
        matches = self._command(
            ["git", "check-ignore", "--stdin", "-z"],
            cwd=checkout,
            timeout=10,
            allowed=(0, 1),
            input_text="\0".join(directories) + "\0",
        )
        return {checkout / path for path in matches.split("\0") if path}

    def _builds(self) -> list[Candidate]:
        candidates = []
        derived = self.home / "Library/Developer/Xcode/DerivedData"
        pending = deque([(self.src, True, _owning_checkout(self.src)), (derived, False, None)])
        started = time.time()
        deadline = started + DISCOVERY_SECONDS
        inspected = 0
        visited: set[Path] = set()
        registered: set[Path] = set()
        ignored_by_checkout: dict[Path, set[Path]] = {}
        while pending:
            parent, recursive, checkout = pending.popleft()
            if parent in visited:
                continue
            visited.add(parent)
            self._check_time()
            if time.time() >= deadline:
                self._failure(f"build discovery exceeded {DISCOVERY_SECONDS} seconds")
                return candidates
            if parent.is_symlink() or parent.resolve() != parent:
                continue
            try:
                ignored: set[Path] = set()
                if checkout:
                    if checkout not in ignored_by_checkout:
                        if checkout not in registered:
                            worktrees = self._command(
                                ["git", "worktree", "list", "--porcelain", "-z"],
                                cwd=checkout,
                                timeout=10,
                            )
                            for field in worktrees.split("\0"):
                                if field.startswith("worktree "):
                                    worktree = Path(field[9:])
                                    registered.add(worktree)
                                    if _within(worktree, self.src):
                                        pending.append((worktree, True, worktree))
                        ignored_by_checkout[checkout] = self._ignored_directories(checkout)
                        gate = checkout / ".lf/tmp/gate"
                        candidates.extend(
                            Candidate(path, "test-evidence", checkout)
                            for path in gate.glob("run-*")
                            if path.is_dir()
                        )
                    ignored = ignored_by_checkout[checkout]
                with os.scandir(parent) as entries:
                    for entry in entries:
                        self._check_time()
                        if entry.name in {".git", ".lf"} or not entry.is_dir(follow_symlinks=False):
                            continue
                        inspected += 1
                        if inspected > DISCOVERY_MAX_ENTRIES or time.time() >= deadline:
                            self._failure(
                                f"build discovery exceeded {DISCOVERY_SECONDS} seconds or "
                                f"{DISCOVERY_MAX_ENTRIES} entries"
                            )
                            return candidates
                        path = Path(entry.path)
                        if path.name == ".uv-cache" and (path / "CACHEDIR.TAG").is_file():
                            self.uv_caches.add(path)
                            continue
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
                            continue
                        nested_checkout = (path / ".git").exists()
                        if path.name in BUILD_NAMES and not nested_checkout:
                            if checkout:
                                if path.name == ".build" and (path / "checkouts").is_dir():
                                    # SwiftPM keeps dependency source beside target
                                    # output. Never make that source a deletion unit.
                                    candidates.extend(
                                        Candidate(output, "build", checkout)
                                        for output in path.iterdir()
                                        if re.match(
                                            r"^(arm64|aarch64|x86_64)-(apple|unknown)-", output.name
                                        )
                                        and output.is_dir()
                                        and not output.is_symlink()
                                    )
                                else:
                                    candidates.append(Candidate(path, "build", checkout))
                        elif path not in ignored or nested_checkout:
                            pending.append((path, True, path if nested_checkout else checkout))
            except FileNotFoundError:
                continue
            except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
                self._failure(f"discovery {parent}: {exc}")
        self._note(
            f"discovered {len(candidates)} candidates across {len(ignored_by_checkout)} checkouts "
            f"and {len(visited)} directories in {time.time() - started:.1f} seconds"
        )
        return candidates

    def _chrome_copies(self, root: Path) -> list[Path]:
        if root.is_symlink() or root.resolve() != root or not root.exists():
            return []
        return [
            path
            for path in root.iterdir()
            if path.name.startswith("code_sign_clone.") and path.is_dir() and not path.is_symlink()
        ]

    def _candidates(self) -> list[Candidate]:
        candidates = self._builds()
        for root in sorted(self.temp_roots):
            if not root.exists():
                continue
            for path in root.iterdir():
                self._check_time()
                if path.is_symlink() or not path.is_dir() or path.stat().st_uid != os.getuid():
                    continue
                if path.name.startswith("pytest-of-"):
                    for run in path.iterdir():
                        if (
                            not run.is_symlink()
                            and run.is_dir()
                            and re.fullmatch(r"(?:pytest-\d+|garbage-[0-9a-f-]+)", run.name)
                        ):
                            candidates.append(Candidate(run, "pytest"))
                elif path.name.startswith(TEMP_PREFIXES):
                    candidates.append(Candidate(path, "coding-temp"))
            # Darwin's X is a sibling of the user's T, never arbitrary /tmp/X.
            if self.config.get("chrome_copies") and root.name == "T":
                chrome = root.parent / "X/com.google.Chrome.code_sign_clone"
                candidates.extend(Candidate(path, "chrome") for path in self._chrome_copies(chrome))
        demos = self.home / ".lf-machine/demos"
        if demos.is_dir() and not demos.is_symlink():
            for path in demos.iterdir():
                # Explicitly marked disposable runtime roots only. Evidence stays
                # outside these roots; ordinary Home/account history is never selected.
                marker = path / ".disposable-coding-demo"
                if (
                    path.is_dir()
                    and not path.is_symlink()
                    and marker.is_file()
                    and not marker.is_symlink()
                    and marker.read_text().strip()
                    == "disposable runtime; evidence stored elsewhere"
                ):
                    candidates.append(Candidate(path, "demo"))
                elif path.is_dir() and not path.is_symlink():
                    self.entries.append(
                        {
                            "path": str(path),
                            "category": "demo",
                            "status": "retained",
                            "bytes": None,
                            "reason": "unmarked demo; preserve source and history",
                        }
                    )
        return candidates

    def _usage(self) -> dict:
        roots = {path: "repository" for path in self.src.iterdir()} if self.src.exists() else {}
        roots.update(
            (self.home / relative, "tooling")
            for relative in (
                ".cache/uv",
                ".cache",
                "Library/Caches/uv",
                "Library/Caches/Mozilla.sccache",
                ".cache/sccache",
                ".cargo",
                ".rustup",
                ".npm",
                ".bun",
                "Library/Developer",
                "Library/Containers/com.docker.docker",
                ".lf",
                ".lf-machine",
                ".local",
                ".lf-machine/demos",
            )
        )
        roots.update((path, "temporary") for path in self.temp_roots)
        roots.update(
            (self.home / relative, "observation-only")
            for relative in (
                "Library/Caches",
                "Library/Application Support",
                "Movies",
                "Music",
                "Pictures",
                "Downloads",
                ".Trash",
            )
        )
        roots.update((path, "configured") for path in self.usage_labels)
        # Rotate bounded inspection across all roots; a large early checkout
        # must not leave the same later repositories permanently unmeasured.
        attempted = {}
        for snapshot in self._usage_history():
            for entry in snapshot["roots"]:
                if entry.get("error") != "usage sampling budget exhausted":
                    attempted[entry["path"]] = snapshot["at"]
        ordered = sorted(
            roots,
            key=lambda path: (
                roots[path] != "configured",
                attempted.get(str(path), 0),
                str(path),
            ),
        )
        samples = []
        deadline = min(self.deadline, time.time() + 120)
        for path in ordered[:256]:
            category = roots[path]
            retention, content_type = self.usage_labels.get(
                path,
                (
                    "unclassified",
                    "code/development" if category in {"repository", "tooling"} else "system/other",
                ),
            )
            entry = {
                "category": category,
                "path": str(path),
                "bytes": None,
                "error": None,
                "retention": retention,
                "content_type": content_type,
            }
            samples.append(entry)
            try:
                if time.time() >= deadline:
                    raise RuntimeError("usage sampling budget exhausted")
                if path.is_symlink() or path.resolve() != path:
                    raise ValueError("symlink root or ancestor")
                if not path.exists():
                    entry["bytes"] = 0
                    continue
                output = self._command(
                    ["du", "-sk", str(path)], timeout=min(15, deadline - time.time())
                )
                entry["bytes"] = int(output.split()[0]) * 1024
            except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
                message = str(exc)
                entry["error"] = message[:2048] + (" [truncated]" if len(message) > 2048 else "")
        return {
            "at": time.time(),
            "roots": samples,
            "truncated": len(roots) > 256,
            "unaccounted_physical_bytes": None,
        }

    def _usage_history(self) -> list[dict]:
        target = self.state / "usage.json"
        if not target.exists():
            return []
        if target.stat().st_size > HISTORY_BYTES:
            raise RuntimeError("usage history exceeds bounded size")
        return json.loads(target.read_text())

    def _record_usage(self, snapshot: dict) -> None:
        self.state.mkdir(parents=True, exist_ok=True, mode=0o700)
        target = self.state / "usage.json"
        history = self._usage_history()
        previous = {}
        for sample in history:
            for entry in sample["roots"]:
                if entry["bytes"] is not None:
                    previous[entry["path"]] = (sample["at"], entry["bytes"])
        for entry in snapshot["roots"]:
            old = previous.get(entry["path"])
            comparable = old is not None and entry["bytes"] is not None
            entry["delta_bytes"] = entry["bytes"] - old[1] if comparable else None
            entry["previous_at"] = old[0] if comparable else None
        snapshot["previous_at"] = history[-1]["at"] if history else None
        history = [*history[-(HISTORY_LIMIT - 1) :], snapshot]
        encoded = json.dumps(history)
        while len(encoded.encode()) + 1 > HISTORY_BYTES and len(history) > 1:
            history.pop(0)
            encoded = json.dumps(history)
        if len(encoded.encode()) + 1 > HISTORY_BYTES:
            raise RuntimeError("usage snapshot exceeds bounded size")
        temp = target.with_suffix(".tmp")
        temp.write_text(encoded + "\n")
        temp.chmod(0o600)
        temp.replace(target)

    def _candidate_entry(
        self,
        candidate: Candidate,
        allocated: int | None,
        status: str,
        reason: str | None,
        activity: Activity,
    ) -> dict:
        return {
            "path": str(candidate.path),
            "category": candidate.category,
            "bytes": allocated,
            "status": status,
            "reason": reason,
            "owner_pids": sorted(activity.owners(candidate)),
            "evidence_outcome": evidence_outcome(candidate.path)
            if candidate.category in EVIDENCE_CATEGORIES
            else None,
        }

    def _inventory(self, activity: Activity) -> list[tuple[Candidate, int]]:
        eligible = []
        pending = self._candidates()
        while pending:
            candidate = pending.pop()
            try:
                # A protected or recently written root already proves retention.
                # Only potential removals need a complete content walk.
                newest = candidate.path.lstat().st_mtime
                reason = self._reason(candidate, newest, activity)
                allocated = None
                if not reason:
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
                if reason:
                    self.entries.append(
                        self._candidate_entry(candidate, allocated, "retained", reason, activity)
                    )
                else:
                    eligible.append((candidate, allocated))
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
                if isinstance(exc, FileNotFoundError) or not candidate.path.exists():
                    self.entries.append(
                        {
                            "path": str(candidate.path),
                            "status": "retained",
                            "reason": "changed during inspection",
                        }
                    )
                else:
                    self._failure(f"inspection {candidate.path}: {exc}")
        return sorted(eligible, key=lambda item: -item[1])

    def _remove(self, candidate: Candidate, allocated: int) -> None:
        self._command(["/bin/rm", "-rf", "--", str(candidate.path)])
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

    def _clean(self, candidate: Candidate, allocated: int, activity: Activity) -> None:
        if not candidate.path.exists():
            return
        # Record the observed outcome before deletion removes its evidence.
        entry = self._candidate_entry(candidate, allocated, "eligible", None, activity)
        reason = None
        try:
            # Only eligible output reaches cleanup; refresh authority before removal.
            if not self.dry_run:
                allocated, newest = self._measure(candidate)
                reason = self._reason(candidate, newest, activity)
                if not reason:
                    fresh = self._activity()
                    reason = self._reason(candidate, newest, fresh)
                    if not reason and fresh.owners(candidate):
                        self._stop_owners(candidate, fresh)
                        allocated, newest = self._measure(candidate)
                        fresh = self._activity()
                        if fresh.owners(candidate):
                            raise RuntimeError("owner appeared before removal")
                        reason = self._reason(candidate, newest, fresh)
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
        entry.update(bytes=allocated, status=status, reason=reason)
        self.entries.append(entry)

    def _step(self, name: str, args: list[str]) -> dict:
        self._note(f"== {name}")
        entry = {"step": name, "status": "completed", "reason": None}
        try:
            output = self._command(args)
            if output.strip():
                self._note(output.strip())
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
            reason = str(exc)
            if (
                name == "uv cache"
                and "Cache is currently in-use" in reason
                and "Timeout (0s)" in reason
            ):
                self._note("kept uv cache (in use; retry next run)")
                entry.update(status="retained", reason=reason)
            else:
                entry.update(status="failed", reason=reason)
                self._failure(f"{name}: {exc}")
        self.maintenance.append(entry)
        return entry

    def _allocated(self, path: Path) -> int:
        if not path.exists():
            return 0
        return int(self._command(["du", "-sk", str(path)], timeout=60).split()[0]) * 1024

    def _prune_uv(self) -> None:
        try:
            default = Path(self._command(["uv", "cache", "dir"]).strip())
            if not default.is_absolute():
                raise RuntimeError("uv returned no absolute cache directory")
            self.uv_caches.add(default.resolve())
            for path in sorted(self.uv_caches):
                if self._reservation(path):
                    self.maintenance.append(
                        {
                            "step": "uv cache",
                            "status": "retained",
                            "reason": "explicit reservation",
                            "path": str(path),
                        }
                    )
                    continue
                cap = self.cache_caps["uv_bytes"]
                # Let the native lock reject a busy cache before walking it.
                # Each stronger operation is needed only while above the cap.
                for operation in (("prune",), ("prune", "--ci"), ("clean",)):
                    entry = self._step(
                        "uv cache", ["uv", "cache", *operation, "--cache-dir", str(path)]
                    )
                    entry.update(path=str(path), limit_bytes=cap)
                    if entry["status"] != "completed":
                        break
                    after = self._allocated(path)
                    entry.update(bytes_after=after, excess_bytes=max(0, after - cap))
                    if after <= cap:
                        break
        except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
            self._failure(f"uv cache: {exc}")

    def _check_sccache(self) -> None:
        try:
            if not any(
                Path(process.command).name == "sccache" for process in self._processes().values()
            ):
                self.maintenance.append(
                    {"step": "sccache", "status": "retained", "reason": "server not running"}
                )
                return
            stats = json.loads(self._command(["sccache", "--show-stats", "--stats-format", "json"]))
            limit = stats.get("max_cache_size")
            if not isinstance(limit, int) or limit > self.cache_caps["sccache_bytes"]:
                raise RuntimeError("native sccache limit exceeds the configured cap or is unknown")
            self.maintenance.append(
                {
                    "step": "sccache",
                    "status": "bounded",
                    "reason": None,
                    "bytes": stats.get("cache_size"),
                    "limit_bytes": limit,
                }
            )
        except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
            self._failure(f"sccache cap: {exc}")

    def _prune_docker(self) -> None:
        storage = (self.home / "Library/Containers/com.docker.docker", self.home / ".docker")
        if any(self._reservation(root) for root in storage):
            self.maintenance.append(
                {"step": "docker", "status": "retained", "reason": "explicit reservation"}
            )
            return
        try:
            context = self._command(["docker", "context", "show"]).strip()
            host = json.loads(
                self._command(
                    [
                        "docker",
                        "context",
                        "inspect",
                        context,
                        "--format",
                        "{{json .Endpoints.docker.Host}}",
                    ]
                )
            )
            if not isinstance(host, str) or not host.startswith("unix://"):
                self.maintenance.append(
                    {"step": "docker", "status": "retained", "reason": "not a local Unix engine"}
                )
                return
            docker = ["docker", "--context", context]
            builder = self._command([*docker, "buildx", "inspect", context])
            if re.search(r"^Driver:\s+docker$", builder, re.MULTILINE) and re.search(
                r"^Endpoint:\s+" + re.escape(context) + r"$", builder, re.MULTILINE
            ):
                self._step(
                    "docker build cache",
                    [
                        *docker,
                        "buildx",
                        "prune",
                        "--builder",
                        context,
                        "--filter",
                        "until=24h",
                        "--max-used-space",
                        str(self.cache_caps["docker_build_bytes"]),
                        "--force",
                    ],
                )
            else:
                self.maintenance.append(
                    {
                        "step": "docker build cache",
                        "status": "retained",
                        "reason": "builder is not the local engine",
                    }
                )
            containers = self._command([*docker, "container", "ls", "-aq"]).split()
            if len(containers) > 256:
                raise RuntimeError("Docker container inventory exceeds 256 entries")
            used = (
                set(
                    self._command(
                        [*docker, "container", "inspect", "--format", "{{.Image}}", *containers]
                    ).split()
                )
                if containers
                else set()
            )
            images = sorted(
                set(self._command([*docker, "image", "ls", "-q", "--no-trunc"]).split())
            )
            if len(images) > 256:
                raise RuntimeError("Docker image inventory exceeds 256 entries")
            for identifier in images:
                metadata = json.loads(
                    self._command(
                        [*docker, "image", "inspect", "--format", "{{json .}}", identifier]
                    )
                )
                image_id = metadata["Id"]
                created = metadata.get("Created")
                if not created:
                    reason = "unknown creation time"
                else:
                    created = re.sub(r"(\.\d{6})\d+(?=Z|[+-])", r"\1", created)
                    age = (
                        datetime.now(timezone.utc)
                        - datetime.fromisoformat(created.replace("Z", "+00:00"))
                    ).total_seconds()
                    reason = (
                        "in use, recent, or no registry digest"
                        if image_id in used or not metadata.get("RepoDigests") or age < 86400
                        else None
                    )
                if reason:
                    self.maintenance.append(
                        {
                            "step": "docker image",
                            "image": image_id,
                            "status": "retained",
                            "reason": reason,
                        }
                    )
                    continue
                # Non-forced image removal also refuses a new container reference.
                entry = self._step("docker image", [*docker, "image", "rm", image_id])
                entry.update(image=image_id, created=created, registry_backed=True)
        except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
            if "Cannot connect to the Docker daemon" in str(
                exc
            ) or "Is the docker daemon running" in str(exc):
                self.maintenance.append(
                    {"step": "docker", "status": "retained", "reason": "local engine not running"}
                )
            else:
                self._failure(f"Docker maintenance: {exc}")

    def run(self) -> int:
        started = time.time()
        before = shutil.disk_usage(self.home).free
        usage = None
        self._note("sampling usage (at most two minutes)")
        try:
            usage = self._usage()
            self._record_usage(usage)
        except (
            OSError,
            ValueError,
            RuntimeError,
            subprocess.TimeoutExpired,
            CleanupStopped,
        ) as exc:
            self._failure(f"usage accounting: {exc}")
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
            for candidate, allocated in self._inventory(activity):
                self._clean(candidate, allocated, activity)
            if not self.dry_run:
                for tool, operation in (
                    ("uv", self._prune_uv),
                    ("sccache", self._check_sccache),
                    ("docker", self._prune_docker),
                ):
                    if shutil.which(tool, path=self.env.get("PATH")):
                        operation()
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
            "stopped_processes": self.stopped,
            "maintenance": self.maintenance,
            "usage": usage,
            "started_at": started,
            "finished_at": time.time(),
            "exit_code": int(bool(self.failures)),
        }
        target = self.log.with_suffix(".json")
        temp = target.with_suffix(".json.tmp")
        temp.write_text(json.dumps(receipt, indent=2) + "\n")
        temp.replace(target)
        return receipt["exit_code"]


def _interrupt(signum: int, frame: object) -> None:
    raise CleanupStopped(f"cleanup interrupted by signal {signum}")


def _expire(signum: int, frame: object) -> None:
    raise CleanupStopped("cleanup wall-clock budget exhausted")


def _schedule(action: str, chrome_copies: bool = False) -> int:
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
        state_root = home / ".lf-machine/maintenance/disk-cleanup"
        state_root.mkdir(parents=True, exist_ok=True, mode=0o700)
        source = Path(__file__).parent
        names = (
            "disk-cleanup.sh",
            "disk_cleanup.py",
            "cleanup_evidence.py",
            "resource_envelope.py",
            "performance-budgets.json",
        )
        contents = {name: (source / name).read_bytes() for name in names}
        digest = hashlib.sha256(b"".join(contents.values())).hexdigest()[:16]
        main = state_root / "versions" / digest
        if not main.exists():
            main.parent.mkdir(exist_ok=True)
            with tempfile.TemporaryDirectory(dir=main.parent) as staging:
                scripts = Path(staging) / "scripts"
                scripts.mkdir()
                for name, content in contents.items():
                    (scripts / name).write_bytes(content)
                Path(staging).rename(main)
        config_path = state_root / "config.json"
        old_config = config_path.read_bytes() if config_path.exists() else None
        config = json.loads(old_config) if old_config else {}
        config.update(
            enabled=True, chrome_copies=chrome_copies or config.get("chrome_copies", False)
        )
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
            # Darwin-background throttling can starve ps/lsof past their safety
            # deadlines under build load. Standard retains light resource limits.
            "ProcessType": "Standard",
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
        config_path.write_text(json.dumps(config) + "\n")
        config_path.chmod(0o600)
        try:
            subprocess.run(
                ["launchctl", "bootstrap", f"gui/{os.getuid()}", str(target)],
                check=True,
                timeout=15,
            )
        except subprocess.SubprocessError:
            if old_config is None:
                config_path.unlink(missing_ok=True)
            else:
                config_path.write_bytes(old_config)
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
        config_path = home / ".lf-machine/maintenance/disk-cleanup/config.json"
        if config_path.exists():
            config = json.loads(config_path.read_text())
            config["enabled"] = False
            config_path.write_text(json.dumps(config) + "\n")
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
    parser.add_argument(
        "--chrome-copies",
        action="store_true",
        help="install: opt in to stale unused Chrome signing-copy cleanup",
    )
    parser.add_argument("--dry-run", action="store_true", help="inventory without deletion")
    parser.add_argument("--budget-seconds", type=float, default=1800, help="total wall-clock limit")
    args = parser.parse_args()
    os.umask(0o077)
    if args.chrome_copies and args.action != "install":
        parser.error("--chrome-copies applies to install only")
    if args.budget_seconds <= 0:
        parser.error("--budget-seconds must be positive")
    try:
        if args.action != "run":
            if args.dry_run:
                parser.error("--dry-run applies to run only")
            lock = _lock_recovery()
            if lock is None:
                raise RuntimeError("cleanup is running; retry installation after it exits")
            with lock:
                return _schedule(args.action, args.chrome_copies)
        config = Path.home() / ".lf-machine/maintenance/disk-cleanup/config.json"
        if not args.dry_run and (
            not config.exists() or not json.loads(config.read_text()).get("enabled")
        ):
            raise RuntimeError("cleanup is disabled; explicitly run install first")
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
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, CleanupStopped) as exc:
        print(f"disk cleanup failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
