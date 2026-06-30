# Launchboard navigation — deep-link + menu bar

Two navigation primitives so Concerto can be pointed at a specific repo (Cadenza)
and move between projects — without relaunching from the CLI. Productizes the
`--repo` flag we've been hand-cranking through the dev script.

## What to build

1. **URL scheme** — `loopflow://open?repo=<path>` opens/focuses a repo window;
   `loopflow://portfolio` shows the overview.
2. **Menu bar** — "Portfolio" + "Move to Repo ▸ <known repos>" + "Open Repo…".

## Grounding (nearly all the scaffolding exists)

- Scheme **`loopflow://`** already registered — `Concerto/Info.plist:21-31`
  (`studio.loopflow.auth`). OAuth uses an ephemeral `ASWebAuthenticationSession`
  callback, so there's **no app-level `.onOpenURL` yet** → adding one for nav is
  conflict-free.
- `--repo` parsed at `ConcertoApp.swift:108-116`; main `WindowGroup` routes
  `RepoWindow` (with `--repo`) vs `PortfolioWindow` (without) — `ConcertoApp.swift:190-203`.
- Reusable repo window: `WindowGroup(id: "repo", for: URL.self)` — `ConcertoApp.swift:213`.
  Open any repo via `openWindow(id: "repo", value: url)` (`@Environment(\.openWindow)`
  already in scope, line 172).
- `PortfolioService` — `Concerto/Platform/macOS/Services/PortfolioService.swift`:
  persisted repo list in UserDefaults; `addRepo(url)`, `removeRepo`, `repos`.
  `PortfolioWindow` already has `addRepo`/`openRepo` (lines 219/224).

## Design

**URL scheme** — `.onOpenURL` on the main WindowGroup content:
```swift
.onOpenURL { url in
    guard url.scheme == "loopflow" else { return }
    switch url.host {
    case "open":
        if let repo = URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?.first(where: { $0.name == "repo" })?.value {
            let repoURL = URL(fileURLWithPath: repo)
            portfolioService.addRepo(repoURL)        // register → joins the portfolio
            openWindow(id: "repo", value: repoURL)
        }
    case "portfolio":
        openWindow(id: "portfolio")
    default: break
    }
}
```

**Menu bar** — `.commands` on the Scene:
```swift
.commands {
    CommandMenu("Go") {
        Button("Portfolio") { openWindow(id: "portfolio") }
            .keyboardShortcut("0", modifiers: .command)
        Menu("Move to Repo") {
            ForEach(portfolioService.repos) { repo in
                Button(repo.displayName) {
                    openWindow(id: "repo", value: URL(fileURLWithPath: repo.path))
                }
            }
        }
        Button("Open Repo…") { /* NSOpenPanel → addRepo + openWindow(id:"repo") */ }
    }
}
```

One structural add: a dedicated portfolio window id. Today the portfolio is the
no-value main `WindowGroup`; for `openWindow(id:"portfolio")` add a
`Window("Portfolio", id: "portfolio")` (or convert the group).

## Coordination

The portfolio *view* is the other session's project (`loopflow.portfolio`). This
project owns the *entry points / navigation* — scheme, menu, open/focus — not the
dashboard internals. Clean seam.

## Done when

- `open "loopflow://open?repo=/Users/jack/src/cadenza"` brings Concerto to a
  Cadenza repo window and registers it in the portfolio.
- Menu "Go ▸ Portfolio" and "Go ▸ Move to Repo ▸ Cadenza" both work.
- OAuth login still works (scheme reuse didn't break the auth callback).

## Immediate peek (no build)

Relaunch Concerto **without** `--repo` → lands on Portfolio → Add Repo →
`~/src/cadenza` → open. Confirms the recut roadmap renders while the nav is built.
