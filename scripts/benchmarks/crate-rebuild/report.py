"""Keep completed rebuild receipts and render their median/range comparison."""

import argparse
import gzip
import hashlib
import html
import json
import statistics
from pathlib import Path

from run import _catalog_complete, _validate_closure

CASES = {
    "warm_root": "Fresh worktree, warm cache",
    "command_help": "Command help edit",
    "execution": "Execution edit",
    "task_record": "Saved Task record edit",
}


def _table(summary: list[dict], metric: str, scale: float = 1) -> str:
    rows = []
    for case, label in CASES.items():
        cells = []
        for owner in ("base", "candidate"):
            sample = next(r for r in summary if r["label"] == owner and r["case"] == case)
            value = sample[metric]
            cells.append(
                f"<td>{value['median'] / scale:.2f} "
                f"<small>[{value['min'] / scale:.2f}–{value['max'] / scale:.2f}]</small></td>"
            )
        rows.append(f"<tr><th>{label}</th>{''.join(cells)}</tr>")
    return (
        "<table><thead><tr><th>Case</th><th>Baseline</th><th>Candidate</th></tr></thead>"
        f"<tbody>{''.join(rows)}</tbody></table>"
    )


def _test_loop(source: Path, output: Path) -> None:
    from test_loop import CASES, summarize, validate

    config = json.loads((source / "configuration.json").read_text())
    receipts = json.loads((source / "receipts.json").read_text())
    calibration = config.get("calibration_cohorts", [])
    warm_cohorts = sorted(set(range(1, config["cohorts"] + 1)) - set(calibration))
    validate(receipts, config["cohorts"], calibration)
    summary = summarize([r for r in receipts if r["cohort"] in warm_cohorts])
    totals = []
    for owner, cases in (("app", CASES[:3]), ("execution", CASES[3:])):
        for mode in ("cache", "incremental"):
            values = [
                sum(
                    r["wall_seconds"]
                    for r in receipts
                    if r["cohort"] == cohort and r["mode"] == mode and r["case"] in cases
                )
                for cohort in warm_cohorts
            ]
            totals.append(
                {
                    "owner": owner,
                    "mode": mode,
                    "median": statistics.median(values),
                    "min": min(values),
                    "max": max(values),
                }
            )
    incremental = all(
        next(r["median"] for r in totals if r["owner"] == owner and r["mode"] == "incremental")
        < next(r["median"] for r in totals if r["owner"] == owner and r["mode"] == "cache")
        for owner in ("app", "execution")
    )
    registration = source / "registration" / "stop.json"
    machine = source / "machine-verification.json"
    interruption = source / "host-interruption.json"
    disk_interruption = source / "disk-interruption.json"
    loads = [
        r[key][0]
        for r in receipts
        if r["cohort"] in warm_cohorts
        for key in ("load_before", "load_after")
        if key in r
    ]
    background = {"min_1m": min(loads), "max_1m": max(loads)} if loads else None
    payload = {
        "configuration": config,
        "warm_cohorts": warm_cohorts,
        "machine_verification": json.loads(machine.read_text()) if machine.exists() else None,
        "registration_during_compile": json.loads(registration.read_text())
        if registration.exists()
        else None,
        "background_load": background,
        "host_interruption": json.loads(interruption.read_text())
        if interruption.exists()
        else None,
        "disk_interruption": json.loads(disk_interruption.read_text())
        if disk_interruption.exists()
        else None,
        "abandoned_attempts": {
            str(p.relative_to(source)): json.loads(p.read_text())
            for p in sorted((source / "abandoned-attempts").glob("*/receipts.json"))
        },
        "summary": summary,
        "three_edit_totals": totals,
        "default_incremental": incremental,
        "concurrent_worktree_capacity": None,
        "capacity_note": "Unmeasured; all Cargo commands ran serially",
        "shared_cache_limit": "The shared Machine cache reached its 10 GiB cap during "
        "LOO-921 (2026-11-16). These isolated warm-cache samples measure the cost "
        "of a surviving entry; they do not establish that an entry survives "
        "eviction between worktrees on the shared cache.",
        "method": "Workspace wrapper records all compiler intervals. Cache deltas are "
        "attributed to a compiler only when no other interval overlaps. "
        "Two statistics RPCs per workspace compiler are included in timings. "
        "Test compiler time includes linking. Memory is sampled every second; "
        "sampled peak can miss shorter peaks. Daemon CPU is separate. "
        "Published cache observations count all loaded roots and name the measured worktrees.",
        "measurement_drivers": config.get("drivers"),
        "report_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "receipts": receipts,
    }
    measured_roots = {Path(tree["path"]) for tree in config.get("trees", {}).values()}
    for group in [receipts, *payload["abandoned_attempts"].values()]:
        for receipt in group:
            for field in ("stats_before", "stats_after"):
                stats = receipt.get(field)
                if stats and "basedirs" in stats:
                    roots = stats.pop("basedirs")
                    stats["basedir_count"] = len(roots)
                    stats["measurement_basedirs"] = [
                        root for root in roots if Path(root) in measured_roots
                    ]
    output.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(payload, indent=2) + "\n"
    (output / "loo-921-raw.json.gz").write_bytes(gzip.compress(raw.encode(), mtime=0))
    compact = {
        **payload,
        "raw_receipts": "loo-921-raw.json.gz",
        "receipts": [
            {
                k: v
                for k, v in r.items()
                if k not in ("stats_before", "stats_after", "memory_samples", "cargo_units")
            }
            for r in receipts
        ],
    }
    compact["abandoned_attempts"] = {
        name: [
            {
                key: row[key]
                for key in ("cohort", "mode", "case", "exit_code", "tests_passed", "wall_seconds")
            }
            for row in rows
        ]
        for name, rows in payload["abandoned_attempts"].items()
    }
    (output / "loo-921.json").write_text(json.dumps(compact, indent=2) + "\n")

    def table(metric: str, scale: float = 1) -> str:
        rows = []
        for case in CASES + ["execution_new"]:
            cells = []
            for mode in ("cache", "incremental"):
                row = next((r for r in summary if r["case"] == case and r["mode"] == mode), None)
                value = row[metric] if row else None
                cells.append(
                    f"<td>{value['median'] / scale:.2f} "
                    f"[{value['min'] / scale:.2f}–{value['max'] / scale:.2f}]</td>"
                    if value
                    else "<td>unmeasured</td>"
                )
            rows.append(f"<tr><th>{case.replace('_', ' ')}</th>{''.join(cells)}</tr>")
        return (
            "<table><tr><th>Sample</th><th>Cache</th><th>Incremental</th></tr>"
            + "".join(rows)
            + "</table>"
        )

    totals_html = "".join(
        f"<tr><th>{r['owner']}</th><td>{r['mode']}</td><td>{r['median']:.2f} "
        f"[{r['min']:.2f}–{r['max']:.2f}]</td></tr>"
        for r in totals
    )
    load_note = (
        f"One-minute load ranged from {background['min_1m']:.1f} to "
        f"{background['max_1m']:.1f} across sample boundaries. "
        "These are observations under background work, not a controlled CPU-speed comparison."
        if background
        else "Background load is unmeasured in these fixtures."
    )
    interruption_note = (
        "The execution seed was interrupted when load rose above 400. "
        "Its earlier receipts are retained; the unfinished seed group restarted."
        if payload["host_interruption"]
        else ""
    )
    calibration_note = config.get(
        "calibration_reason", "Both incremental settings were seeded before measurement."
    )
    jobs = config.get("jobs")
    jobs_note = (
        f"Cargo used {html.escape(str(jobs))} jobs."
        if jobs is not None
        else "Cargo used the Machine's default job count."
    )
    stop_note = ""
    if stop := payload["registration_during_compile"]:
        cached = sum(stop["interrupted_rlibs"].values())
        compiler = (
            "The same compiler was running after the daemon exited."
            if stop["compiler_outlived_daemon"]
            else "Compiler survival was unconfirmed."
        )
        stop_note = (
            "<h2>Registration during compilation</h2><p>"
            "Worktree registration and daemon shutdown took "
            f"{stop['registration_and_drain_seconds']:.1f} seconds. {compiler} "
            f"The focused application test {'passed' if stop['test_passed'] else 'failed'}. "
            f"The replacement {'loaded' if stop['new_root_loaded'] else 'did not load'} "
            f"the new root. The disk cache received {cached} of "
            f"{len(stop['interrupted_rlibs'])} interrupted library builds.</p>"
        )
    document = f"""<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width"><title>LOO-921 edit-to-test loop</title>
<style>body{{font:16px/1.5 system-ui;max-width:1050px;margin:48px auto;padding:0
24px;color:#20252b}}
table{{width:100%;border-collapse:collapse;margin:24px 0}}
th,td{{text-align:left;padding:10px;border-bottom:1px solid #ddd}}
th{{background:#f4f5f6}}td{{font-variant-numeric:tabular-nums}}</style>
<h1>A Rust edit through a passing test</h1>
<p>{len(warm_cohorts)} warm alternating cohorts. {jobs_note}
Median [minimum–maximum]. Each sample passes one focused test and includes its link.</p>
<p>Revision <code>{html.escape(config["revision"])}</code>.<br>{html.escape(config["rustc"])}</p>
<p>{html.escape(load_note)} {html.escape(interruption_note)}</p>
<p>{html.escape(calibration_note)}</p>
<p>{html.escape(payload["shared_cache_limit"])}</p>
<h2>Elapsed seconds</h2>{table("wall_seconds")}
<h2>Three edits, elapsed seconds</h2><p>Application includes new-worktree creation and its first
build.
Execution follows the application tests in the same target; the separate execution-new row is
measured
only in cache mode. These totals each contain three semantic edits, including the first build.</p>
<table><tr><th>Owner</th><th>Mode</th><th>Seconds</th></tr>{totals_html}</table>
<p>Measured test-profile default: incremental <strong>{"on" if incremental else "off"}</strong>.
Incremental must win both three-edit totals to become the default.</p>
{stop_note}
<h2>Client CPU-seconds</h2>{table("cpu_seconds")}
<h2>Daemon CPU-seconds (separate)</h2>{table("daemon_cpu_seconds")}
<h2>Largest process RSS, GiB</h2>{table("rss_bytes", 1024**3)}
<h2>Peak sampled process-tree RSS, GiB</h2>{table("tree_rss_bytes", 1024**3)}
<p>{html.escape(payload["method"])} Concurrent-worktree capacity is unmeasured.
Both incremental settings need their own third-party cache keys. The medians include only
complete measured cohorts. The raw file also includes the seed commands for this run.
Each worktree owns its target; all samples use one private cache.
New roots are registered before starting its daemon. Worktree creation is timed separately and added
to the first build; setup overlaps no Cargo command. Registry build scripts and proc macros still
build.</p>
<p><a href="loo-921.json">Compiler closures and command counters</a> ·
<a href="loo-921-raw.json.gz">Raw cache, memory and Cargo unit receipts</a> ·
<a href="../README.md">Method</a></p></html>"""
    (output / "loo-921.html").write_text(document)
    print(output / "loo-921.html")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path)
    parser.add_argument("--test-loop", type=Path)
    parser.add_argument("--cache-validation", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.test_loop:
        _test_loop(args.test_loop.resolve(), args.output.resolve())
        return
    if not args.input or not args.cache_validation:
        parser.error("--input and --cache-validation are required without --test-loop")
    source = args.input.resolve()
    receipts = json.loads((source / "receipts.json").read_text())
    expected = {(label, 0, "cold_seed") for label in ("base", "candidate")} | {
        (label, cohort, case)
        for label in ("base", "candidate")
        for cohort in range(1, 6)
        for case in CASES
    }
    actual = {(r["label"], r["cohort"], r["case"]) for r in receipts}
    if actual != expected or len(receipts) != len(expected):
        raise RuntimeError("five complete paired cohorts and two seeds are required")
    for receipt in receipts:
        _validate_closure(receipt)
    summary = json.loads((source / "summary.json").read_text())
    if len(summary) != 8 or any(row["n"] != 5 for row in summary):
        raise RuntimeError("summary does not contain five samples per case and revision")
    if not _catalog_complete(source):
        raise RuntimeError("catalogue checks are incomplete")
    if not json.loads((args.cache_validation / "passed.json").read_text())["passed"]:
        raise RuntimeError("cache root validation did not pass")
    identities = [json.loads(p.read_text()) for p in sorted(source.glob("*_identity.json"))]
    if len(identities) != 12:
        raise RuntimeError("each source copy needs a runtime identity receipt")
    for identity in identities:
        if identity["observed"] != f"{identity['root']}|{identity['revision']}-dirty":
            raise RuntimeError("runtime identity differs from its source copy")
    config = json.loads((source / "configuration.json").read_text())
    inputs = json.loads((source / "inputs.json").read_text())
    base_packages = {tuple(row) for row in inputs["base"]["registry_packages"]}
    candidate_packages = {tuple(row) for row in inputs["candidate"]["registry_packages"]}
    base = next(r["revision"] for r in receipts if r["label"] == "base")
    candidate = next(r["revision"] for r in receipts if r["label"] == "candidate")
    payload = {
        "base": base,
        "candidate": candidate,
        "configuration": config,
        "validation_driver_sha256": hashlib.sha256(
            Path(__file__).with_name("run.py").read_bytes()
        ).hexdigest(),
        "registry_packages_removed": sorted(base_packages - candidate_packages),
        "registry_packages_added": sorted(candidate_packages - base_packages),
        "summary": summary,
        "receipts": receipts,
        "runtime_identities": identities,
        "source_paths": {
            p.stem: json.loads(p.read_text()) for p in sorted(source.glob("*_source_paths.json"))
        },
        "catalogue": json.loads((source / "catalog-refresh.json").read_text()),
        "cache_root_validation": json.loads((args.cache_validation / "receipts.json").read_text()),
        "validation_resume": "Cargo reports the application as rlib; the validator was corrected "
        "after the first candidate warm-root sample. Its valid timing was retained; "
        "no build repeated.",
    }
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(payload, indent=2) + "\n"
    (output / "loo-650-raw.json.gz").write_bytes(gzip.compress(raw.encode(), mtime=0))
    compact = {**payload, "receipts": []}
    compact["raw_receipts"] = "loo-650-raw.json.gz"
    compact["catalogue"] = [
        {
            "case": row["case"],
            "schema_sha256": row["schema_sha256"],
            "catalogue_sha256": hashlib.sha256(json.dumps(row["catalog"]).encode()).hexdigest(),
            "migration_count": len(row["catalog"]),
            "drafts": [name for name, merged in row["catalog"] if merged == "false"],
        }
        for row in payload["catalogue"]
    ]
    for receipt in receipts:
        row = {
            k: v
            for k, v in receipt.items()
            if k not in ("stats_before", "stats_after", "nonfresh_libraries")
        }
        before, after = receipt["stats_before"], receipt["stats_after"]
        row["non_cacheable_reasons"] = {
            key: value - before["not_cached"].get(key, 0)
            for key, value in after["not_cached"].items()
            if value != before["not_cached"].get(key, 0)
        }
        row["cache_error_counts_unchanged"] = all(
            before[key] == after[key]
            for key in ("cache_errors", "cache_read_errors", "cache_write_errors", "cache_timeouts")
        )
        compact["receipts"].append(row)
    (output / "loo-650.json").write_text(json.dumps(compact, indent=2) + "\n")
    escape = html.escape
    records = {(r["label"], r["case"]): r for r in summary}

    def change(case: str, metric: str) -> float:
        before = records["base", case][metric]["median"]
        after = records["candidate", case][metric]["median"]
        return (after / before - 1) * 100

    document = f"""<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>LOO-650 rebuild measurements</title>
<style>
body {{font:16px/1.5 system-ui,sans-serif;max-width:1050px;margin:48px auto;
padding:0 24px;color:#20252b}}
h1 {{font-size:32px;letter-spacing:-.03em}} h2 {{margin-top:40px;font-size:22px}}
code,td {{font-variant-numeric:tabular-nums}} code {{font-size:.9em}}
table {{width:100%;border-collapse:collapse;margin:16px 0 28px}}
th,td {{padding:10px;border-bottom:1px solid #ddd;text-align:left}}
thead {{background:#f4f5f6}} small {{color:#626970;white-space:nowrap}} a {{color:#145b90}}
</style>
<h1>Rebuild only affected code</h1>
<p>Five alternating paired cohorts; each sample includes the executable link.
Values are <strong>median [minimum–maximum]</strong>. Lower is better.</p>
<p>Command-help elapsed fell {-change("command_help", "wall_seconds"):.1f}%,
execution elapsed fell {-change("execution", "wall_seconds"):.1f}%, and fresh-worktree
elapsed fell {-change("warm_root", "wall_seconds"):.1f}%.
Saved-record edits cost {change("task_record", "wall_seconds"):.1f}% more elapsed
and {change("task_record", "cpu_seconds"):.1f}% more client CPU, while largest RSS
fell {-change("task_record", "rss_bytes"):.1f}%. The cut improves feedback and peak
memory with a small saved-record cost increase.</p>
<p>Baseline <code>{base}</code><br>Candidate <code>{candidate}</code></p>
<h2>Elapsed seconds</h2>{_table(summary, "wall_seconds")}
<h2>Client CPU seconds</h2>{_table(summary, "cpu_seconds")}
<h2>Largest process RSS, GiB</h2>{_table(summary, "rss_bytes", 1024**3)}
<h2>Workspace compiler closure</h2>
<table><tr><th>Edit</th><th>Baseline misses</th><th>Candidate misses</th></tr>
<tr><th>Command help</th><td>application</td><td>application</td></tr>
<tr><th>Execution</th><td>application</td><td>execution + application</td></tr>
<tr><th>Saved record</th><td>application</td><td>storage + execution + application</td></tr></table>
<p>Every fresh candidate worktree retrieves model, definitions, terminal, storage
and execution from its cache. The application compiles its own checkout identity.
Subsequent edit cases reuse unchanged libraries as Cargo-fresh artifacts or cache
hits. Receipts distinguish both from compiler misses. Non-cacheable
third-party build steps and links remain in the whole-build costs above.</p>
<h2>Method and limits</h2>
<p>Independent targets and cold-seeded caches; one Cargo job. Cohort order alternates
candidate/base, base/candidate. Edits have unique cohort suffixes. Two cold seeds
are calibration and are excluded from the medians. Profiles match. The registry dependency delta is
{len(base_packages - candidate_packages)} removed and
{len(candidate_packages - base_packages)} added version/checksum tuples.
Machine: {config["cpus"]} logical CPUs, {escape(config["machine"][4])},
{escape(config["machine"][0])} {escape(config["machine"][2])}.
Compiler: {escape(config["rustc"])}.</p>
<p>The selected experimental sccache binary is pinned by SHA-256
<code>{escape(config["cache_sha256"])}</code>. Installed sccache 0.19.0 lacks the
required Rust basedir and verbatim environment-dependency support. Root upkeep
passed through an observed natural idle exit; no Machine cache configuration or
shared daemon was changed.</p>
<p>The wrapper adds two statistics RPCs per workspace compiler; that overhead is
included. CPU covers the timed client process tree, excluding the cache daemon.
RSS is its largest process, not concurrent Machine memory. These observations do
not establish total Machine CPU savings or a safe concurrency budget. The validator
was resumed after recognizing Cargo’s rlib spelling; completed samples were retained.</p>
<p>All twelve source copies passed runtime root/revision checks. Catalogue checks
retained the canonical schema through Git reference and source-package changes.
Retrieved execution and storage rlibs can retain the seed checkout’s debug-source
paths; use <code>scripts/dev-lf --isolated-build</code> when those paths no longer resolve.</p>
<p><a href="loo-650.json">Compiler, resource and correctness receipts</a> ·
<a href="loo-650-raw.json.gz">Full cache/resource receipts (compressed JSON)</a> ·
<a href="../README.md">Reproduce the comparison</a></p></html>
"""
    (output / "loo-650.html").write_text(document)
    scratch = Path(__file__).resolve().parents[3] / "scratch/measure.html"
    scratch.write_text(
        document.replace(
            'href="loo-650.json"', 'href="../scripts/benchmarks/crate-rebuild/results/loo-650.json"'
        )
        .replace(
            'href="loo-650-raw.json.gz"',
            'href="../scripts/benchmarks/crate-rebuild/results/loo-650-raw.json.gz"',
        )
        .replace('href="../README.md"', 'href="../scripts/benchmarks/crate-rebuild/README.md"')
    )
    print(output / "loo-650.html")


if __name__ == "__main__":
    main()
