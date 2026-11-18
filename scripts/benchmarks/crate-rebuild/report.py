"""Keep completed rebuild receipts and render their median/range comparison."""

import argparse
import gzip
import hashlib
import html
import json
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
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--name", required=True, help="File stem and page title, e.g. loo-923")
    parser.add_argument(
        "--cache-validation", type=Path, help="root_probe.py output, when it was run"
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
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
<style>
body {{font:16px/1.5 system-ui,sans-serif;max-width:1050px;margin:48px auto;
padding:0 24px;color:#20252b}}
h1 {{font-size:32px;letter-spacing:-.03em}} h2 {{margin-top:40px;font-size:22px}}
code,td {{font-variant-numeric:tabular-nums}} code {{font-size:.9em}}
table {{width:100%;border-collapse:collapse;margin:16px 0 28px}}
th,td {{padding:10px;border-bottom:1px solid #ddd;text-align:left}}
thead {{background:#f4f5f6}} small {{color:#626970;white-space:nowrap}} a {{color:#145b90}}
</style>
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
