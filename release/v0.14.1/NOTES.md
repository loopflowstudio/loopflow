# v0.14.1

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.1 makes Desktop easier to make your own and easier to return to when a conversation is running elsewhere. Whole-app themes bring terminals and navigation into one palette, while an explicit **Move here** action replaces a dead-end Retry. Retired-work cleanup also does less historical scanning without treating uncertain agent state as permission to delete work.

## Make the workspace yours

Choose separate Light and Dark themes from 24 built-ins or your own Ghostty theme files. The choice applies across terminals, navigation, selection, focus, and status colors, with retained panes and file drafts preserved during the switch.

- Open **Appearance → Choose Theme…** to preview the workspace. **Apply** saves the choice; Cancel, Escape, or closing the chooser leaves it unchanged.
- System appearance offers both Light and Dark pickers. **Loopflow Light** retains the original cream and burgundy palette; **Loopflow Dark** uses slate and bronze.
- Import or customize themes in `~/.lf/themes/`. Loopflow also reads `~/.config/ghostty/themes/` without writing there, and derives app colors when a theme supplies only terminal colors.
- Store the selection in `~/.lf/config.yaml`, for example `theme: "light:Linen,dark:Hearth"`. Manual edits follow across windows; an unknown name retains the last theme and reports the error. Repository config does not set appearance.
- Personal Ghostty settings unrelated to color still apply. Loopflow controls terminal colors without editing Ghostty's config, and blocks Apply when app, selection, or cursor text is unreadable.

## Bring an active conversation into Desktop

Opening a Task whose conversation is active in another terminal now shows **Move here** in its conversation pane. The other client stays running until that action is clicked, including when entering through a Task link.

- Conversations active elsewhere remain usable destinations instead of appearing as failed opens.
- The transfer warning explains that moving loses unsent text in the other client. Retry remains available for actual opening failures.

## Clean up retired work without losing uncertain history

Retired checkout deletion now looks up matching current and historical branch owners and the target Task's unfinished execution, avoiding repeated loads of unrelated Task and PR history.

- `lf monitor prune --dry-run --json` reports exact AgentProcess dispositions and reasons for retaining them.
- A reservation without a PID can be retired only when launch was never requested and its exact owning Loopflow process is independently proven dead, with that evidence rechecked before retirement.
- Unknown providers and providers whose launch was requested remain protected. Age and Task completion alone do not establish that a provider exited; recorded outcomes and Task/PR history remain intact.

## Operational notes

- **About Loopflow** now opens an offline, searchable and selectable third-party license viewer. Packaging includes notices, pinned source archives, application source and build inputs, libintl libraries for both architectures, and a tested rebuild/relink recipe.
- App assembly and future GhosttyKit archive creation reject missing or mismatched materials before signing or publication. Source materials add approximately 217 MiB before compression; existing remote GhosttyKit archives remain unchanged.
- Live-state cleanup remains a post-install operation; shipping the cleanup changes does not establish that existing state has been cleaned.
- Headless tests cover theme colors and transfer preparation. Native visual coherence, live terminal input and focus during theme changes, and end-to-end client handoff remain unverified in the supplied release evidence.

## Small changes

- Change an existing Task's stack parent with `lf checkout CHILD --stack-on PARENT`, preserving its PR identity, base commit, and history. Run `lf sync` afterwards to integrate the new parent; cycles remain rejected.