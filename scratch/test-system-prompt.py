#!/usr/bin/env python3
"""
Ablation test: what triggers the "Third-party apps" restriction?

Binary-searches on the real context file content to find the trigger.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BUILTINS = REPO / "rust/loopflow/src/engine/builtins"
SURFACES = BUILTINS / "surfaces"
TASK = "Say 'hello' and nothing else."

LOOPFLOW_DOC = (BUILTINS / "LOOPFLOW.md").read_text()
RLM_DOC = (BUILTINS / "RLM.md").read_text()
VOICE_DOC = (BUILTINS / "VOICE.md").read_text()
SURFACE_HEADLESS = (SURFACES / "headless.md").read_text()
SURFACE_CLI = (SURFACES / "cli.md").read_text()
SURFACE_CONCERTO_MAC = (SURFACES / "concerto_mac.md").read_text()
SURFACE_CONCERTO_IPHONE = (SURFACES / "concerto_iphone.md").read_text()

# The real context file that triggers the error
context_files = sorted(REPO.glob(".lf/prompts/*.context.md"))
FULL_CONTEXT_PATH = context_files[-1] if context_files else None
FULL_CONTEXT = FULL_CONTEXT_PATH.read_text() if FULL_CONTEXT_PATH else None


def write_tmp(content: str) -> Path:
    f = tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False)
    f.write(content)
    f.close()
    return Path(f.name)


def is_blocked(system_prompt: str | None = None, system_prompt_file: Path | None = None) -> bool:
    """Returns True if the request gets the third-party app error."""
    cmd = ["claude", "-p", TASK, "--max-turns", "1", "--output-format", "json"]
    if system_prompt_file:
        cmd += ["--append-system-prompt-file", str(system_prompt_file)]
    elif system_prompt is not None:
        cmd += ["--append-system-prompt", system_prompt]

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        out = (proc.stdout + proc.stderr).lower()
        return "third-party" in out or "not your plan" in out
    except subprocess.TimeoutExpired:
        return False


def test(name: str, **kwargs) -> bool:
    sys.stdout.write(f"  {name[:70]:<70} ")
    sys.stdout.flush()
    blocked = is_blocked(**kwargs)
    print("BLOCKED" if blocked else "ok")
    return blocked


def main():
    print("=" * 78)
    print("Third-party app restriction: ablation tests")
    print("=" * 78)
    print()

    # ── Baselines ─────────────────────────────────────────────────────
    print("── Baselines ──")
    test("No system prompt")
    test("Inline: short text", system_prompt="You are helpful.")
    test("Inline: 1KB text", system_prompt="Follow conventions. " * 50)
    print()

    # ── Reproduce with real context file ──────────────────────────────
    print("── Reproduce with real context file ──")
    if FULL_CONTEXT_PATH:
        real_blocked = test(f"File: {FULL_CONTEXT_PATH.name}", system_prompt_file=FULL_CONTEXT_PATH)
        # Also try the same content inline vs file
        test("Inline: same content as real context file", system_prompt=FULL_CONTEXT)
        # Same content, fresh temp file
        test("File: same content, fresh temp file", system_prompt_file=write_tmp(FULL_CONTEXT))
    else:
        print("  (no context file found)")
        real_blocked = False
    print()

    if not real_blocked:
        print("Real context file did NOT trigger the block. Can't binary search.")
        print("Try running this closer to when the error occurs.")
        # Still run the other tests for data
        print()

    # ── File vs inline ────────────────────────────────────────────────
    print("── Mechanism: file vs inline ──")
    test("File: LOOPFLOW.md", system_prompt_file=write_tmp(f"<lf:loopflow>\n{LOOPFLOW_DOC}\n</lf:loopflow>"))
    test("Inline: LOOPFLOW.md", system_prompt=f"<lf:loopflow>\n{LOOPFLOW_DOC}\n</lf:loopflow>")
    test("File: RLM.md", system_prompt_file=write_tmp(f"<lf:rlm>\n{RLM_DOC}\n</lf:rlm>"))
    test("Inline: RLM.md", system_prompt=f"<lf:rlm>\n{RLM_DOC}\n</lf:rlm>")
    print()

    # ── Surface instructions ──────────────────────────────────────────
    print("── Surface instructions ──")
    test("Inline: headless", system_prompt=SURFACE_HEADLESS)
    test("Inline: CLI", system_prompt=SURFACE_CLI)
    test("Inline: Concerto Mac", system_prompt=SURFACE_CONCERTO_MAC)
    test("Inline: Concerto iPhone", system_prompt=SURFACE_CONCERTO_IPHONE)
    print()

    # ── Structured replies ────────────────────────────────────────────
    print("── Structured reply XML ──")
    test("Inline: structured_replies block", system_prompt="""<lf:structured_replies>
<lf:suggest_actions>
After each response, suggest 2-4 concrete next actions.
Format: JSON array [{"label": "...", "message": "..."}]
</lf:suggest_actions>
</lf:structured_replies>""")
    print()

    # ── Progressive buildup ───────────────────────────────────────────
    print("── Progressive buildup (add one section at a time) ──")
    sections = []

    sections.append(("+ loopflow", f"<lf:loopflow>\n{LOOPFLOW_DOC}\n</lf:loopflow>"))
    test("loopflow only", system_prompt_file=write_tmp(sections[-1][1]))

    sections.append(("+ rlm", f"<lf:rlm>\n{RLM_DOC}\n</lf:rlm>"))
    combined = "\n\n".join(s[1] for s in sections)
    test("loopflow + rlm", system_prompt_file=write_tmp(combined))

    sections.append(("+ voice", f"<lf:voice>\n{VOICE_DOC}\n</lf:voice>"))
    combined = "\n\n".join(s[1] for s in sections)
    test("loopflow + rlm + voice", system_prompt_file=write_tmp(combined))

    sections.append(("+ headless surface", SURFACE_HEADLESS))
    combined = "\n\n".join(s[1] for s in sections)
    test("loopflow + rlm + voice + headless", system_prompt_file=write_tmp(combined))

    sections.append(("+ structured replies", """<lf:structured_replies>
<lf:suggest_actions>
Suggest 2-4 actions. Format: [{"label": "...", "message": "..."}]
</lf:suggest_actions>
</lf:structured_replies>"""))
    combined = "\n\n".join(s[1] for s in sections)
    test("loopflow + rlm + voice + headless + structured", system_prompt_file=write_tmp(combined))
    print()

    # ── Binary search on the real context file ────────────────────────
    if FULL_CONTEXT and real_blocked:
        print("── Binary search on real context file ──")
        # Split into sections by double newline
        raw_sections = FULL_CONTEXT.split("\n\n")
        print(f"  Context has {len(raw_sections)} sections ({len(FULL_CONTEXT)} bytes)")
        print()

        # Test first half vs second half
        mid = len(raw_sections) // 2
        first_half = "\n\n".join(raw_sections[:mid])
        second_half = "\n\n".join(raw_sections[mid:])

        first_blocked = test(f"First half ({len(first_half)} bytes)", system_prompt_file=write_tmp(first_half))
        second_blocked = test(f"Second half ({len(second_half)} bytes)", system_prompt_file=write_tmp(second_half))

        # Drill into whichever half is blocked
        target = raw_sections[:mid] if first_blocked else raw_sections[mid:] if second_blocked else None
        if target:
            print()
            print("  Drilling into blocked half...")
            qmid = len(target) // 2
            q1 = "\n\n".join(target[:qmid])
            q2 = "\n\n".join(target[qmid:])
            q1_blocked = test(f"  Quarter A ({len(q1)} bytes)", system_prompt_file=write_tmp(q1))
            q2_blocked = test(f"  Quarter B ({len(q2)} bytes)", system_prompt_file=write_tmp(q2))

            # One more level
            target2 = target[:qmid] if q1_blocked else target[qmid:] if q2_blocked else None
            if target2:
                print()
                print("  Drilling deeper...")
                emid = len(target2) // 2
                e1 = "\n\n".join(target2[:emid])
                e2 = "\n\n".join(target2[emid:])
                e1_blocked = test(f"    Eighth A ({len(e1)} bytes)", system_prompt_file=write_tmp(e1))
                e2_blocked = test(f"    Eighth B ({len(e2)} bytes)", system_prompt_file=write_tmp(e2))

                # Print the triggering section content for inspection
                trigger = target2[:emid] if e1_blocked else target2[emid:] if e2_blocked else None
                if trigger:
                    print()
                    print("  Likely trigger content (first 500 chars):")
                    print("  " + "-" * 60)
                    content = "\n\n".join(trigger)
                    for line in content[:500].splitlines():
                        print(f"  | {line}")
                    print("  " + "-" * 60)
        print()

    # ── Per-section isolation (test each section of context file alone) ──
    if FULL_CONTEXT:
        print("── Per-section isolation (each top-level section alone) ──")
        # Split by XML-like tags
        import re
        tag_sections = re.split(r'(?=<lf:)', FULL_CONTEXT)
        tag_sections = [s.strip() for s in tag_sections if s.strip()]
        for i, section in enumerate(tag_sections[:15]):  # cap at 15
            # Extract tag name for label
            tag_match = re.match(r'<lf:(\w+)', section)
            tag_name = tag_match.group(1) if tag_match else f"section-{i}"
            size = len(section)
            test(f"Section alone: <lf:{tag_name}> ({size} bytes)", system_prompt_file=write_tmp(section))
        print()

    print("Done.")


if __name__ == "__main__":
    main()
