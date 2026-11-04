# Forward preferences before building settings sync

LOO-714 · Research for Jack Heart · November 3, 2026

Recommendation for review, not accepted implementation direction. Built from
settings-catalog.md, machine-config-prior-art.md and primary sources below.
No live config, machine, account or execution state was changed.

Subsequent decision, November 3: Jack Heart accepted origin personal preferences
for forwarded launches and saved remote preferences for work initiated directly
there. The executor supplies its environment. Different entry points may differ;
retention and independent remote edits remain separate from launch authority.
This supersedes the open choice of per-launch authority below, without accepting
every proposed persistence mechanism. settings-catalog.md owns the current design.

## Decision

Start with forwarding portable preferences and retaining them remotely. Refresh
the retained copy automatically while it remains unchanged; preserve independently
edited remote config and report that persistence needs reconciliation. Defer
automatic merging of independent edits across machines.

The earlier merge recommendation assumed a requirement Jack has not stated: all
machines are equal writers whose edits should eventually reach every other machine.
The stated requirements are laptop preferences governing remote work and remote
retention for later work, including mobile use while the laptop is unavailable.

## What the references establish

| Reference | Observed behavior | Lesson for LF |
| --- | --- | --- |
| [VS Code Remote SSH](https://code.visualstudio.com/docs/remote/ssh#_ssh-hostspecific-settings) | Reuses local user settings for remote work, with explicit remote overrides. Workspace settings override both. | Forwarding is useful independently of synchronization. LF retains its own precedence. |
| [VS Code Settings Sync](https://code.visualstudio.com/docs/configure/settings-sync) | Separately merges preferences across devices, pauses on conflicts, and provides resolution choices. Machine-scoped settings are excluded by default. | Multi-device editing entails conflict UI and portability rules. |
| [Mutagen](https://mutagen.io/documentation/synchronization) | Distinguishes one-way-safe from one-way-replica and two-way modes. Safe forwarding preserves conflicting destination modifications; replica mode overwrites them. | Direction and overwrite policy are separate decisions. |
| [Syncthing](https://docs.syncthing.net/users/foldertypes.html) | Receive-only suppresses outgoing local edits but permits divergence. Later incoming conflicts retain a conflict copy and favor the cluster version. | Calling something a replica does not eliminate hand-edit behavior. |
| [chezmoi](https://www.chezmoi.io/user-guide/daily-operations/) | Uses Git-backed source files with explicit update/apply. A [merge command](https://www.chezmoi.io/reference/commands/merge/) reconciles source, target and destination with a merge tool. | File portability can use an existing dotfile workflow. |
| [Rex](https://www.superlogical.com/rex/docs/reference/cli) | Hosts follow one selected home; writes target their owning server, with offline changes held for later delivery. | Home establishes a write destination but introduces queuing. Arbitrary config conflict behavior is not established. |
| [Git push](https://git-scm.com/docs/git-push#_note_about_fast_forwards) | Normal branch pushes reject divergent history until changes are integrated or explicitly replaced. | Detecting divergence without automatically resolving it is useful. A content comparison is not Git ancestry. |

Prior Paseo/Herdr research establishes daemon-owned directories and file writers,
not automatic convergence of independently edited profiles. No reviewed reference
implements LF's exact choice/default, scope, occurrence and origin semantics.

## Alternatives

| Approach | Benefit | Cost or missing behavior |
| --- | --- | --- |
| Forward per request only | Origin preferences govern the request. | Fails retained remote/mobile preferences. |
| Always replace remote config | Simple propagation and retention. | Erases remote edits; an old client can regress the remote. |
| Forward and refresh an unchanged retained copy | Remote works offline; ordinary laptop updates propagate; remote edits survive. | Divergence requires a choice; no automatic reverse propagation. Recommended initially. |
| One writable source, other devices send edits to it | One authority for changes. | Offline writes require queuing or delayed saves; ownership becomes a product concept. |
| Every machine exchanges and merges edits | Independent offline editing can converge. | Ancestry, deletion semantics, compound-value merging and conflict recovery become LF features. |
| Delegate distribution to Git/chezmoi/Syncthing | Existing tools own synchronization. | Additional setup; does not itself supply LF's automatic forwarding and retention. |

## Proposed behavior

A laptop request carries authored portable declarations, preserving source, scope,
targets and agent/default_agent strength. Invocation choices remain separate. The
destination can retain the declarations in ordinary personal config files:
~/.lf/config.yaml and the accepted ./lf.config.local.yaml. Preserve machine-local
bindings; credentials and local machine identity never travel as preferences.

Existing differing remote preferences require an explicit adoption choice rather
than replacement merely because a client connected. After adoption, refresh only
when retained portable content still matches what was last applied, or already
matches the incoming content. Remote deletions count as edits too. Compare each
coherent portable configuration document/projection initially, without merging leaf
values. This can flag non-overlapping edits but keeps the policy understandable.

An independent remote edit through a text editor or lf set takes effect there.
On a conflicting delivery, preserve that file and expose the incoming version for
comparison. Keep the remote version, explicitly replace it, or bring changes back
to the source. Do not silently introduce two personal profiles as precedence layers.

Forwarding and saving have separate outcomes: the request still uses its carried
personal declarations, even when saving them would overwrite remote edits.
Inspection reports “used laptop preferences for this launch; remote defaults
retained.” Subsequent direct remote/mobile commands use the actual saved settings.
Do not block work or silently use a different profile because retention conflicted.

| Event | Result |
| --- | --- |
| Laptop supplies implement: Codex | Remote uses it and retains it after adoption. |
| Laptop sleeps; mobile connects directly to remote | New work uses retained Codex preferences. |
| Laptop changes to Claude; remote has no independent changes | Next delivery refreshes the remote to Claude. |
| Remote independently edits its preferences | Direct remote work uses that edit. |
| Laptop later sends differing preferences | Forwarded work uses laptop settings; saved remote settings survive, with an inspectable mismatch. |

Refresh at remote request boundaries. Offline use cannot promise changes that have
not arrived. Running commands are not reconfigured by delivery.

This still needs ordered delivery and atomic compare-and-write: older revisions
cannot replace newer ones from the same origin, and another client cannot silently
become the new source. Last-applied content/origin/version is operational metadata,
appropriate for the DB. It is not another editable preference layer. Any evidence
or backup copy is not read as a second authority.

## Why merging is a larger commitment

These are deductions from LF's model, not claims about the cited products:

- Account lists are ordered. Combining two edits can invent an unintended order.
- Deleting an override reveals a lower layer; deletion is different from empty.
- Changing agent to default_agent changes strength. A merge of different keys can
  accidentally preserve an explicit choice that defeats the new fallback.
- Planning-provider records and machine aliases have related fields/references.
- Different host settings can be intentional rather than changes to synchronize.

Catalogue portability rules help distribution, but do not define merge semantics.
The recommendation adds no internal Git repository, CRDT or background sync daemon.
It borrows a one-way safety boundary, not Mutagen's exact behavior: in particular,
LF should preserve remote deletions rather than resurrect them automatically.

## Decision for review

This proposal is insufficient if Jack routinely edits permanent preferences on a
phone connected directly to a remote and expects those edits to update the laptop
automatically. Two laptops frequently editing the same profile offline also justify
genuine synchronization. Using preferences on mobile does not itself require this.

1. Which workflow must the first release support?

   1a. Primarily edit on the laptop; remote/mobile use retained preferences, and
   occasional remote edits survive until explicit reconciliation. Recommended.

   1b. Edit permanent preferences anywhere and have changes flow back automatically.
   Choose a write authority or design peer synchronization before implementation.

Validation: primary documentation reviewed; no external software executed and no
sync implementation or live migration performed. Mutagen operates on files, not
YAML-setting semantics. VS Code's remote docs do not establish LF-style retention
for standalone commands. Rex's host directory is narrower than general config.
