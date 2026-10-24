# LOO-568: Polish Desktop Dashboards after LOO-497

Status: draft plan, reconciled October 23. PR #1660 merged the Dashboard
polish and the Desktop-helper fix. `LF_DESKTOP` targeting (7a21a557f9) and the
compress edits (`is_app_helper`, query-field seeding) are on this branch,
unpublished; they need a follow-up PR.

## Problem

Jack Heart merged LOO-497 (#1580) without a final demo. This Task collects the
polish. PR #1660 (merged) shipped the re-render cuts, working pin and layout saves,
started tiles that open rather than Start, Working Tasks zoom, the hover-only
`+`, and `lf desktop arrange` taking only `order`/`hidden` (`mode` is rejected).

Jack has already judged these in Loopflow Dev: terminal scrolling, the focus
spring and zoom regrid ("looks good"), and ⌘1–9 staying positional. Those
decisions stand.

Items, with status:

1. **CLI↔native targeting** (implemented on this branch, unpublished, not yet
   shown from a Dev Shell). Every `lf desktop` and `lf open` route sends to
   `com.loopflow.mac` (`open.rs` `DESKTOP_BUNDLE`, the `DESKTOP_EVENT`
   AppleScript). An agent inside Loopflow Dev (`com.loopflow.mac.dev`) that runs
   `lf desktop state` therefore inspects the installed app, or reports it as not
   running. The CLI and the native app can't be shown agreeing in a Dev build.
2. **Doubled frame.** Nobody has judged it yet. A Dashboard tile strokes
   `palette.border` (`DashboardView.swift:244`) around a workspace whose pane
   also strokes `palette.border` (`WorkspacePanesView.swift:375`).
3. **Open conversation inline.** Shipped in #1580. Jack hasn't seen it yet.
4. **LOO-497's Linear acceptance text** (done October 23): names LOO-519.
5. **Dev pairs with the installed CLI** (merged in #1660). On the main Home,
   `dispatch_default_cli` forwarded every command, including a Dev app's
   bundled lf, to the installed CLI. Dev's Swift then decoded frames from an
   older lf: v0.14.14 predates the Flow→Graph rename (#1643), so the Dashboard
   failed with `keyNotFound("graphs")`. Jack folded the fix into this Task
   (October 23).

## Demo

In Loopflow Dev, open a Dashboard tile's Shell and run `lf desktop state`. It
returns that Dev window's workspaces, with the same selection ID the sidebar
shows. `lf desktop focus …` from the same shell moves focus in Dev, not in the
installed app. Then look at a Dashboard tile with its conversation open inline:
is there one frame or two?

## Chosen approach

### Desktop names itself in `LF_DESKTOP`

Done in this branch (7a21a557f9). The `"1"` fallback is marked LOO-571.
Not yet shown: from this headless run, Dev's inspection reached Dev
("Loopflow Dev got an error: AppleEvent timed out") but returned nothing,
probably because Automation consent for this caller is pending. Confirm in
the demo from a Dev Shell.

Desktop already sets `LF_DESKTOP=1` on every child
(`ProcessEnvironment.swift:62`), and `lf open` reads its presence to mean "open
beside me". Set it to `Bundle.main.bundleIdentifier` instead. In lf, one
resolver chooses the target:

```rust
/// The Desktop a caller sits in, else the installed app.
pub(crate) fn desktop_bundle() -> String  // LF_DESKTOP if it names a bundle, else com.loopflow.mac
```

`"1"` (a released Desktop's value) and an empty value resolve to
`com.loopflow.mac`. That bounded case covers a branch lf running inside a
released app. No new flag. From an outside terminal, `LF_DESKTOP=com.loopflow.mac.dev lf desktop state`
targets Dev explicitly, which is the same variable with the same meaning.

Consumers that switch to the resolver:
- `desktop.rs` `DESKTOP_EVENT`: pass the bundle as `argv` item 1 and use
  `application id bundle`. Caller text still never enters the script source.
- `desktop.rs` `open_url`, and `open.rs` `DesktopBatch` (`installed()` and `send`).

These stay on the installed constant: `install.rs` quit and `link_receiver.rs`
scheme ownership. Both concern the installed app, not the caller's Desktop.

**Risk resolved (probe, October 23):** AppleScript compiles raw events against a
runtime bundle id. `osascript … 'tell application id b to return «event CNRTinsp»' com.loopflow.mac.dev`
returned Dev's live inspection JSON. Running-state checks work the same way.

### A Desktop helper runs itself

Done in this branch. #1381 (LOO-342) needed one Home, not one binary. An lf at
`<App>.app/Contents/MacOS/lf` is its Desktop's own build, so
`dispatch_default_cli` keeps it, and `resolve_lf_binary` hands it to children
on the main Home. Desktop already exports `LF_BIN` and puts the helper first on
PATH in its shells, so agents and Flows started from Dev run Dev's lf. Draft
migrations still route to a store copy first. A released app's helper is
checked to be the promoted revision (`install.rs`), so released behavior is
unchanged. An app left open across `lf install` now runs its own older lf
until relaunch; the store's reader floor names that skew.

### Dashboard header and New Dashboard (Jack, October 23, in Dev)

Done in this branch:
- "Working Tasks · 9" repeated the breadcrumb; it's gone.
- The query now sits in the header on every board, editable in place. Return
  saves a Dashboard's query; on Working Tasks or an unsaved view it opens the
  edited query as a view, which Save Dashboard can keep.
- The `+` vanished when the pointer reached it: as a bordered button it
  tracked the pointer itself, ending the section's hover. It's plain now.

### Doubled frame: judge, then pick one owner

If Jack sees a doubled frame, the fix is a single owner. A tile that hosts a
workspace drops its own stroke, and the pane's focused/unfocused stroke becomes
the tile's frame. LOO-492's even-clearance rule still holds: derive tile radius
from pane radius plus inset, never by padding alone. If Jack sees one frame, no
code changes.

### Linear text

Done October 23: LOO-497's scripting-parity bullet now names LOO-519.

## Alternatives rejected

- **`--app`/`LF_DESKTOP_BUNDLE` flag.** It adds a second knob for what
  `LF_DESKTOP` already means, and agents inside Dev would still need it set.
- **Probe running Loopflow bundles and pick one.** With Dev and the installed app
  both open, it's ambiguous, and it guesses where the caller already knows.

## Scope

Exclusions, each owned elsewhere:
- Cold Shell reattachment: LOO-416.
- Codex "Server connection could not be restored" in tiles, where lf reports the
  Session live: LOO-312.
- Pid-less Codex Resume: LOO-389.
- Remaining render items (`WaveDetailSnapshot` Equatable, `ConsoleCommandActions`
  per render, RepoView `workScope`, divider drag, body formatters): file a Task
  if Jack wants them. They are not visible defects today.

## Delete — do not maintain

- `LF_DESKTOP` as a presence-only boolean (`"1"`). The value now carries
  identity. The `"1"` → installed default stays only while released Desktop
  builds set it. Mark it with the Task that removes it, per AGENTS.md.
- Direct uses of `open::DESKTOP_BUNDLE` in routing paths. The constant stays the
  resolver's default and install's target.

## Forbidden outcomes

- Two resolvers, for example `open` honoring `LF_DESKTOP` while `desktop` stays
  hardcoded.
- A Dev-only code path or `#if DEBUG` bundle switch in Rust.

## Done when

- Loopflow Dev on the main Home shows the Dashboard rather than a decode error,
  and its Shells' `lf --version` reports the Dev build. (Merged in #1660;
  not yet confirmed in Dev.)
- `lf desktop state` from a shell inside Loopflow Dev returns Dev's windows, and
  from an ordinary terminal it still targets the installed app.
- Rust unit test: `desktop_bundle()` resolves an unset value, `"1"` and an empty
  value to `com.loopflow.mac`, and `com.loopflow.mac.dev` to itself.
- Swift test: `ProcessEnvironment` children carry the app's bundle identifier in
  `LF_DESKTOP`.
- Jack has judged the doubled frame and inline Open conversation in a demo, with
  any frame fix applied.
- LOO-497's Linear acceptance text names LOO-519.

Gate commands:
`cargo test -p loopflow desktop_bundle < /dev/null`;
`cargo clippy -p loopflow --all-targets -- -D warnings`;
the Swift headless suite filtered to `ProcessEnvironment`.

## Checks

- Merged in #1660: desktop-helper tests, clippy, Swift `build-for-testing`.
- Realign (Oct 23): `cargo fmt --check` + `cargo test -p loopflow --lib -- desktop_helper desktop_bundle private_home_keeps`: 3 passed. Clippy and Swift `ProcessEnvironment`: left to gate.
