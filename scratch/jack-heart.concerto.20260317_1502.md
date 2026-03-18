# Connections Panel Redesign + Secrets Provider

Two pieces that belong together: secrets provider needs a home, and the connections panel needs structure.

## Problem

ConnectionSettingsView is a flat list: mode picker, status dot, provider logins, secrets, harnesses. Everything at the same level. Secrets provider feels bolted on because there's no grouping that says "these things are related."

## Design

Group providers by **role**. Each group handles both auth and enable/disable.

### Provider Roles

```
Agents
├── Claude       ● Active    [Enabled]
├── Codex        ○ —         [Connect]
└── OpenCode     ○ —         [Connect]

Source Control
└── GitHub       ● Active    [Enabled]

Project Management
├── Linear       ○ —         [Connect]
└── Asana        ○ —         [Connect]

Secrets
└── Doppler      ● Active    dev / myapp
                              Config: [dev ▾]   [Refresh]
```

Each provider row: icon, name, auth status dot, action button. Secrets group is special — when connected, expands to show project/config selection inline.

### Per-Repo Connections

Keep the connections panel per-repo. The panel groups providers by role and each repo can:
- Enable/disable providers
- Override secrets config (Doppler project/config)

Portfolio gets its own connections panel that sets user-level defaults. Repos inherit those defaults.

### Two Panels, Same Component

`ConnectionsPanel` renders grouped providers. Takes a `scope`:

- `.global` — Portfolio panel. Sets defaults. All groups visible.
- `.repo` — Repo panel. Inherits global auth. Secrets group shows per-repo config.

In `.repo` scope, auth status comes from the server (same as now). The panel lets you enable/disable and configure per repo.

## Model Changes

### AuthProvider gets a role

```swift
public enum ProviderRole: String, Codable, Sendable {
    case agent
    case sourceControl
    case projectManagement
    case secrets
}

extension AuthProvider {
    public var role: ProviderRole { ... }
}
```

Grouping is a view concern driven by this property.

### Enable/disable state

Per-repo provider enablement. Persisted alongside connection config. Simple `Set<AuthProvider>` of disabled providers (enabled by default).

## View Structure

```
ConnectionsPanel
├── ConnectionModeSection (bundled/remote — existing, top of panel)
├── ProviderGroupSection(role: .agent)
│   └── ProviderRow per agent provider
├── ProviderGroupSection(role: .sourceControl)
│   └── ProviderRow per source control provider
├── ProviderGroupSection(role: .projectManagement)
│   └── ProviderRow per PM provider
└── ProviderGroupSection(role: .secrets)
    └── SecretsProviderSection (existing, now has a home)
```

`ProviderRow` replaces `AuthProviderCard`. Same capabilities (connect/disconnect, status display) plus enable/disable toggle. Tighter — no card border per provider, the group provides the visual container.

## What's NOT Changing

- `AuthProviderStore`, `SecretsProviderStore` internals
- lfd API — no backend changes
- `ConnectionStore` bundled/remote mode
- The secrets provider Rust/API work already on this branch

## Secrets Provider (already built)

- Doppler as OAuth provider (DopplerAuthBroker, auto-detect from CLI)
- Project/config discovery via Doppler API
- Smart config defaults (dev > prd > prod)
- Auto-persist CLI tokens in resolve_snapshot
- Swift models, store, tests all passing

## Implementation Order

1. Add `ProviderRole` to `AuthProvider`
2. Build `ProviderGroupSection` — groups providers under a role header
3. Build `ProviderRow` — replaces AuthProviderCard, adds enable/disable
4. Build `ConnectionsPanel` — assembles groups, embeds SecretsProviderSection
5. Replace ConnectionSettingsView body with ConnectionsPanel
6. Add ConnectionsPanel to PortfolioWindow (toolbar → sheet)

## Open Questions

- Portfolio panel location: toolbar button → sheet? Dedicated sidebar item?
- Per-repo enable/disable: do we need this for v1, or is global enough?
