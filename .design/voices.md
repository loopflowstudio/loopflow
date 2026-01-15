# Voices

Adds reusable personas (system prompts) that get prepended to tasks.

## Review

**Verdict:** Ready to ship

Clean implementation. Tests pass. Config/frontmatter/CLI priority chain works correctly. Error messages are helpful (shows available voices or suggests creation path).

## Design notes

**Prompt assembly**: Voices appear between "The task." header and the task content. Single voice uses `<lf:voice:name>` directly; multiple voices wrap in `<lf:voices>`.

**Voice files are plain markdown** in `.lf/voices/`. No frontmatter or templates. Content is stripped of leading/trailing whitespace on load.

**Priority**: CLI `--voice` > frontmatter `voice:` > config `voice:` > none. Frontmatter and config support both string and list syntax.
