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
    @State private var openTarget: OpenHandoffTarget?

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
                    if census.isEmpty && reading.notices.isEmpty {
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
        .sheet(item: $openTarget) { target in
            HandoffAttachSheet(sessionId: target.id, query: query) { openTarget = nil }
        }
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
        openTarget = OpenHandoffTarget(id: sessionId)
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
                notices: notices,
                error: nil
            )
        } catch {
            reading = ActiveSessionsReading(
                census: nil,
                notices: [],
                error: "Active Sessions unavailable: \(error.localizedDescription)"
            )
        }
    }
}

private struct ActiveSessionsReading {
    var census: ActiveSessionsCensus?
    var notices: [String] = []
    var error: String?
}

private struct OpenHandoffTarget: Identifiable {
    let id: String
}

// MARK: - One Wave's group

private struct WaveGroupView: View {
    let group: ActiveSessionWaveGroup
    let onOpen: (String) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
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
                    depth: depth(of: row),
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
    private func depth(of row: ActiveSessionRow) -> Int {
        var depth = 0
        var current = row.parentRowId
        let byId = Dictionary(group.rows.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
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

// MARK: - Open: re-attach the exact durable Session

/// Open records first-attach evidence through `lf handoff attach` and embeds the
/// returned descriptor in Ghostty. The Session is the vendor's; this view owns no
/// lifecycle — it renders the argv the contract hands back.
private struct HandoffAttachSheet: View {
    let sessionId: String
    let query: RegistryQuery
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @State private var attach: InteractiveHandoffAttach?
    @State private var error: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Interactive handoff")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                Spacer()
                Button(action: onClose) {
                    Image(systemName: "xmark").font(Typography.caption())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close handoff")
            }
            .padding(Spacing.md)
            Divider()
            content
        }
        .frame(minWidth: 720, minHeight: 460)
        .background(palette.background)
        .task { await attachSession() }
    }

    @ViewBuilder
    private var content: some View {
        if let attach {
            GhosttyTerminalView(
                workingDirectory: attach.cwd,
                argv: attach.argv,
                env: attach.environment,
                sessionId: "handoff-\(attach.sessionId)"
            )
            .id(attach.sessionId)
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

    private func attachSession() async {
        do {
            attach = try await query.attachHandoff(sessionId: sessionId)
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}

#endif
