#if os(macOS)
import Loopflow
import SwiftUI

/// Work Census: a machine-wide census of every live body, grouped by Wave.
/// The projection (`WorkCensus`) owns every rule — red propagation,
/// evidence classification, and which rows are view-only. This view only renders
/// what the census decided and, for a deliberate interactive launch, offers the
/// one mutation: Open, which re-attaches the exact Launch in Ghostty.
struct WorkCensusView: View {
    var query: RegistryQuery = RegistryQueryLocal.shared

    @Environment(\.palette) private var palette
    @State private var reading = WorkCensusReading()
    @State private var openTarget: LaunchSurfaceRecord?

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
                    // "No active work" is a *healthy* empty state, so it may
                    // only show when nothing is wrong: no live bodies, no scoped
                    // read failure, and no Wave whose evidence is unavailable.
                    let anyUnavailable = census.groups.contains { $0.evidence == .unavailable }
                    if census.isEmpty && reading.notices.isEmpty && !anyUnavailable {
                        emptyState
                    }
                    ForEach(census.groups) { group in
                        WaveGroupView(group: group, onOpen: openLaunch)
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
        .sheet(item: $openTarget) { launch in
            LaunchAttachSheet(launch: launch, query: query) { openTarget = nil }
        }
        .accessibilityIdentifier("control-work-census")
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("No active work")
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

    private func openLaunch(_ launchId: String) {
        // Open needs the durable row's provider, Home, and resume token to
        // resolve the surface; look it up from the same list the census read.
        openTarget = reading.launches.first { $0.id == launchId }
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
            let launches: [LaunchSurfaceRecord]
            do {
                launches = try await query.activeLaunches()
            } catch {
                launches = []
                notices.append("Interactive launches unavailable: \(error.localizedDescription)")
            }
            reading = WorkCensusReading(
                census: WorkCensus(roadmap: roadmap, runs: runs, launches: launches),
                launches: launches,
                notices: notices,
                error: nil
            )
        } catch {
            reading = WorkCensusReading(
                census: nil,
                launches: [],
                notices: [],
                error: "Work Census unavailable: \(error.localizedDescription)"
            )
        }
    }
}

private struct WorkCensusReading {
    var census: WorkCensus?
    var launches: [LaunchSurfaceRecord] = []
    var notices: [String] = []
    var error: String?
}

// MARK: - One Wave's group

private struct WaveGroupView: View {
    let group: WaveActivity
    let onOpen: (String) -> Void

    @Environment(\.palette) private var palette

    /// Parent index built once per group, so row indentation is an O(1) lookup.
    private var rowsById: [String: WorkActivity] {
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
                WorkActivityRowView(
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
    private static func depth(of row: WorkActivity, in byId: [String: WorkActivity]) -> Int {
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

private struct WorkActivityRowView: View {
    let row: WorkActivity
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
                if let reason = row.reason, !reason.isEmpty, row.kind != .launch {
                    Text(reason)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                }
            }

            Spacer(minLength: Spacing.sm)

            if row.isOpenable, let launchId = row.launchId {
                Button("Open") { onOpen(launchId) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .accessibilityLabel("Open interactive launch")
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
        case .launch: "hand.raised"
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
        if let owner = row.nextOwner, row.kind != .launch {
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
        case .blue: WaveLensColor.blue.glow
        case .black: palette.text
        case .neutral: .statusNeutral
        }
    }
}

private enum EvidenceStyle {
    static func badge(_ evidence: ActivityEvidence) -> (text: String, color: Color)? {
        switch evidence {
        case .observed: nil
        case .stale: ("stale", .statusWarning)
        case .stopped: ("stopped", .statusError)
        case .unreachable: ("unreachable", .statusWarning)
        case .unavailable: ("unavailable", .statusNeutral)
        case .missing: ("no bodies", .statusNeutral)
        }
    }

    static func groupEmptyPhrase(_ evidence: ActivityEvidence) -> String {
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

// MARK: - Open: present the exact Launch in the remembered surface

/// Open resolves *where* to present the launch — the last successful surface for
/// this provider on this Home, then the last overall, then embedded Ghostty — and
/// records the choice only after a launch succeeds. Every target attaches the one
/// Launch by running the argv the contract hands back; this view owns no
/// lifecycle and never creates or names the provider session.
private struct LaunchAttachSheet: View {
    let launch: LaunchSurfaceRecord
    let query: RegistryQuery
    let onClose: () -> Void
    var preferences: LaunchTargetPreferences = .shared

    @Environment(\.palette) private var palette
    @State private var attach: LaunchSurfaceRecord?
    @State private var capability: LaunchTargetCapability?
    @State private var surface: LaunchTarget?
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
            Text(launch.attention?.kind == "user" ? "Feedback" : "Interactive launch")
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
            .accessibilityLabel("Close launch")
        }
        .padding(Spacing.md)
    }

    /// The honest picker: only surfaces that can reach this launch, each labeled
    /// by the reach it delivers so a worktree-only option never overclaims.
    private func surfaceMenu(
        capability: LaunchTargetCapability,
        attach: LaunchSurfaceRecord
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
        .accessibilityLabel("Choose where to open the launch")
    }

    @ViewBuilder
    private var content: some View {
        if let attach, surface == .ghostty {
            let command = launch.attention?.kind == "user"
                ? LaunchTargetLauncher.feedbackCommand(for: attach)
                : LaunchTargetLauncher.command(for: attach, home: launch.home)
            GhosttyTerminalView(
                workingDirectory: command.cwd,
                argv: command.argv,
                env: command.environment,
                sessionId: "feedback-\(attach.launchId)"
            )
            .id(attach.launchId)
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
            let descriptor = try await query.attachLaunch(launchId: launch.id)
            attach = descriptor
            if descriptor.attention?.kind == "user" {
                surface = .ghostty
                externalNote = nil
                return
            }
            // Consume the descriptor's Home, not a local-only assumption: a remote
            // worktree makes local editors and plain windows unavailable. The
            // provider and session id determine whether an IDE can attach (Claude
            // with a known session id) or is worktree-only.
            let cap = LaunchTargetLauncher.capability(
                host: descriptor.host,
                cwd: descriptor.cwd,
                provider: launch.provider,
                providerSessionId: launch.providerSessionId
            )
            capability = cap
            let resolution = LaunchTargetResolver.resolve(
                provider: launch.provider,
                home: launch.home,
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

    /// Present the launch in `target`. Ghostty embeds; an external target
    /// launches through the shared command. The preference advances only when a
    /// user-initiated *attach* launch succeeds — an auto-resolved fallback never
    /// rewrites the remembered surface (so a briefly-unavailable app returns), and
    /// a worktree-only launch never overwrites the last valid attach preference
    /// (opening a folder is not the surface the human attaches through). A failed
    /// launch falls back visibly to the embedded terminal.
    private func present(
        _ target: LaunchTarget,
        attach: LaunchSurfaceRecord,
        capability: LaunchTargetCapability,
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

        let result = await LaunchTargetLauncher.launch(
            target,
            attach: attach,
            home: launch.home,
            reach: reach
        )
        switch result {
        case .attached:
            surface = target
            externalNote = "Attached in \(target.appName). Complete or hand back from there."
            if userInitiated { fallbackNotice = nil }
            recordIfEarned(target, reach: reach, userInitiated: userInitiated, launched: true)
        case .worktreeOnly:
            surface = target
            externalNote = "Opened the worktree in \(target.appName) — this does not attach the provider session."
            if userInitiated { fallbackNotice = nil }
            // A worktree-only outcome never overwrites the last attach preference.
        case .failed:
            // Visible fallback: embed Ghostty and leave the preference untouched.
            surface = .ghostty
            externalNote = nil
            fallbackNotice = "\(target.appName) unavailable — fell back to the embedded terminal."
        }
    }

    private func recordIfEarned(
        _ surface: LaunchTarget,
        reach: LaunchTargetReach,
        userInitiated: Bool,
        launched: Bool
    ) {
        preferences.recordLaunch(
            surface,
            provider: launch.provider,
            home: launch.home,
            reach: reach,
            userInitiated: userInitiated,
            launchSucceeded: launched
        )
    }
}

#endif
