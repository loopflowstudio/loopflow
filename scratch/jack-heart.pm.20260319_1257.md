# PM round-trip: ingest-assign + push-diff

Two ops that close the PM loop. Ingest claims a task from PM. Push-diff sends back what the branch changed.

## Ingest-assign

`lf ops ingest` gains PM awareness when PM is configured for the wave:

1. Pull fresh items from PM provider
2. Filter to unassigned items (no assignee)
3. Pick highest priority
4. **Assign immediately in PM** — this is the lock. Two agents racing ingest see each other's assignments.
5. Write assignment to local frontmatter (`status: in_progress`)
6. Copy item to scratch/ (existing ingest behavior)

The PM write happens *before* the local write. If PM assignment fails (already claimed), try the next item. Optimistic locking — first writer wins.

### What "assign" means

- Set `assignee` to the API token owner (simplest — no config needed)
- Set status to in-progress equivalent (`In Progress` in Linear, move to section in Asana)

## Push-diff

`lf ops pm push-diff [WAVE]`:

1. Find baseline commit:
   - Search branch history for last commit whose message contains `pm pull`
   - If none found, use `merge-base HEAD main`
2. `git diff --name-only <baseline> HEAD -- wave/<wave>/`
3. For each changed `.md` file:
   - Parse before (from baseline via `git show`) and after (from disk)
   - Compare title, description, status fields
   - Push only the fields that actually changed to PM provider
4. For new files (no baseline version): create in PM, write ID back to frontmatter
5. For deleted files: no-op (additive-only, don't delete from PM)

### What gets pushed

- Title changes → `update_item` with new name
- Description changes → `update_item` with new description
- Status changes (frontmatter `status` field) → update PM status
- New items without provider ID → `create_item`

### What doesn't get pushed

- Rank/ordering changes (local concern)
- Items unchanged between baseline and HEAD
- Items imported during pull (they're in the baseline, so they diff out)

## Flow changes

```yaml
# deploy.yaml — all mutations land here
- gate
- op: land --create-pr
- op: pm push-diff

# sync.yaml — catching up
- rebase
- integrate-upstream
- op: pm pull

# build-or-silent.yaml — fresh state before picking work
- op: pm pull
- ingest
- xor: ...

# garden-or-silent.yaml — fresh state before scanning
- op: pm pull
- garden/scan
- garden/assess
- xor: ...

# Planning flows now end in deploy (push-diff included):
# wave-reduce: and(reduce×3) → update-wave → deploy
# wave-polish: and(polish×3) → update-wave → deploy
# wave-expand: and(expand×3) → update-wave → deploy
# garden: wave/mutate → wave/review → deploy
```

## Executor changes

Remove the auto `pm_sync` calls at run start and run end. The flows handle it now.

## CLI surface

```
lf ops pm push-diff [WAVE]     # push branch changes to PM
lf ops pm push-diff --all      # push all PM-enabled waves
```

## Done when

```bash
cargo test -p loopflow pm_push_diff
cargo test -p loopflow ingest
cargo clippy -- -D warnings
cargo test --all
```
