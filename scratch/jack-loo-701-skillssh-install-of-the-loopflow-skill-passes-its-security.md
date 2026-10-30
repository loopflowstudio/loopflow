# LOO-701: skills.sh install of the loopflow skill

Status: implemented 2026-10-30; proof waits on the merged skill's re-audit.

## Evidence

- skills.sh audits the published `skills/loopflow/SKILL.md` only. The CLI
  (skills 1.7.4) reads ratings from `https://www.skills.sh/tele/audit?source=loopflowstudio/loopflow&skills=loopflow`.
- At 22:35 UTC on 2026-10-30, current main's skill was already rated Gen `safe`,
  Socket `0 alerts`, Snyk `low`, all analyzed 21:43 UTC. The High/Critical
  ratings Jack saw were an earlier audit. Pages:
  https://skills.sh/loopflowstudio/loopflow/loopflow/security/agent-trust-hub ·
  …/security/socket · …/security/snyk
- Gen still names two behaviors at Safe level. This change scopes both:
  - INDIRECT_PROMPT_INJECTION: the person's text went into a double-quoted shell
    argument (`--title "<…>"`, `lf comment <ID> "<…>"`). It now goes in on stdin
    through a quoted heredoc (`lf task create` takes its title from the first line
    of the piped report), or as one single-quoted argument.
    The skill README's examples show the same single-quoted form.
  - DYNAMIC_EXECUTION: running repository Skills/Graphs. The skill now runs one
    only when the person asks, and reads it and says what it does first.
- PromptScript: installer limitation. With `-y`, the CLI adds every
  `.agents/skills` agent, including project-only PromptScript, to a global
  install, so any `-g -y` install reports that failure. The documented command
  drops `-y`, so the installer shows its assessments and asks to confirm. The
  skill README says to install per project inside a PromptScript project.
  An agent running the installer still forces `-y` (the CLI does that itself).

## Remaining

- After merge, run a fresh `npx skills add loopflowstudio/loopflow --skill loopflow -g`
  and link the three audit pages in the PR. If Gen or Snyk rates the new text
  High/Critical, read the finding and repair the skill.

Check: `uv run pytest python/tests/test_loopflow_skill_alignment.py` passed (4).
