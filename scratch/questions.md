- Host pane PATH: the caller's PATH plus the Desktop tool directories (`server::supervised_path`). The other
  shape is lf adding those directories itself whenever it starts a provider, wherever it runs.
- `rex capture` reads a failed pane's text; `lf open` could put that text in its own failure message instead of
  pointing at the window.
- `lf open` writes `--agent <recorded harness:model>` into the pane command, so a pane opened before a move drags
  the conversation back to the old harness when rerun. Intended?
- `lf session new` on a closed conversation with no transcript now starts over from its first prompt. That also
  covers a real conversation whose transcript the provider deleted (retention): it restarts unseeded instead of
  refusing. `lf resume` on it still fails without replacing it. Right split?
- `resuming_an_ended_conversation_launches_its_provider_as_the_session` fails under `cargo test -p loopflow-execution
  --lib human_session` (shared process) and passes alone, before this change too. Order dependence; not fixed here.
- The new-thread message still says "seeded with what the claude thread knew" when there was nothing to seed.
