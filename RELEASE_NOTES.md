# v0.14.7

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.7 makes it easier to return to ongoing work while expanding the choice of coding harness. Desktop keeps working Tasks in a stable sidebar and retains their Shell and Files tabs as you switch Tasks or hide tools. Pi joins the supported harnesses for native Sessions and headless skills and Flows, with saved conversation history and portable generated skill names.

## Switch Tasks without losing your place

Desktop now keeps explicitly opened Tasks in a working list above Waves, replacing activity-driven regrouping with predictable return paths. Shell and Files collect each Task's tools into retained tabs so switching work preserves the workspace during the app session. (#1594)

- Working Tasks keep their order through activity changes, filtering and completion. Pausing hides a Task while retaining its position and workspace; saved working order survives relaunch.
- Use ⌘1–9 and Control-Tab to navigate working slots. Breadcrumbs, tools and actions share the native titlebar.
- Keep multiple terminals in one Shell tool. Files retains document, Brief, Changes and PR tabs; hiding either tool preserves clients and drafts.
- Browse repositories and Waves without starting work. Waves show their full current Task plan, and unstarted Tasks open their brief.
- Codex excerpts follow the current Session and native turn. New turns clear old text, working excerpts expire after 30 seconds, and Available results retain their source age. Other providers show labeled summaries.
- Provider switching reads saved history before releasing the current terminal. If the transcript is missing, the conversation stays intact for retry.

## Use Pi for conversations and automation

Pi users can run native Sessions and headless skills and Flows through Loopflow, then resume the same saved conversation. Pi owns model selection and credentials; Loopflow retains text, tools and reported usage in its existing history. (#1591)

- Choose `pi` or `pi:provider/model` through the normal agent selector.
- Context refreshes after compaction, live steering receives independent receipts, and native usage checkpoints survive crashes.
- Headless execution waits through Pi's internal retries and rejects blank final answers.
- Built-in bash commands use Pi's executor with Loopflow's watchdog established before execution, allowing cleanup of detached tool groups if the provider dies. Custom tools retain their own lifecycle ownership.

## Operational notes

- **Desktop command change:** `lf desktop shell` adds a Shell tab. `lf desktop split` is removed. (#1594)
- **Desktop upgrade and persistence:** use the matching published CLI to apply the Session-preview migration. Working order persists across launches; pane layouts and embedded clients do not. (#1594)
- **Pi baseline and limits:** Pi 1.3.0 is the tested baseline. Pi supplies no OS sandbox; callers own confinement. Managed account switching, subscription estimates, response schemas, turn limits and cross-harness history import are unsupported. Native session switching remains outside Loopflow's recorded conversation identity. (#1591)
- **Acceptance status:** real-Pi probes covered terminal and protocol behavior, steering, compaction, errors, crash recovery and viewer handoff. The supplied record does not establish a clean final combined-tree matrix: the broad local Rust run stopped with failures and unrun tests, leaving resolution to candidate and merge-queue CI. Desktop's mounted input continuity and final visual acceptance remain unverified; verify that Task switching and hiding/restoring tools preserve live input, drafts, selection and focus on a configured Mac. (#1591, #1594)

## Small changes

- Generated skill names use a shared lowercase-letter, digit and hyphen format capped at 64 characters. Long names shorten with stable suffixes, and collisions retain both sources while preserving canonical Loopflow identities, resource paths and third-party bundles. The tested shared Pi catalog loaded 101 skills with zero diagnostics; all 27 third-party skill files remained byte-identical. (#1591)
- Failed Desktop status observers restart on a later event. (#1594)