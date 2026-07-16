#if os(macOS)
import Loopflow
import SwiftUI

/// Active Sessions: a machine-wide census of every live body, grouped by Wave.
/// The projection (`ActiveSessionsCensus`) owns every rule — red propagation,
/// evidence classification, and which rows are view-only. This view only renders
/// what the census decided and, for a deliberate interactive handoff, offers the
/// one mutation: Open, which re-attaches the exact durable Session in Ghostty.
struct ActiveSessionsView: View {
    var query: RegistryQuery = RegistryQueryLocal.shared

    @Environment(\.palette) private var palette
    @State private var reading = ActiveSessionsReading()
    @State private var openTarget: InteractiveHandoffListRow?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                if let error = reading.error {
                    notice(error, color: .statusError)
                }
                ForEach(reading.notices, id: \.self) { message in
                    notice(message, color: .statusWarning)
                }
                if let census = reading.census {
                    // "No active sessions" is a *healthy* empty state, so it may
                    // only show when nothing is wrong: no live bodies, no scoped
                    // read failure, and no Wave whose evidence is unavailable.
                    let anyUnavailable = census.groups.contains { $0.evidence == .unavailable }
                    if census.isEmpty && reading.notices.isEmpty && !anyUnavailable {
                        emptyState
                    }
                    ForEach(census.groups) { group in
                        WaveGroupView(group: group, onOpen: openHandoff)
                    }
                } else if reading.error == nil {
                    ProgressView().padding(Spacing.xxl)
                }
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task {
            while !Task.isCancelled {
                await load()
                try? await Task.sleep(for: .seconds(30))
            }
        }
        .refreshable { await load() }
        .sheet(item: $openTarget) { handoff in
            HandoffAttachSheet(handoff: handoff, query: query) { openTarget = nil }
        }
        .accessibilityIdentifier("control-active-sessions")
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("No active sessions")
                .font(Typography.sectionTitle(15))
                .foregroundStyle(palette.text)
            Text("Nothing is running across the machine right now.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
        .padding(Spacing.lg)
    }

    private func notice(_ message: String, color: Color) -> some View {
        Label(message, systemImage: "exclamationmark.triangle")
            .font(Typography.caption(11))
            .foregroundStyle(color)
            .textSelection(.enabled)
    }

    private func openHandoff(_ sessionId: String) {
        // Open needs the durable row's provider, Home, and provider session id to
        // resolve the surface; look it up from the same list the census read.
        openTarget = reading.handoffs.first { $0.sessionId == sessionId }
    }

    private func load() async {
        do {
            let roadmap = try await query.roadmap()
            var notices: [String] = []
            let runs: [SkillRunEntry]
            do {
                runs = try await query.recentRuns()
            } catch {
                runs = []
                notices.append("Direct executions unavailable: \(error.localizedDescription)")
            }
            let handoffs: [InteractiveHandoffListRow]
            do {
                handoffs = try await query.activeHandoffs()
            } catch {
                handoffs = []
                notices.append("Interactive handoffs unavailable: \(error.localizedDescription)")
            }
            reading = ActiveSessionsReading(
                census: ActiveSessionsCensus(roadmap: roadmap, runs: runs, handoffs: handoffs),
                handoffs: handoffs,
                notices: notices,
                error: nil
            )
        } catch {
            reading = ActiveSessionsReading(
                census: nil,
                handoffs: [],
                notices: [],
                error: "Active Sessions unavailable: \(error.localizedDescription)"
            )
        }
    }
}

private struct ActiveSessionsReading {
    var census: ActiveSessionsCensus?
    var handoffs: [InteractiveHandoffListRow] = []
    var notices: [String] = []
    var error: String?
}

// MARK: - One Wave's group

private struct WaveGroupView: View {
    let group: ActiveSessionWaveGroup
    let onOpen: (String) -> Void

    @Environment(\.palette) private var palette

    /// Parent index built once per group, so row indentation is an O(1) lookup.
    private var rowsById: [String: ActiveSessionRow] {
        Dictionary(group.rows.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    }

    var body: some View {
        let byId = rowsById
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            header
            if let reason = group.unavailableReason {
                Text(reason)
                    .font(Typography.caption(11))
                    .foregroundStyle(Color.statusWarning)
                    .textSelection(.enabled)
            }
            if group.rows.isEmpty && group.unavailableReason == nil {
                Text(EvidenceStyle.groupEmptyPhrase(group.evidence))
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            ForEach(group.rows) { row in
                SessionRowView(
                    row: row,
                    depth: Self.depth(of: row, in: byId),
                    onOpen: onOpen
                )
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.55))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Circle()
                .fill(TintStyle.color(group.tint, palette: palette))
                .frame(width: 9, height: 9)
                .accessibilityHidden(true)
            Text(group.waveName)
                .font(Typography.sectionTitle(16))
                .foregroundStyle(palette.text)
            if group.remote {
                Image(systemName: "network")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .help(group.home)
            }
            Spacer()
            if let badge = EvidenceStyle.badge(group.evidence) {
                EvidenceBadge(text: badge.text, color: badge.color)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Wave \(group.waveName), \(group.tint.rawValue) lens, \(group.evidence.rawValue)")
    }

    /// Indent a row under the parent the census declared, by walking parent ids.
    private static func depth(of row: ActiveSessionRow, in byId: [String: ActiveSessionRow]) -> Int {
        var depth = 0
        var current = row.parentRowId
        while let parentId = current, let parent = byId[parentId], depth < 4 {
            depth += 1
            current = parent.parentRowId
        }
        return depth
    }
}

// MARK: - One body row

private struct SessionRowView: View {
    let row: ActiveSessionRow
    let depth: Int
    let onOpen: (String) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Circle()
                .fill(TintStyle.color(row.tint, palette: palette))
                .frame(width: 7, height: 7)
                .padding(.top, 5)
                .accessibilityHidden(true)
            Image(systemName: kindIcon)
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
                .frame(width: 16)
                .padding(.top, 2)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(spacing: Spacing.xs) {
                    Text(row.title)
                        .font(Typography.body(13))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                    if let badge = EvidenceStyle.badge(row.evidence) {
                        EvidenceBadge(text: badge.text, color: badge.color)
                    }
                }
                if let subtitle = row.subtitle, subtitle != row.title {
                    Text(subtitle)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                }
                if !metadata.isEmpty {
                    Text(metadata)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
                if let reason = row.reason, !reason.isEmpty, row.kind != .handoff {
                    Text(reason)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                }
            }

            Spacer(minLength: Spacing.sm)

            if row.isOpenable, let sessionId = row.handoffSessionId {
                Button("Open") { onOpen(sessionId) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .accessibilityLabel("Open interactive handoff")
                    .accessibilityHint(row.reason ?? "")
            }
        }
        .padding(Spacing.sm)
        .padding(.leading, CGFloat(depth) * Spacing.lg)
        .background(palette.background.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .accessibilityElement(children: .combine)
        .accessibilityLabel(row.accessibilityLabel)
    }

    private var kindIcon: String {
        switch row.kind {
        case .project: "square.stack.3d.up"
        case .task: "checklist"
        case .directExecution: "bolt"
        case .handoff: "hand.raised"
        }
    }

    /// Only the fields the contract actually carried — provider, model, Home,
    /// worktree, step, age, next owner. Absent fields stay absent.
    private var metadata: String {
        var parts: [String] = []
        if let provider = row.provider { parts.append(provider) }
        if let model = row.model { parts.append(model) }
        if let worktree = row.worktree {
            parts.append((worktree as NSString).lastPathComponent)
        }
        if let step = row.step { parts.append(step) }
        if let age = row.ageSecs { parts.append("\(RelativeAge.phrase(age)) ago") }
        if let owner = row.nextOwner, row.kind != .handoff {
            parts.append("→ \(owner.rawValue)")
        }
        return parts.joined(separator: " · ")
    }
}

// MARK: - Shared styling

private enum TintStyle {
    static func color(_ tint: AttentionTint, palette: LoopflowPalette) -> Color {
        switch tint {
        case .green: .statusSuccess
        case .red: .statusError
        case .black: palette.text
        case .neutral: .statusNeutral
        }
    }
}

private enum EvidenceStyle {
    static func badge(_ evidence: SessionEvidence) -> (text: String, color: Color)? {
        switch evidence {
        case .observed: nil
        case .stale: ("stale", .statusWarning)
        case .stopped: ("stopped", .statusError)
        case .unreachable: ("unreachable", .statusWarning)
        case .unavailable: ("unavailable", .statusNeutral)
        case .missing: ("no bodies", .statusNeutral)
        }
    }

    static func groupEmptyPhrase(_ evidence: SessionEvidence) -> String {
        evidence == .missing ? "No active bodies in this Wave." : "No rows to show."
    }
}

private struct EvidenceBadge: View {
    let text: String
    let color: Color

    var body: some View {
        Text(text)
            .font(Typography.caption(9))
            .foregroundStyle(color)
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, 1)
            .background(color.opacity(0.14))
            .clipShape(Capsule())
    }
}

private enum RelativeAge {
    static func phrase(_ seconds: Int) -> String {
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3600 { return "\(seconds / 60)m" }
        if seconds < 86400 { return "\(seconds / 3600)h" }
        return "\(seconds / 86400)d"
    }
}

// MARK: - Open: present the exact durable Session in the remembered surface

/// Open resolves *where* to present the handoff — the last successful surface for
/// this provider on this Home, then the last overall, then embedded Ghostty — and
/// records the choice only after a launch succeeds. Every target attaches the one
/// durable Session by running the argv the contract hands back; this view owns no
/// lifecycle and never creates or names the Session.
private struct HandoffAttachSheet: View {
    let handoff: InteractiveHandoffListRow
    let query: RegistryQuery
    let onClose: () -> Void
    var preferences: HandoffSurfacePreferences = .shared

    @Environment(\.palette) private var palette
    @State private var attach: InteractiveHandoffAttach?
    @State private var capability: HandoffSurfaceCapability?
    @State private var surface: HandoffSurface?
    @State private var externalNote: String?
    @State private var fallbackNotice: String?
    @State private var error: String?

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .frame(minWidth: 720, minHeight: 460)
        .background(palette.background)
        .task { await start() }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Text("Interactive handoff")
                .font(Typography.sectionTitle(15))
                .foregroundStyle(palette.text)
            if let fallbackNotice {
                Text(fallbackNotice)
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusWarning)
                    .lineLimit(1)
            }
            Spacer()
            if let capability, let attach {
                surfaceMenu(capability: capability, attach: attach)
            }
            Button(action: onClose) {
                Image(systemName: "xmark").font(Typography.caption())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Close handoff")
        }
        .padding(Spacing.md)
    }

    /// The honest picker: only surfaces that can reach this handoff, each labeled
    /// by the reach it delivers so a worktree-only option never overclaims.
    private func surfaceMenu(
        capability: HandoffSurfaceCapability,
        attach: InteractiveHandoffAttach
    ) -> some View {
        Menu {
            ForEach(capability.offeredOptions) { option in
                Button(option.label) {
                    Task { await present(option.surface, attach: attach, capability: capability, userInitiated: true) }
                }
            }
        } label: {
            Label("Open in \(surface?.appName ?? "…")", systemImage: "rectangle.on.rectangle")
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityLabel("Choose where to open the handoff")
    }

    @ViewBuilder
    private var content: some View {
        if let attach, surface == .ghostty {
            let command = HandoffSurfaceLauncher.command(for: attach, home: handoff.home)
            GhosttyTerminalView(
                workingDirectory: command.cwd,
                argv: command.argv,
                env: command.environment,
                sessionId: "handoff-\(attach.sessionId)"
            )
            .id(attach.sessionId)
        } else if let surface, let externalNote {
            ContentUnavailableView {
                Label("Presented in \(surface.appName)", systemImage: "arrow.up.forward.app")
            } description: {
                Text(externalNote).textSelection(.enabled)
            } actions: {
                if let attach, let capability {
                    Button("Open here in Ghostty instead") {
                        Task { await present(.ghostty, attach: attach, capability: capability, userInitiated: true) }
                    }
                }
            }
        } else if let error {
            ContentUnavailableView {
                Label("Could not attach", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error).textSelection(.enabled)
            }
        } else {
            ProgressView("Attaching…").padding(Spacing.xxl)
        }
    }

    private func start() async {
        do {
            let descriptor = try await query.attachHandoff(sessionId: handoff.sessionId)
            attach = descriptor
            // Consume the descriptor's Home, not a local-only assumption: a remote
            // worktree makes local editors and plain windows unavailable.
            let cap = HandoffSurfaceLauncher.capability(host: descriptor.host, cwd: descriptor.cwd)
            capability = cap
            let resolution = HandoffSurfaceResolver.resolve(
                provider: handoff.provider,
                home: handoff.home,
                memory: preferences.memory,
                capability: cap
            )
            // A remembered surface that could not be honored names itself and why;
            // show that reason so the fallback is never silent.
            fallbackNotice = resolution.fallbackReason
            await present(resolution.surface, attach: descriptor, capability: cap, userInitiated: false)
        } catch {
            self.error = error.localizedDescription
        }
    }

    /// Present the handoff in `target`. Ghostty embeds; an external target
    /// launches through the shared command. The preference advances only when a
    /// user-initiated *attach* launch succeeds — an auto-resolved fallback never
    /// rewrites the remembered surface (so a briefly-unavailable app returns), and
    /// a worktree-only launch never overwrites the last valid attach preference
    /// (opening a folder is not the surface the human attaches through). A failed
    /// launch falls back visibly to the embedded terminal.
    private func present(
        _ target: HandoffSurface,
        attach: InteractiveHandoffAttach,
        capability: HandoffSurfaceCapability,
        userInitiated: Bool
    ) async {
        error = nil
        let reach = capability.reach(target)

        if target == .ghostty {
            surface = .ghostty
            externalNote = nil
            // An explicit Ghostty choice clears a stale fallback reason; an
            // auto-resolved fallback keeps the reason set in `start()`.
            if userInitiated { fallbackNotice = nil }
            recordIfEarned(target, reach: reach, userInitiated: userInitiated, launched: true)
            return
        }

        let launched = await HandoffSurfaceLauncher.launch(
            target,
            attach: attach,
            home: handoff.home,
            reach: reach
        )
        if launched {
            surface = target
            externalNote = reach == .attach
                ? "Attached in \(target.appName). Complete or hand back from there."
                : "Opened the worktree in \(target.appName) — this does not attach the Session."
            if userInitiated { fallbackNotice = nil }
            recordIfEarned(target, reach: reach, userInitiated: userInitiated, launched: true)
        } else {
            // Visible fallback: embed Ghostty and leave the preference untouched.
            surface = .ghostty
            externalNote = nil
            fallbackNotice = "\(target.appName) unavailable — fell back to the embedded terminal."
        }
    }

    private func recordIfEarned(
        _ surface: HandoffSurface,
        reach: HandoffSurfaceReach,
        userInitiated: Bool,
        launched: Bool
    ) {
        preferences.recordLaunch(
            surface,
            provider: handoff.provider,
            home: handoff.home,
            reach: reach,
            userInitiated: userInitiated,
            launchSucceeded: launched
        )
    }
}

#endif
