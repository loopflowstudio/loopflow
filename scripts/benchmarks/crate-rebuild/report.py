"""Keep completed rebuild receipts and render their median/range comparison."""

import argparse
import gzip
import hashlib
import html
import json
import statistics
from pathlib import Path

from run import CASES, _catalog_complete, _validate_closure

LABELS = {
    "warm_root": "Fresh worktree, warm cache",
    "command_help": "Command help edit",
    "execution": "Execution edit",
    "task_record": "Saved Task record edit",
    "harness_format": "Decoder edit",
    "harness_test": "Decoder edit, owner's test binary",
}


REPORT_STYLE = """<style>
body {font:16px/1.5 system-ui,sans-serif;max-width:1050px;margin:48px auto;
padding:0 24px;color:#20252b}
h1 {font-size:32px;letter-spacing:-.03em} h2 {margin-top:40px;font-size:22px}
code,td {font-variant-numeric:tabular-nums} code {font-size:.9em}
table {width:100%;border-collapse:collapse;margin:16px 0 28px}
th,td {padding:10px;border-bottom:1px solid #ddd;text-align:left}
thead {background:#f4f5f6} small {color:#626970;white-space:nowrap} a {color:#145b90}
</style>"""


def _table(summary: list[dict], metric: str, scale: float = 1) -> str:
    rows = []
    for case in CASES:
        samples = {r["label"]: r[metric] for r in summary if r["case"] == case}
        cells = "".join(
            f"<td>{samples[owner]['median'] / scale:.2f} "
            f"<small>[{samples[owner]['min'] / scale:.2f}–{samples[owner]['max'] / scale:.2f}]"
            "</small></td>"
            for owner in ("base", "candidate")
        )
        base, candidate = samples["base"], samples["candidate"]
        delta = (candidate["median"] - base["median"]) / scale
        # A candidate median inside the baseline's own spread is not a measured change.
        inside = base["min"] <= candidate["median"] <= base["max"]
        note = " <small>inside baseline range</small>" if inside else ""
        rows.append(
            f"<tr><th>{LABELS[case]}</th>{cells}"
            f"<td>{delta:+.2f} ({delta * scale / base['median'] * 100:+.1f}%){note}</td></tr>"
        )
    return (
        "<table><thead><tr><th>Case</th><th>Baseline</th><th>Candidate</th>"
        f"<th>Change</th></tr></thead><tbody>{''.join(rows)}</tbody></table>"
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
    incremental = True
    for owner, cases in (("app", CASES[:3]), ("execution", CASES[3:])):
        medians = {}
        for mode in ("cache", "incremental"):
            values = [
                sum(
                    r["wall_seconds"]
                    for r in receipts
                    if r["cohort"] == cohort and r["mode"] == mode and r["case"] in cases
                )
                for cohort in warm_cohorts
            ]
            medians[mode] = statistics.median(values)
            totals.append(
                {
                    "owner": owner,
                    "mode": mode,
                    "median": medians[mode],
                    "min": min(values),
                    "max": max(values),
                }
            )
        incremental = incremental and medians["incremental"] < medians["cache"]

    def observation(name: str):
        path = source / name
        return json.loads(path.read_text()) if path.exists() else None

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
        "machine_verification": observation("machine-verification.json"),
        "registration_during_compile": observation("registration/stop.json"),
        "background_load": background,
        "host_interruption": observation("host-interruption.json"),
        "disk_interruption": observation("disk-interruption.json"),
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


def _seed_probe(source: Path, output: Path) -> None:
    from seed_probe import ARMS, validate

    config = json.loads((source / "configuration.json").read_text())
    receipts = json.loads((source / "receipts.json").read_text())
    validate(receipts, config["cohorts"])
    steps = {
        "add": "<code>git worktree add</code>",
        "build": "<code>cargo build -p loopflow --bin lf</code>",
        "test": "Focused <code>cargo test -p loopflow --lib</code>",
        "total": "Through the passing test",
    }

    # Each build and test counts units separately. Only costs have a total.
    samples = {arm: [] for arm in ARMS}
    for receipt in receipts:
        add, build, test = (receipt[step] for step in ("add", "build", "test"))
        total = {
            "wall_seconds": add["wall_seconds"] + build["wall_seconds"] + test["wall_seconds"],
            "cpu_seconds": build["cpu_seconds"] + test["cpu_seconds"],
            "written_bytes": build["written_bytes"] + test["written_bytes"],
            "rss_bytes": max(build["rss_bytes"], test["rss_bytes"]),
            "tree_rss_bytes": max(build["tree_rss_bytes"], test["tree_rss_bytes"]),
        }
        samples[receipt["arm"]].append({**receipt, "total": total})

    def table(metric: str, scale: float = 1, digits: int = 1) -> str:
        rows = []
        for step, label in steps.items():
            if metric not in samples["seeded"][0][step]:
                continue
            cells = []
            medians = {}
            for arm in ARMS:
                values = [sample[step][metric] / scale for sample in samples[arm]]
                median = medians[arm] = statistics.median(values)
                cells.append(
                    f"<td>{median:.{digits}f} <small>[{min(values):.{digits}f}–"
                    f"{max(values):.{digits}f}]</small></td>"
                )
            cold = medians["cold"]
            delta = medians["seeded"] - cold
            change = f"{delta:+.{digits}f} ({delta / cold * 100:+.0f}%)" if cold else ""
            rows.append(f"<tr><th>{label}</th>{''.join(cells)}<td>{change}</td></tr>")
        return (
            "<table><thead><tr><th>Step</th><th>Seeded</th><th>Cold</th><th>Change</th>"
            f"</tr></thead><tbody>{''.join(rows)}</tbody></table>"
        )

    seeded = [r for r in receipts if r["seed"]]
    shared = statistics.median(r["still_seeded_bytes"] / r["target_bytes"] for r in seeded)
    loads = [
        r[s][k][0]
        for r in receipts
        for s in ("build", "test")
        for k in ("load_before", "load_after")
    ]
    sources = sorted(
        {(r["seed"]["source"], r["seed"]["commit"], r["seed"]["commits_apart"]) for r in seeded}
    )
    payload = {
        "configuration": config,
        "method": {
            "written_bytes": "Allocated bytes (st_blocks × 512) of regular files under "
            "target/debug that the command created or replaced, by inode and mtime against "
            "a walk before it; hard links count once. In a seeded target every other file "
            "still shares its blocks with the source's clone.",
            "cpu_seconds": "/usr/bin/time user + sys of the Cargo process tree.",
            "rss_bytes": "Largest single process, from /usr/bin/time.",
            "tree_rss_bytes": "Peak of the Cargo process tree, sampled each second.",
            "units": "Lines Cargo -v prints as Compiling and Fresh.",
        },
        "background_load": {"min_1m": min(loads), "max_1m": max(loads)},
        "seed_sources": sources,
        "report_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "receipts": receipts,
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "loo-922.json").write_text(json.dumps(payload, indent=2) + "\n")
    escape = html.escape
    source_rows = "".join(
        f"<li><code>{escape(Path(path).name)}</code> at <code>{escape(commit or '?')}</code>, "
        f"{apart if apart is not None else '?'} commits from the measured revision</li>"
        for path, commit, apart in sources
    )
    document = f"""<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>LOO-922 seeded worktrees</title>
{REPORT_STYLE}
<h1>LOO-922: a new worktree seeded from a sibling's target, against a cold one</h1>
<p>{config["cohorts"]} cohorts alternating which arm goes first. Each sample adds a
worktree through the installed post-checkout hook, builds <code>lf</code>, then runs
<code>{escape(" ".join(config["commands"]["test"]))}</code>. The cold arm removes the target
right after checkout and includes that time in the add. Values are
<strong>median [minimum–maximum]</strong>; lower is better.</p>
<p>Revision <code>{escape(config["revision"])}</code>. The Machine's shared cache and Cargo
jobs. One-minute load average {min(loads):.0f}–{max(loads):.0f} on {config["cpus"]} logical
CPUs from other agents; elapsed time moves with it, CPU-seconds less.</p>
<h2>Elapsed seconds</h2>{table("wall_seconds")}
<h2>CPU-seconds</h2>{table("cpu_seconds")}
<h2>Units Cargo compiled</h2>{table("compiled", digits=0)}
<h2>Units Cargo found fresh</h2>{table("fresh", digits=0)}
<h2>Target bytes written, GiB</h2>{table("written_bytes", 1024**3, 2)}
<p>{escape(payload["method"]["written_bytes"])} The method doesn't depend on the source
keeping its copy; APFS private-size counters would. After the focused test, a median
{shared * 100:.0f}% of a seeded target's allocated bytes were still the files the seed
cloned.</p>
<h2>Largest process RSS, GiB</h2>{table("rss_bytes", 1024**3, 2)}
<h2>Peak process-tree RSS, GiB</h2>{table("tree_rss_bytes", 1024**3, 2)}
<h2>Seed sources</h2><p>The newest sibling target with no Cargo running, as the hook
picked it at each add. The probe removes its own worktrees after each sample, so none
seeds a later one.</p><ul>{source_rows}</ul>
<p>Compiler: {escape(config["rustc"].splitlines()[0])}.
Machine: {escape(config["machine"][4])}, {escape(config["machine"][0])}
{escape(config["machine"][2])}. Shared-cache counters include other agents' compiles and
are not recorded.</p>
<p><a href="loo-922.json">Receipts</a> · <a href="../README.md#seeded-worktrees">Method</a></p>
</html>
"""
    (output / "loo-922.html").write_text(document)
    print(output / "loo-922.html")


def _compiled(receipts: list[dict], label: str, case: str) -> str:
    """What the workspace compiled in this case, the same in every cohort or all variants."""
    variants = {
        " + ".join(
            c["crate"] + (" (tests)" if c["crate_type"] is None else "")
            for c in r["workspace_compilers"]
            if c["cache_misses"] or c["crate_type"] is None
        )
        for r in receipts
        if r["label"] == label and r["case"] == case
    }
    return html.escape(" | ".join(sorted(variants)))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    source_group = parser.add_mutually_exclusive_group(required=True)
    source_group.add_argument("--input", type=Path)
    source_group.add_argument("--test-loop", type=Path)
    source_group.add_argument("--seed-probe", type=Path)
    parser.add_argument("--name", help="File stem and page title for --input, e.g. loo-923")
    parser.add_argument(
        "--cache-validation", type=Path, help="root_probe.py output, when it was run"
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.test_loop:
        _test_loop(args.test_loop.resolve(), args.output.resolve())
        return
    if args.seed_probe:
        _seed_probe(args.seed_probe.resolve(), args.output.resolve())
        return
    if not args.name:
        parser.error("--name is required with --input")
    source = args.input.resolve()
    name = args.name
    config = json.loads((source / "configuration.json").read_text())
    cohorts = config["cohorts"]
    receipts = json.loads((source / "receipts.json").read_text())
    expected = {(label, 0, "cold_seed") for label in ("base", "candidate")} | {
        (label, cohort, case)
        for label in ("base", "candidate")
        for cohort in range(1, cohorts + 1)
        for case in CASES
    }
    actual = {(r["label"], r["cohort"], r["case"]) for r in receipts}
    if actual != expected or len(receipts) != len(expected):
        raise RuntimeError(f"{cohorts} complete paired cohorts and two seeds are required")
    for receipt in receipts:
        _validate_closure(receipt)
    summary = json.loads((source / "summary.json").read_text())
    if len(summary) != 2 * len(CASES) or any(row["n"] != cohorts for row in summary):
        raise RuntimeError(f"summary does not contain {cohorts} samples per case and revision")
    if not _catalog_complete(source):
        raise RuntimeError("catalogue checks are incomplete")
    validation = None
    if args.cache_validation:
        if not json.loads((args.cache_validation / "passed.json").read_text())["passed"]:
            raise RuntimeError("cache root validation did not pass")
        validation = json.loads((args.cache_validation / "receipts.json").read_text())
    identities = [json.loads(p.read_text()) for p in sorted(source.glob("*_identity.json"))]
    if len(identities) != 2 * (cohorts + 1):
        raise RuntimeError("each source copy needs a runtime identity receipt")
    for identity in identities:
        if identity["observed"] != f"{identity['root']}|{identity['revision']}-dirty":
            raise RuntimeError("runtime identity differs from its source copy")
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
        "cache_root_validation": validation,
    }
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(payload, indent=2) + "\n"
    (output / f"{name}-raw.json.gz").write_bytes(gzip.compress(raw.encode(), mtime=0))
    compact = {**payload, "receipts": []}
    compact["raw_receipts"] = f"{name}-raw.json.gz"
    compact["catalogue"] = [
        {
            "case": row["case"],
            "schema_sha256": row["schema_sha256"],
            "catalogue_sha256": hashlib.sha256(json.dumps(row["catalog"]).encode()).hexdigest(),
            "migration_count": len(row["catalog"]),
            "drafts": [slug for slug, merged in row["catalog"] if merged == "false"],
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
    (output / f"{name}.json").write_text(json.dumps(compact, indent=2) + "\n")
    escape = html.escape
    timed = [r for r in receipts if r["case"] != "cold_seed"]
    loads = [r["load_before"][0] for r in timed] + [r["load_after"][0] for r in timed]
    closure = "".join(
        f"<tr><th>{LABELS[case]}</th><td>{_compiled(receipts, 'base', case)}</td>"
        f"<td>{_compiled(receipts, 'candidate', case)}</td></tr>"
        for case in CASES
    )
    root_check = (
        "Root upkeep passed through an observed natural idle exit."
        if validation
        else "The root upkeep probe was not run for this comparison."
    )
    document = f"""<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>{escape(name.upper())} rebuild measurements</title>
{REPORT_STYLE}
<h1>{escape(name.upper())}: what each edit rebuilds, and what it costs</h1>
<p>{cohorts} alternating paired cohorts, one Cargo job. Values are
<strong>median [minimum–maximum]</strong>; lower is better. Every case but the
last builds <code>lf</code> and includes its link; the last builds the unit-test
binary of the crate that owns the decoder, without running it.</p>
<p>Baseline <code>{base}</code><br>Candidate <code>{candidate}</code></p>
<p>The one-minute load average while samples ran was {min(loads):.1f}–{max(loads):.1f}
on {config["cpus"]} logical CPUs. Elapsed time moves with that load; client CPU
seconds move less.</p>
<h2>What the workspace compiled</h2>
<table><thead><tr><th>Case</th><th>Baseline</th><th>Candidate</th></tr></thead>
<tbody>{closure}</tbody></table>
<p>Compiler misses and test binaries, from per-invocation receipts. A workspace
library not named in a row was reused: Cargo-fresh, or in the fresh-worktree case
a cache hit. The validator requires each edit to recompile exactly the libraries
downstream of it in that revision.</p>
<h2>Elapsed seconds</h2>{_table(summary, "wall_seconds")}
<h2>Client CPU seconds</h2>{_table(summary, "cpu_seconds")}
<h2>Largest process RSS, GiB</h2>{_table(summary, "rss_bytes", 1024**3)}
<h2>Method and limits</h2>
<p>Independent targets and cold-seeded caches. Cohort order alternates
candidate/base, base/candidate. Edits have unique cohort suffixes. Two cold seeds
are calibration and are excluded from the medians. Before the test-binary case,
the owner's test binary and its dev-dependencies are built once outside the
timing, then a second decoder message is edited. Profiles match. The registry
dependency delta is {len(base_packages - candidate_packages)} removed and
{len(candidate_packages - base_packages)} added version/checksum tuples.
Machine: {config["cpus"]} logical CPUs, {escape(config["machine"][4])},
{escape(config["machine"][0])} {escape(config["machine"][2])}.
Compiler: {escape(config["rustc"])}.</p>
<p>The sccache binary is pinned by SHA-256
<code>{escape(config["cache_sha256"])}</code>; it needs Rust basedir and verbatim
environment-dependency support, which 0.19.0 lacks. {root_check} No Machine cache
configuration or shared daemon was changed.</p>
<p>The wrapper adds two statistics RPCs per workspace compiler; that overhead is
included. CPU covers the timed client process tree, excluding the cache daemon.
RSS is its largest process, not concurrent Machine memory. These observations do
not establish total Machine CPU savings or a safe concurrency budget.</p>
<p>All {len(identities)} source copies passed runtime root/revision checks. Catalogue
checks retained the canonical schema through Git reference and source-package changes.
Retrieved rlibs can retain the seed checkout’s debug-source paths; use
<code>scripts/dev-lf --isolated-build</code> when those paths no longer resolve.</p>
<p><a href="{name}.json">Compiler, resource and correctness receipts</a> ·
<a href="{name}-raw.json.gz">Full cache/resource receipts (compressed JSON)</a> ·
<a href="../README.md">Reproduce the comparison</a></p></html>
"""
    (output / f"{name}.html").write_text(document)
    print(output / f"{name}.html")


if __name__ == "__main__":
    main()
