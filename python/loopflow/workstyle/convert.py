from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
import re
import shutil
import subprocess
from typing import Any, Callable

import yaml

_BROWSER_SKILLS = {
    "benchmark",
    "browse",
    "canary",
    "connect-chrome",
    "qa",
    "qa-only",
    "setup-browser-cookies",
}
_SECTION_BREAK = re.compile(r"^#{1,2}\s")
_TOP_LEVEL_HEADING = re.compile(r"^#\s")
_RENAMED_STEPS = {
    "design-review": "design-audit",
    "plan-ceo-review": "ceo-review",
    "plan-design-review": "design-review",
    "plan-eng-review": "eng-review",
}
_GSTACK_STEP_REFERENCES = (
    "plan-design-review",
    "plan-ceo-review",
    "plan-eng-review",
    "setup-browser-cookies",
    "design-consultation",
    "document-release",
    "land-and-deploy",
    "connect-chrome",
    "gstack-upgrade",
    "setup-deploy",
    "office-hours",
    "design-review",
    "benchmark",
    "investigate",
    "autoplan",
    "careful",
    "canary",
    "browse",
    "freeze",
    "unfreeze",
    "guard",
    "codex",
    "retro",
    "review",
    "ship",
    "cso",
    "qa-only",
    "qa",
    "gstack",
)
_REMOVED_SECTION_PREFIXES = (
    "## Voice",
    "## Voice & Tone",
    "## Contributor Mode",
    "## Completion Status Protocol",
    "## Completion: Write Review Logs",
    "## Telemetry",
    "## Plan Status Footer",
    "## GSTACK REVIEW REPORT",
    "## Repo Ownership",
    "## Review Log",
    "## Search Before Building",
    "## Step 8.75: Persist ship metrics",
    "## AskUserQuestion Format",
    "## Completeness Principle",
)
_CLEANUP_PATTERNS = (
    (
        re.compile(
            r"\n```bash\nmkdir -p ~/.gstack/analytics\n"
            r'echo \'{"skill":"[^"]+".*?skill-usage\.jsonl.*?\n```\n',
            re.DOTALL,
        ),
        "\n",
    ),
    (
        re.compile(
            r"\n3\. Append metrics:\n```bash\n"
            r"mkdir -p ~/.gstack/analytics\n"
            r'.*?spec-review\.jsonl.*?\n```\n'
            r"Replace ITERATIONS, FOUND, FIXED, REMAINING, SCORE with actual values from the review\.\n",
            re.DOTALL,
        ),
        "\n",
    ),
    (
        re.compile(
            r"\nLog (?:the result for the review dashboard|to the review dashboard):\n"
            r"(?:\n```bash\n.*?mkdir -p ~/.gstack/projects/\$SLUG.*?\n```\n)?"
            r"\nWrite a JSONL entry(?: with timing data)?:"
            r"(?: .*?\n|\n```json\n.*?\n```\n)",
            re.DOTALL,
        ),
        "\n",
    ),
    (
        re.compile(
            r"\n# \d+\. gstack skill usage telemetry \(if available\)\n"
            r"cat ~/.gstack/analytics/skill-usage\.jsonl 2>/dev/null \|\| true\n"
        ),
        "\n",
    ),
    (
        re.compile(
            r"\n\*\*Skill Usage \(if analytics exist\):\*\*.*?"
            r"If the JSONL file doesn't exist or has no entries in the window, "
            r"skip the Skill Usage row\.\n",
            re.DOTALL,
        ),
        "\n",
    ),
    (
        re.compile(
            r"\n\*\*Eureka Moments \(if logged\):\*\*.*?"
            r"If the JSONL file doesn't exist or has no entries in the window, "
            r"skip the Eureka Moments row\.\n",
            re.DOTALL,
        ),
        "\n",
    ),
)
_CLEANUP_REPLACEMENTS = (
    (
        "Read ETHOS.md for the full Search Before Building framework (three layers, eureka moments). "
        "The preamble's Search Before Building section has the ETHOS.md path.",
        "Read ETHOS.md for the full Search Before Building framework (three layers, eureka moments).",
    ),
    (
        "Read ETHOS.md for the Search Before Building framework "
        "(the preamble's Search Before Building section has the path).",
        "Read ETHOS.md for the Search Before Building framework.",
    ),
    (" Log the eureka moment (see preamble).", ""),
    (" Log it (see preamble).", ""),
    (" (see preamble's Search Before Building section)", ""),
)


@dataclass
class GstackSkill:
    name: str
    version: str | None
    description: str
    allowed_tools: list[str]
    benefits_from: list[str]
    preamble_tier: int | None
    instructions: str


@dataclass
class WorkstyleManifest:
    name: str
    source_repo: str
    source_ref: str
    last_sync: str
    last_commit: str
    step_prefix: str
    steps: list[str]


def convert_gstack_repo(
    source_repo: Path,
    output_dir: Path,
    *,
    source_repo_name: str = "garrytan/gstack",
    source_ref: str = "main",
    direction_output: Path | None = None,
) -> WorkstyleManifest:
    skill_paths = _find_skill_paths(source_repo)
    converted_steps: list[tuple[str, GstackSkill]] = []
    extracted_voice: str | None = None
    fallback_voice: str | None = None
    last_commit = _git_head(source_repo)
    last_sync = _utc_now()

    for skill_path in skill_paths:
        skill, voice = _convert_skill(skill_path)
        step_name = _step_name(skill.name)
        converted_steps.append((step_name, skill))
        if fallback_voice is None and voice:
            fallback_voice = voice
        if voice and "You are GStack" in voice:
            extracted_voice = voice

    extracted_voice = extracted_voice or fallback_voice
    if extracted_voice is None:
        raise ValueError("No gstack voice section found")

    manifest = WorkstyleManifest(
        name="gstack",
        source_repo=source_repo_name,
        source_ref=source_ref,
        last_sync=last_sync,
        last_commit=last_commit,
        step_prefix="gstack",
        steps=[name for name, _skill in converted_steps],
    )
    _write_workstyle(output_dir, converted_steps, manifest)

    if direction_output is not None:
        direction_output.parent.mkdir(parents=True, exist_ok=True)
        direction_output.write_text(extracted_voice.rstrip() + "\n", encoding="utf-8")

    return manifest


def extract_openclaw_direction(path: Path) -> str:
    content = path.read_text(encoding="utf-8")
    _frontmatter, body = _split_frontmatter(content)
    return body.strip() + "\n"


def _find_skill_paths(source_repo: Path) -> list[Path]:
    skill_paths = [source_repo.joinpath("SKILL.md")]
    skill_paths.extend(sorted(source_repo.glob("*/SKILL.md")))
    return [path for path in skill_paths if path.is_file()]


def _convert_skill(skill_path: Path) -> tuple[GstackSkill, str | None]:
    frontmatter, body = _split_frontmatter(skill_path.read_text(encoding="utf-8"))
    metadata = yaml.safe_load(frontmatter) if frontmatter else {}
    if not isinstance(metadata, dict):
        raise ValueError(f"Invalid frontmatter in {skill_path}")

    voice = _extract_voice(body)
    body_without_preamble = _strip_preamble_and_wrapper(body)
    instructions = _strip_named_sections(body_without_preamble, _REMOVED_SECTION_PREFIXES)
    instructions = _cleanup_imported_artifacts(instructions)
    instructions = _rewrite_imported_references(instructions)
    instructions = _cleanup_loopflow_integration_artifacts(instructions).strip()

    skill = GstackSkill(
        name=str(metadata["name"]),
        version=_maybe_string(metadata.get("version")),
        description=_rewrite_imported_references(_maybe_string(metadata.get("description")) or ""),
        allowed_tools=_string_list(metadata.get("allowed-tools")),
        benefits_from=[_step_name(name) for name in _string_list(metadata.get("benefits-from"))],
        preamble_tier=_maybe_int(metadata.get("preamble-tier")),
        instructions=instructions + "\n" if instructions else "",
    )
    return skill, voice


def _split_frontmatter(content: str) -> tuple[str | None, str]:
    if not content.startswith("---"):
        return None, content

    parts = content.split("---", 2)
    if len(parts) != 3:
        return None, content

    _before, frontmatter, body = parts
    return frontmatter.strip(), body.lstrip("\n")


def _extract_voice(body: str) -> str | None:
    return _extract_section_body(body, "## Voice")


def _strip_preamble_and_wrapper(body: str) -> str:
    stripped = _strip_named_sections(body, ("## Preamble",))
    stripped = _remove_auto_generated_comments(stripped)
    lines = stripped.splitlines(keepends=True)
    first_heading = _find_first_top_level_heading(lines)
    if first_heading is None:
        return stripped
    return "".join(lines[first_heading:])


def _remove_auto_generated_comments(body: str) -> str:
    kept_lines = [
        line for line in body.splitlines(keepends=True) if not line.lstrip().startswith("<!--")
    ]
    return "".join(kept_lines).lstrip()


def _extract_section_body(text: str, heading_prefix: str) -> str | None:
    lines = text.splitlines(keepends=True)
    start = _find_markdown_line(lines, lambda line: line.startswith(heading_prefix))
    if start is None:
        return None

    end = _find_section_end(lines, start)
    return "".join(lines[start + 1 : end]).strip() or None


def _strip_named_sections(text: str, heading_prefixes: tuple[str, ...]) -> str:
    lines = text.splitlines(keepends=True)
    kept: list[str] = []
    index = 0

    while index < len(lines):
        line = lines[index]
        if any(line.startswith(prefix) for prefix in heading_prefixes):
            index = _find_section_end(lines, index)
            continue
        kept.append(line)
        index += 1

    return _remove_auto_generated_comments("".join(kept))


def _write_workstyle(
    output_dir: Path,
    converted_steps: list[tuple[str, GstackSkill]],
    manifest: WorkstyleManifest,
) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    stale_steps_dir = output_dir.joinpath("steps")
    if stale_steps_dir.exists():
        shutil.rmtree(stale_steps_dir)
    for path in output_dir.glob("*.md"):
        path.unlink()

    for step_name, skill in converted_steps:
        _write_step(output_dir.joinpath(f"{step_name}.md"), step_name, skill)

    manifest_data = {
        "name": manifest.name,
        "description": "Garry Tan's sprint factory",
        "source": {
            "repo": manifest.source_repo,
            "ref": manifest.source_ref,
            "last_commit": manifest.last_commit,
            "last_sync": manifest.last_sync,
        },
        "prefix": manifest.step_prefix,
    }
    output_dir.joinpath("workstyle.yaml").write_text(
        yaml.safe_dump(manifest_data, sort_keys=False, allow_unicode=True),
        encoding="utf-8",
    )


def _write_step(path: Path, step_name: str, skill: GstackSkill) -> None:
    frontmatter: dict[str, Any] = {}
    if skill.description:
        frontmatter["description"] = skill.description
    if skill.allowed_tools:
        frontmatter["tools"] = skill.allowed_tools
    if skill.version is not None:
        frontmatter["version"] = skill.version
    if skill.preamble_tier is not None:
        frontmatter["preamble_tier"] = skill.preamble_tier
    if skill.benefits_from:
        frontmatter["after"] = skill.benefits_from
    if step_name in _BROWSER_SKILLS:
        frontmatter["requires"] = ["browser"]

    yaml_frontmatter = yaml.safe_dump(frontmatter, sort_keys=False, allow_unicode=True).strip()
    path.write_text(f"---\n{yaml_frontmatter}\n---\n{skill.instructions}", encoding="utf-8")


def _git_head(repo: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        capture_output=True,
        check=True,
        text=True,
    )
    return result.stdout.strip()


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _step_name(name: str) -> str:
    return _RENAMED_STEPS.get(name, name)


def _string_list(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, list):
        return [str(item) for item in value]
    return [str(value)]


def _maybe_string(value: Any) -> str | None:
    if value is None:
        return None
    return str(value)


def _maybe_int(value: Any) -> int | None:
    if value is None:
        return None
    return int(value)


def _cleanup_imported_artifacts(text: str) -> str:
    cleaned = text
    for pattern, replacement in _CLEANUP_PATTERNS:
        cleaned = pattern.sub(replacement, cleaned)
    for old, new in _CLEANUP_REPLACEMENTS:
        cleaned = cleaned.replace(old, new)
    return cleaned


def _rewrite_imported_references(text: str) -> str:
    rewritten = re.sub(
        r"~/.claude/skills/gstack/([a-z0-9-]+)/SKILL\.md",
        lambda match: f".lf/steps/gstack/{_step_name(match.group(1))}.md",
        text,
    )

    for skill_name in _GSTACK_STEP_REFERENCES:
        prefixed_name = f"gstack:{_step_name(skill_name)}"
        rewritten = re.sub(
            rf"(?<![\w.-])/{re.escape(skill_name)}(?![\w-])",
            prefixed_name,
            rewritten,
        )

    return rewritten


def _cleanup_loopflow_integration_artifacts(text: str) -> str:
    cleaned = re.sub(
        r"\nFollow it inline, \*\*skipping these sections\*\* "
        r"\(already handled by (?:the )?parent skill\):\n(?:- .+\n)+",
        "\nFollow the imported loopflow step content directly.\n",
        text,
    )
    cleaned = re.sub(
        r"\nFollow it inline, skipping these sections "
        r"\(already handled by (?:the )?parent skill\):\n(?:[^\n]*\n)+?(?=\n(?:Note current Step|After completion))",
        "\nFollow the imported loopflow step content directly.\n\n",
        cleaned,
    )
    cleaned = re.sub(
        r"\n\*\*Section skip list — when following a loaded skill file, SKIP these sections\s*"
        r"\(they are already handled by gstack:autoplan\):\*\*\n(?:- .+\n)+\n",
        "\n",
        cleaned,
    )
    return cleaned


def _find_first_top_level_heading(lines: list[str]) -> int | None:
    return _find_markdown_line(lines, lambda line: _TOP_LEVEL_HEADING.match(line) is not None)


def _find_markdown_line(lines: list[str], predicate: Callable[[str], bool]) -> int | None:
    in_fence = False
    for index, line in enumerate(lines):
        if _is_fence_boundary(line):
            in_fence = not in_fence
            continue
        if not in_fence and predicate(line):
            return index
    return None


def _find_section_end(lines: list[str], start: int) -> int:
    in_fence = False
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if _is_fence_boundary(line):
            in_fence = not in_fence
            continue
        if not in_fence and _SECTION_BREAK.match(line):
            return index
    return len(lines)


def _is_fence_boundary(line: str) -> bool:
    return line.lstrip().startswith("```")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source_repo", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--source-repo-name", default="garrytan/gstack")
    parser.add_argument("--source-ref", default="main")
    parser.add_argument("--direction-output", type=Path)
    args = parser.parse_args()

    manifest = convert_gstack_repo(
        args.source_repo,
        args.output_dir,
        source_repo_name=args.source_repo_name,
        source_ref=args.source_ref,
        direction_output=args.direction_output,
    )
    print(
        f"Converted {len(manifest.steps)} skills from {manifest.source_repo}@{manifest.source_ref} "
        f"to {args.output_dir}"
    )


if __name__ == "__main__":
    main()
