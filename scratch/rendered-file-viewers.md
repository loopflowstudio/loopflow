# Rendered file viewers — 2026-11-04

Jack Heart confirmed the development CLI opened the actual docs, then requested
better rendered Markdown and HTML viewers. They must work cross-platform and
cross-machine. Ignore the withdrawn “vim” message; no Vim-specific direction
was accepted. Jack subsequently proposed making automatic file opening exclusive
to Loopflow Desktop. That proposal is being decided in the primary conversation;
it has not yet changed the accepted design or shipping scope.

## Evidence

- Installed Rex `block_creator.list` exposes terminal and onboarding blocks,
  with no Markdown, HTML or browser creator. Terminal editing was demonstrated;
  rendered viewing is a distinct capability.
- Desktop `TaskFilesStore.rendersSelection` selects Markdown/HTML in read mode;
  `Views/TaskFilesView.swift` uses `TaskFileReadingView`. Existing `RegistryQuery`
  provides remote file read, save and watch operations. Remote reading passes
  no local baseURL: relative images, styles and linked assets need explicit proof
  before claiming full remote document parity. The current client is macOS.
- [Markserv](https://github.com/markserv/markserv) renders Markdown to HTML,
  serves HTML/assets and reloads on saves. It is a possible browser preview
  prototype, not an established Loopflow Machine/auth/session integration.
- [VS Code Remote SSH](https://code.visualstudio.com/docs/remote/ssh) supports
  remote workspaces on Linux, macOS and Windows; its
  [integrated browser](https://code.visualstudio.com/docs/debugtest/integrated-browser)
  supports proxying HTTP(S) over the workspace connection, not file:// URLs.
  This is a concrete cross-machine reference, but making an IDE the required
  viewer would add a separate workspace owner to the flow.
- [Glow](https://github.com/charmbracelet/glow) is a rendered terminal Markdown
  reader. It does not meet the general HTML/CSS/interactive-page requirement.
  Mac-only preview apps do not satisfy Jack's platform constraint.

## Proposed boundary

Write and preserve the real docs on every Machine. Automatically present only
inside an already attached Desktop Files view; otherwise provide paths while
implementation continues. Keep Task and file transport ownership unchanged.
This scopes automatic presentation to a capable existing host; it does not
claim Desktop is already a cross-platform client. Universal rendered viewing
would be separate work, likely browser-based, with relative assets, live reload,
remote access and draft preservation included in its outcome.

No new viewer package, server, public endpoint or host integration has been
installed or implemented during this research.
