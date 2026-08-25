import Charts
import Loopflow
import SwiftUI

/// Direct Run usage evidence beside codebase size and local ledger health.
struct TelemetryDashboardView: View {
    @Environment(\.palette) private var palette

    /// The codebase moves on a slower clock than a day's runs: a year shows the
    /// shape of the thing (a rewrite, a vendored tree), where a month shows noise.
    private static let codebaseDays = 365

    @State private var usage: [RunSnapshot] = []
    @State private var doctor: DoctorReport?
    @State private var codebase: CodeNode?
    @State private var growth: [CodeSnapshot] = []
    @State private var selectedRepo: String?
    /// Repo-relative path of the subtree the flame is zoomed into. Empty is the
    /// whole repo.
    @State private var focusPath: String = ""
    @State private var errorMessage: String?
    @State private var codebaseError: String?
    @State private var isLoading = true

    /// Repos recent Run manifests name, by absolute path.
    private var repos: [String] {
        Array(Set(usage.compactMap(\.repo))).sorted()
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xl) {
                    ledgerHealth

                    chartCard(
                        "Direct Run usage · 30 days",
                        subtitle: "Provider-authored cumulative counters. Dashes are unknown; final receipts and evidence gaps remain explicit."
                    ) {
                        DirectRunUsageList(runs: usage, repo: selectedRepo)
                    }

                    chartCard(
                        "Codebase over time · 12 months",
                        subtitle: "What a model pays to read this repo, by file extension"
                    ) {
                        CodebaseGrowthChart(snapshots: growth, failure: codebaseError)
                            .frame(height: 260)
                    }

                    chartCard(
                        codebaseTitle,
                        subtitle: "Files on disk. Width is tokens. Click a directory to zoom in."
                    ) {
                        VStack(alignment: .leading, spacing: Spacing.md) {
                            breadcrumb
                            CodeFlame(
                                root: focusedNode,
                                failure: codebaseError,
                                onSelect: { node in focusPath = node.path }
                            )
                            .frame(minHeight: 120)
                        }
                    }

                }
                .padding(Spacing.xxl)
            }
        }
        .background(palette.background)
        .task { await refresh() }
    }

    private var header: some View {
        HStack(spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Telemetry")
                    .font(Typography.heroTitle(26))
                    .foregroundStyle(palette.text)
                Text("Where the tokens went, and whether the ledger heard it.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            Spacer()
            if repos.count > 1 {
                Picker("Repo", selection: repoSelection) {
                    ForEach(repos, id: \.self) { repo in
                        Text(shortRepoName(repo)).tag(Optional(repo))
                    }
                }
                .labelsHidden()
                .frame(maxWidth: 220)
            }
            Text(totalLabel)
                .font(Typography.code(12))
                .foregroundStyle(palette.textSecondary)
            Button {
                Task { await refresh() }
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .buttonStyle(.plain)
            .disabled(isLoading)
            .help("Refresh telemetry")
        }
        .padding(.horizontal, Spacing.xxl)
        .padding(.vertical, Spacing.lg)
    }

    private var codebaseTitle: String {
        guard let node = focusedNode else { return "Codebase flame" }
        return "Codebase flame · \(node.lines.formatted()) lines · \(compactTokens(node.tokens)) tokens"
    }

    /// User selection owns one codebase load. Programmatic selection during a
    /// refresh assigns `selectedRepo` directly and the refresh awaits its load,
    /// avoiding two concurrent year-long history walks on first launch.
    private var repoSelection: Binding<String?> {
        Binding(
            get: { selectedRepo },
            set: { repo in
                selectedRepo = repo
                focusPath = ""
                codebase = nil
                growth = []
                Task { await loadCodebase() }
            }
        )
    }

    /// The subtree the flame is showing. A focus path that no longer exists (the
    /// repo changed under us) falls back to the whole repo rather than nothing.
    private var focusedNode: CodeNode? {
        guard let codebase else { return nil }
        guard !focusPath.isEmpty else { return codebase }
        return Self.find(focusPath, in: codebase) ?? codebase
    }

    private static func find(_ path: String, in node: CodeNode) -> CodeNode? {
        if node.path == path { return node }
        // Only descend where the path can still live.
        guard node.path.isEmpty || path.hasPrefix(node.path + "/") else { return nil }
        for child in node.children {
            if let hit = find(path, in: child) { return hit }
        }
        return nil
    }

    @ViewBuilder
    private var breadcrumb: some View {
        HStack(spacing: Spacing.xs) {
            crumb(label: codebase?.name ?? "repo", path: "")
            ForEach(crumbs, id: \.path) { crumb in
                Text("/").foregroundStyle(palette.textSecondary)
                self.crumb(label: crumb.name, path: crumb.path)
            }
            Spacer()
        }
        .font(Typography.code(11))
    }

    private var crumbs: [(name: String, path: String)] {
        guard !focusPath.isEmpty else { return [] }
        var prefix = ""
        return focusPath.split(separator: "/").map { component in
            prefix = prefix.isEmpty ? String(component) : "\(prefix)/\(component)"
            return (String(component), prefix)
        }
    }

    private func crumb(label: String, path: String) -> some View {
        Button(label) { focusPath = path }
            .buttonStyle(.plain)
            .foregroundStyle(path == focusPath ? palette.text : palette.textSecondary)
    }

    private func shortRepoName(_ repo: String) -> String {
        repo.split(separator: "/").last.map(String.init) ?? repo
    }

    private var totalLabel: String {
        let gaps = usage.reduce(0) { $0 + $1.evidenceGaps }
        return "\(usage.count) Runs · \(gaps) evidence gaps"
    }

    @ViewBuilder
    private var ledgerHealth: some View {
        if let errorMessage {
            Text(errorMessage)
                .font(Typography.caption())
                .foregroundStyle(Color.statusError)
        } else if let doctor {
            HStack(spacing: Spacing.sm) {
                ForEach(doctor.checks) { check in
                    HStack(spacing: Spacing.xs) {
                        Circle()
                            .fill(healthColor(check.status))
                            .frame(width: 7, height: 7)
                        Text(check.name)
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.text)
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(palette.surfaceMuted, in: Capsule())
                    .help(check.detail)
                }
                Spacer()
                Text("\(doctor.rows) events")
                    .font(Typography.code(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    private func chartCard<Content: View>(
        _ title: String,
        subtitle: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.text)
                Text(subtitle)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            content()
        }
        .padding(Spacing.lg)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.lg))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        }
    }

    private func refresh() async {
        isLoading = true
        defer { isLoading = false }
        do {
            usage = try await RegistryQueryLocal.shared.usage()
            errorMessage = nil
            do {
                doctor = try await RegistryQueryLocal.shared.doctor()
            } catch {
                doctor = nil
                errorMessage = error.localizedDescription
            }
        } catch {
            errorMessage = error.localizedDescription
        }
        if selectedRepo == nil || !repos.contains(selectedRepo ?? "") {
            selectedRepo = repos.first
        }
        await loadCodebase()
    }

    /// A repo the ledger remembers may no longer exist on disk (a worktree that
    /// was removed). That is a missing chart — but say why, because a silently
    /// empty chart is indistinguishable from a codebase of size zero.
    private func loadCodebase() async {
        guard let repo = selectedRepo else {
            codebase = nil
            growth = []
            codebaseError = "No repo in the ledger to measure"
            return
        }
        do {
            let nextCodebase = try await RegistryQueryLocal.shared.codebase(repoPath: repo)
            guard selectedRepo == repo else { return }
            let nextGrowth = try await RegistryQueryLocal.shared.codebaseHistory(
                repoPath: repo, days: Self.codebaseDays
            )
            guard selectedRepo == repo else { return }
            codebase = nextCodebase
            growth = nextGrowth
            codebaseError = nil
        } catch {
            guard selectedRepo == repo else { return }
            codebase = nil
            growth = []
            codebaseError = error.localizedDescription
        }
    }

    private func healthColor(_ status: String) -> Color {
        switch status {
        case "ok": .statusSuccess
        case "warn": .statusWarning
        default: .statusError
        }
    }
}

private struct DirectRunUsageList: View {
    @Environment(\.palette) private var palette
    let runs: [RunSnapshot]
    let repo: String?

    private var visible: [RunSnapshot] {
        runs.filter { repo == nil || $0.repo == repo }.prefix(30).map { $0 }
    }

    var body: some View {
        if visible.isEmpty {
            EmptyChartHint(message: "No direct Run usage in this window")
        } else {
            VStack(spacing: 0) {
                row("RUN", "INPUT", "OUTPUT", "FINAL", "GAPS", heading: true)
                ForEach(visible) { run in
                    Divider()
                    row(
                        run.skill ?? run.harness,
                        tokens(run.usage.inputTokens),
                        tokens(run.usage.outputTokens),
                        "\(run.usage.finalStreams)/\(run.usage.streams)",
                        run.evidenceGaps.formatted(),
                        heading: false
                    )
                    .help(run.id)
                }
            }
        }
    }

    private func tokens(_ value: Int?) -> String {
        value?.formatted() ?? "—"
    }

    private func row(
        _ run: String,
        _ input: String,
        _ output: String,
        _ finality: String,
        _ gaps: String,
        heading: Bool
    ) -> some View {
        HStack(spacing: Spacing.md) {
            Text(run).frame(maxWidth: .infinity, alignment: .leading)
            Text(input).frame(width: 100, alignment: .trailing)
            Text(output).frame(width: 100, alignment: .trailing)
            Text(finality).frame(width: 70, alignment: .trailing)
            Text(gaps).frame(width: 60, alignment: .trailing)
        }
        .font(heading ? Typography.caption(10) : Typography.code(12))
        .foregroundStyle(heading ? palette.textSecondary : palette.text)
        .padding(.vertical, Spacing.sm)
    }
}

/// Integer division made 1,802,919 and 1,033,737 both read "1M" — two bars that
/// differ by 770k tokens wearing the same label. Keep a decimal in the millions.
private func compactTokens(_ tokens: Int) -> String {
    switch tokens {
    case 1_000_000...:
        return String(format: "%.1fM", Double(tokens) / 1_000_000)
    case 10_000...:
        return "\(tokens / 1_000)k"
    case 1_000...:
        return String(format: "%.1fk", Double(tokens) / 1_000)
    default:
        return "\(tokens)"
    }
}

// MARK: - Codebase: growth over time, and the flame on disk

/// Stacked by file extension. Lines are what a human counts; tokens are what a
/// run costs, and they disagree — a lockfile is cheap in lines and ruinous here.
/// This plots the number the context budget spends.
private struct CodebaseGrowthChart: View {
    @Environment(\.palette) private var palette
    let snapshots: [CodeSnapshot]
    var failure: String?

    private struct Point: Identifiable {
        let id: String
        let date: Date
        let ext: String
        let tokens: Int
    }

    private var points: [Point] {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return snapshots.flatMap { snapshot -> [Point] in
            guard let date = formatter.date(from: snapshot.date) else { return [] }
            return snapshot.slices.map { slice in
                Point(
                    id: "\(snapshot.commit)-\(slice.ext)",
                    date: date,
                    ext: slice.ext,
                    tokens: slice.tokens
                )
            }
        }
    }

    var body: some View {
        if points.isEmpty {
            EmptyChartHint(
                message: failure ?? "No git history in this window",
                hint: "`lf tokens --days 30` walks the repo's commits"
            )
        } else {
            Chart(points) { point in
                AreaMark(
                    x: .value("Day", point.date, unit: .day),
                    y: .value("Tokens", point.tokens)
                )
                .foregroundStyle(by: .value("Extension", point.ext))
            }
            .chartLegend(position: .bottom, spacing: Spacing.sm)
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(palette.border)
                    AxisValueLabel {
                        if let tokens = value.as(Int.self) {
                            Text(compactTokens(tokens))
                        }
                    }
                }
            }
        }
    }
}

/// An icicle over the files on disk. Each node's bar fills the width its parent
/// allotted it, and its children tile that width in proportion to their tokens.
/// A child can never be wider than its parent — which is what makes the picture
/// readable, and what the first version got wrong by indenting each row while
/// still sizing it against the root.
///
/// Width is tokens, never lines and never time.
private struct CodeFlame: View {
    let root: CodeNode?
    var failure: String?
    var onSelect: (CodeNode) -> Void = { _ in }

    var body: some View {
        if let root, root.tokens > 0 {
            GeometryReader { geometry in
                IcicleNode(node: root, width: geometry.size.width, depth: 0, onSelect: onSelect)
            }
            .frame(height: flameHeight(root))
        } else {
            EmptyChartHint(
                message: failure ?? "No codebase measured",
                hint: "`lf tokens` measures the selected repo"
            )
        }
    }

    private func flameHeight(_ root: CodeNode) -> CGFloat {
        CGFloat(min(depth(of: root), maxFlameDepth + 1)) * (barHeight + barGap)
    }

    private func depth(of node: CodeNode) -> Int {
        1 + (node.children.map(depth).max() ?? 0)
    }
}

/// Below this the bars are thinner than their labels, and a chart nobody can
/// read is worse than one that stops.
private let maxFlameDepth = 3
private let barHeight: CGFloat = 22
private let barGap: CGFloat = 2
/// A segment narrower than this cannot show even one character; drawing it adds
/// noise, not information.
private let minSegmentWidth: CGFloat = 3

private struct IcicleNode: View {
    @Environment(\.palette) private var palette
    let node: CodeNode
    let width: CGFloat
    let depth: Int
    let onSelect: (CodeNode) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: barGap) {
            bar
            if depth < maxFlameDepth, node.tokens > 0 {
                HStack(alignment: .top, spacing: barGap) {
                    ForEach(visibleChildren, id: \.node.id) { child in
                        IcicleNode(
                            node: child.node,
                            width: child.width,
                            depth: depth + 1,
                            onSelect: onSelect
                        )
                        .frame(width: child.width, alignment: .leading)
                    }
                }
                .frame(width: width, alignment: .leading)
            }
        }
        .frame(width: width, alignment: .leading)
    }

    /// A leaf is a file: there is nothing to zoom into, so it does not pretend
    /// to be clickable.
    private var isZoomable: Bool { !node.children.isEmpty }

    private var bar: some View {
        ZStack(alignment: .leading) {
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .fill(depthColor.opacity(0.85))
            if width > 44 {
                Text("\(node.name)  \(compactTokens(node.tokens))")
                    .font(Typography.code(11))
                    .foregroundStyle(palette.text)
                    .padding(.leading, Spacing.xs)
                    .lineLimit(1)
                    .allowsTightening(true)
            }
        }
        .frame(width: width, height: barHeight)
        .contentShape(Rectangle())
        .onTapGesture { if isZoomable { onSelect(node) } }
        .pointerStyle(isZoomable ? .link : .default)
        .help("\(node.path.isEmpty ? node.name : node.path): \(node.tokens.formatted()) tokens · \(node.lines.formatted()) lines")
    }

    /// Children share exactly the width this node was given, minus the gaps
    /// between them, so a subtree always fits inside its parent.
    private var visibleChildren: [(node: CodeNode, width: CGFloat)] {
        guard node.tokens > 0, !node.children.isEmpty else { return [] }
        let gaps = CGFloat(max(node.children.count - 1, 0)) * barGap
        let usable = max(width - gaps, 0)
        return node.children.compactMap { child in
            let childWidth = usable * CGFloat(child.tokens) / CGFloat(node.tokens)
            guard childWidth >= minSegmentWidth else { return nil }
            return (child, childWidth)
        }
    }

    private var depthColor: Color {
        let ramp: [Color] = [.loopflowBurgundy, .statusInfo, .statusSuccess, .statusWarning]
        return ramp[min(depth, ramp.count - 1)]
    }
}

private struct EmptyChartHint: View {
    @Environment(\.palette) private var palette

    var message: String = "No recorded spend in this window"
    var hint: String = "Run `lf doctor` to check the ledger is being written"

    var body: some View {
        VStack(spacing: Spacing.xs) {
            Text(message)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            Text(hint)
                .font(Typography.code(11))
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, minHeight: 120)
    }
}
