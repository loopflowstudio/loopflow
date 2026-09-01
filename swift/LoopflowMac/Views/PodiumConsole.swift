// The Podium console: a burgundy path header showing the selected
// Wave › Project › Task, and cascading drawer columns that pull out beneath
// it. The hierarchy renders here and only here — the work surface below the
// header belongs to the selected node. Every row and every path segment
// carries a FaderSwitch, and pressing one actually spins the agent up or
// down through the same `lf` lifecycle verbs the CLI uses.

import Loopflow
import SwiftUI

/// Deepening shades carry depth: the selected row's background is its child
/// column's color, so the open cascade and the closed path read as one object.
private enum ConsoleShade {
    static let bar = Color(hex: 0x601F28)
    static let wave = Color.loopflowBurgundy
    static let project = Color(hex: 0x7E3944)
    static let task = Color(hex: 0x8A434E)
}

private let segmentTip: CGFloat = 12

struct PodiumConsole<Content: View>: View {
    @Bindable var model: PodiumModel
    @ViewBuilder let content: Content

    /// How many drawer columns are out: 0 closed, 1 waves, 2 +projects,
    /// 3 +tasks. Explored ids are the cascade's own path and may diverge from
    /// the committed selection while the user walks the tree.
    @State private var openDepth = 0
    @State private var exploredWaveId: String?
    @State private var exploredProjectId: String?
    @State private var activeControlId: String?
    @State private var controlError: String?

    var body: some View {
        VStack(spacing: 0) {
            pathBar
            if let controlError {
                errorBanner(controlError)
            }
            ZStack(alignment: .topLeading) {
                content
                if openDepth > 0 {
                    Rectangle()
                        .fill(Color.black.opacity(0.18))
                        .onTapGesture { closeDrawers() }
                        .accessibilityHidden(true)
                        .transition(.opacity)
                    drawerColumns
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
            .animation(.easeOut(duration: 0.2), value: openDepth)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-console")
    }

    // MARK: - Path bar

    private var pathBar: some View {
        HStack(spacing: -segmentTip) {
            rootSegment
                .zIndex(4)
            ForEach(Array(pathNodes.enumerated()), id: \.element.id) { index, node in
                pathSegment(node, position: index)
                    .zIndex(Double(3 - index))
            }
            Spacer(minLength: segmentTip)
            if model.waves.errorMessage != nil || model.roadmap.errorMessage != nil {
                Label("Evidence unavailable", systemImage: "exclamationmark.triangle.fill")
                    .font(Typography.caption(9).weight(.semibold))
                    .foregroundStyle(Color(hex: 0xF2C36B))
                    .lineLimit(1)
                    .help(model.waves.errorMessage ?? model.roadmap.errorMessage ?? "")
                    .padding(.trailing, Spacing.lg)
                    .accessibilityIdentifier("podium-console-error")
            }
            if visibleWaves.isEmpty, !model.waves.isLoading {
                Text("No Waves found.")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.58))
                    .padding(.trailing, Spacing.lg)
                    .accessibilityIdentifier("podium-console-empty")
            }
        }
        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
        .background(ConsoleShade.bar)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-console-path")
    }

    private var rootSegment: some View {
        segmentChip(
            kicker: "WAVES",
            title: "\(visibleWaves.count)",
            shade: ConsoleShade.wave,
            isOpen: openDepth == 1,
            leadingPad: Spacing.lg,
            fader: nil,
            accessibilityId: "podium-path-root"
        ) {
            toggleDrawers(to: 1)
        }
    }

    @ViewBuilder
    private func pathSegment(_ node: PathNode, position: Int) -> some View {
        let depth = position + 1
        segmentChip(
            kicker: node.kicker,
            title: node.title,
            shade: node.shade,
            isOpen: openDepth == depth + 1 && depth < 3,
            leadingPad: segmentTip + Spacing.md,
            fader: node.fader,
            accessibilityId: "podium-path-\(node.kicker.lowercased())"
        ) {
            // A segment opens the drawer for choosing among its own siblings:
            // the wave segment drops the waves column, the project segment the
            // projects column. The task segment re-opens the full cascade.
            toggleDrawers(to: min(depth, 3))
        }
    }

    private func segmentChip(
        kicker: String?,
        title: String,
        shade: Color,
        isOpen: Bool,
        leadingPad: CGFloat,
        fader: FaderModel?,
        accessibilityId: String,
        onOpen: @escaping () -> Void
    ) -> some View {
        HStack(spacing: Spacing.sm) {
            Button(action: onOpen) {
                HStack(spacing: Spacing.xs) {
                    VStack(alignment: .leading, spacing: 0) {
                        if let kicker {
                            Text(kicker)
                                .font(Typography.caption(7.5).weight(.bold))
                                .tracking(1.3)
                                .foregroundStyle(.white.opacity(0.6))
                        }
                        Text(title)
                            .font(Typography.sectionTitle(15))
                            .foregroundStyle(.white)
                            .lineLimit(1)
                    }
                    Image(systemName: "chevron.down")
                        .font(.system(size: 8, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.55))
                        .rotationEffect(.degrees(isOpen ? 180 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier(accessibilityId)

            if let fader {
                faderSwitch(fader, width: 14, height: 32)
            }
        }
        .padding(.leading, leadingPad)
        .padding(.trailing, segmentTip + Spacing.md)
        .padding(.vertical, Spacing.xs)
        .frame(minHeight: 50)
        .background(shade)
        .clipShape(PathSegmentShape(tip: segmentTip))
    }

    private func errorBanner(_ message: String) -> some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusWarning)
            Text(message)
                .font(Typography.caption(10))
                .textSelection(.enabled)
                .lineLimit(2)
            Spacer()
            Button("Dismiss") { controlError = nil }
                .buttonStyle(.plain)
                .font(Typography.caption(10).weight(.semibold))
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.xs)
        .background(Color.statusWarning.opacity(0.12))
        .accessibilityIdentifier("podium-console-control-error")
    }

    // MARK: - Drawer columns

    private var drawerColumns: some View {
        HStack(alignment: .top, spacing: 0) {
            waveColumn
                .frame(width: 210)
                .background(ConsoleShade.wave)
            if openDepth >= 2, let entry = exploredWave {
                projectColumn(entry)
                    .frame(width: 210)
                    .background(ConsoleShade.project)
                    .transition(.move(edge: .leading).combined(with: .opacity))
            }
            if openDepth >= 3, let entry = exploredWave, let project = exploredProject(in: entry) {
                taskColumn(project, wave: entry)
                    .frame(width: 250)
                    .background(ConsoleShade.task)
                    .transition(.move(edge: .leading).combined(with: .opacity))
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .shadow(color: Color.black.opacity(0.45), radius: 14, x: 8, y: 4)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-drawer-stack")
    }

    private var waveColumn: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 1) {
                columnLip("WAVES · \(visibleWaves.count)")
                ForEach(waveEntries) { entry in
                    consoleRow(
                        title: entry.vm.displayName,
                        subtitle: nil,
                        titleFont: Typography.sectionTitle(15),
                        isCurrent: exploredWaveId == entry.vm.id && openDepth >= 2,
                        currentShade: ConsoleShade.project,
                        fader: waveFader(entry),
                        accessibilityId: "podium-console-wave-\(entry.vm.id)",
                        accessibilityLabel: "Wave: \(entry.vm.displayName)"
                    ) {
                        exploredWaveId = entry.vm.id
                        exploredProjectId = nil
                        openDepth = 2
                        model.select(.wave(id: entry.vm.id))
                    }
                }
            }
            .padding(.horizontal, Spacing.xs)
            .padding(.bottom, Spacing.sm)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-drawer-waves")
    }

    private func projectColumn(_ entry: WaveEntry) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 1) {
                columnLip("\(entry.vm.displayName.uppercased()) · \(projects(of: entry).count) PROJECTS")
                if projects(of: entry).isEmpty {
                    Text(projectsEmptyReason(entry))
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.6))
                        .padding(Spacing.md)
                        .accessibilityIdentifier("podium-drawer-projects-empty")
                } else {
                    ForEach(projects(of: entry)) { project in
                        consoleRow(
                            title: project.project.name,
                            subtitle: nil,
                            titleFont: Typography.body(12.5).weight(.medium),
                            isCurrent: exploredProjectId == project.id && openDepth >= 3,
                            currentShade: ConsoleShade.task,
                            fader: projectFader(project, wave: entry),
                            accessibilityId: "podium-console-project-\(project.id)",
                            accessibilityLabel: "Project: \(project.project.name)"
                        ) {
                            exploredProjectId = project.id
                            openDepth = 3
                            model.select(.project(id: project.id))
                        }
                    }
                }
            }
            .padding(.horizontal, Spacing.xs)
            .padding(.bottom, Spacing.sm)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-drawer-projects")
    }

    private func taskColumn(_ project: RoadmapProject, wave entry: WaveEntry) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 1) {
                columnLip("\(project.project.name.uppercased()) · \(project.tasks.count) TASKS")
                if project.tasks.isEmpty {
                    Text(project.nextMove.reason)
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.6))
                        .padding(Spacing.md)
                } else {
                    ForEach(project.tasks) { task in
                        consoleRow(
                            title: task.task.name,
                            subtitle: "\(task.task.identifier) · \(task.condition.reason)",
                            titleFont: Typography.body(12),
                            isCurrent: model.selection == .task(id: task.id),
                            currentShade: Color.white.opacity(0.14),
                            fader: taskFader(task, wave: entry, project: project),
                            accessibilityId: "podium-console-task-\(task.id)",
                            accessibilityLabel: "Task: \(task.task.name)"
                        ) {
                            model.select(.task(id: task.id))
                            closeDrawers()
                        }
                    }
                }
            }
            .padding(.horizontal, Spacing.xs)
            .padding(.bottom, Spacing.sm)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-drawer-tasks")
    }

    private func columnLip(_ text: String) -> some View {
        Text(text)
            .font(Typography.caption(8.5).weight(.bold))
            .tracking(1.4)
            .foregroundStyle(.white.opacity(0.6))
            .lineLimit(1)
            .padding(.horizontal, Spacing.md)
            .padding(.top, Spacing.md)
            .padding(.bottom, Spacing.xs)
    }

    private func consoleRow(
        title: String,
        subtitle: String?,
        titleFont: Font,
        isCurrent: Bool,
        currentShade: Color,
        fader: FaderModel,
        accessibilityId: String,
        accessibilityLabel: String,
        onSelect: @escaping () -> Void
    ) -> some View {
        HStack(alignment: .center, spacing: Spacing.sm) {
            Button(action: onSelect) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(title)
                        .font(titleFont)
                        .foregroundStyle(.white)
                        .lineLimit(subtitle == nil ? 1 : 2)
                    if let subtitle {
                        Text(subtitle)
                            .font(Typography.caption(9.5))
                            .foregroundStyle(.white.opacity(0.62))
                            .lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(accessibilityLabel)
            .accessibilityValue(fader.phase.label)
            .accessibilityIdentifier(accessibilityId)
            .accessibilityAddTraits(isCurrent ? [.isSelected] : [])

            faderSwitch(fader, width: 14, height: 30)
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .fill(isCurrent ? currentShade : Color.clear)
        )
    }

    // MARK: - Fader wiring

    /// Everything a rendered fader needs, with the press already bound to the
    /// node's one legal lifecycle move (nil verbs render display-only).
    private struct FaderModel {
        let phase: FaderPhase
        let verb: String?
        let controlId: String
        let accessibilityId: String
        let accessibilityLabel: String
        let action: (() -> Void)?
    }

    private func faderSwitch(_ model: FaderModel, width: CGFloat, height: CGFloat) -> some View {
        FaderSwitch(
            phase: model.phase,
            width: width,
            height: height,
            verb: model.action == nil ? nil : model.verb,
            isBusy: activeControlId == model.controlId,
            accessibilityId: model.accessibilityId,
            accessibilityLabel: model.accessibilityLabel,
            action: model.action
        )
        .disabled(activeControlId != nil && activeControlId != model.controlId)
    }

    private func waveFader(_ entry: WaveEntry) -> FaderModel {
        let action: (() -> Void)?
        switch entry.phase {
        case .off:
            action = { performWave(entry, up: true) }
        case .starting, .producing:
            action = { performWave(entry, up: false) }
        case .waiting:
            action = { model.select(.wave(id: entry.vm.id)) }
        }
        return FaderModel(
            phase: entry.phase,
            verb: entry.phase.verb,
            controlId: "wave:\(entry.vm.id)",
            accessibilityId: "podium-fader-wave-\(entry.vm.id)",
            accessibilityLabel: "\(entry.vm.displayName) switch",
            action: action
        )
    }

    private func projectFader(_ project: RoadmapProject, wave entry: WaveEntry) -> FaderModel {
        let phase = ConsoleSignal.phase(
            humanStop: project.tasks.contains { $0.condition.state == .blocked },
            agentRunning: false,
            signal: signal([])
        )
        // No exact process ownership exists below the Wave level.
        return FaderModel(
            phase: phase,
            verb: nil,
            controlId: "project:\(project.id)",
            accessibilityId: "podium-fader-project-\(project.id)",
            accessibilityLabel: "\(project.project.name) status",
            action: nil
        )
    }

    private func taskFader(
        _ task: RoadmapTask,
        wave entry: WaveEntry,
        project: RoadmapProject
    ) -> FaderModel {
        let phase = ConsoleSignal.phase(
            humanStop: task.condition.state == .blocked,
            agentRunning: false,
            signal: signal([])
        )
        let action: (() -> Void)?
        switch phase {
        case .off:
            if let start = ConsoleSignal.taskStart(task) {
                action = { performTask(task, wave: entry, start: start) }
            } else {
                action = nil
            }
        case .starting, .producing:
            action = nil
        case .waiting:
            action = {
                model.select(.task(id: task.id))
                closeDrawers()
            }
        }
        return FaderModel(
            phase: phase,
            verb: phase.verb,
            controlId: "task:\(task.id)",
            accessibilityId: "podium-fader-task-\(task.id)",
            accessibilityLabel: "\(task.task.identifier) switch",
            action: action
        )
    }

    private func performWave(_ entry: WaveEntry, up: Bool) {
        let controlId = "wave:\(entry.vm.id)"
        let repo = entry.vm.api.repo
        let name = entry.vm.api.name
        run(controlId: controlId) {
            if up {
                _ = try await RegistryQueryLocal.shared.start(wave: name, cwd: repo)
            } else {
                try await Task.detached(priority: .userInitiated) {
                    try LocalWaveAgentLauncher.stopWave(repoPath: repo, waveName: name)
                }.value
            }
        }
    }

    private func performTask(
        _ task: RoadmapTask,
        wave entry: WaveEntry,
        start: ConsoleSignal.TaskStart?
    ) {
        let controlId = "task:\(task.id)"
        let repo = entry.roadmap?.wave.repo ?? entry.vm.api.repo
        let issue = task.task.identifier
        run(controlId: controlId) {
            try await Task.detached(priority: .userInitiated) {
                switch start {
                case .run:
                    try LocalWaveAgentLauncher.runTask(repoPath: repo, issue: issue)
                case .resume:
                    try LocalWaveAgentLauncher.resumeTask(repoPath: repo, issue: issue)
                case nil:
                    try LocalWaveAgentLauncher.interruptTask(repoPath: repo, issue: issue)
                }
            }.value
        }
    }

    private func run(controlId: String, _ command: @escaping () async throws -> Void) {
        activeControlId = controlId
        controlError = nil
        Task {
            do {
                try await command()
                await model.refresh()
            } catch {
                controlError = error.localizedDescription
            }
            if activeControlId == controlId {
                activeControlId = nil
            }
        }
    }

    // MARK: - Signals

    private struct WaveEntry: Identifiable {
        let vm: WaveViewModel
        let roadmap: WaveRoadmap?
        let phase: FaderPhase
        var id: String { vm.id }
    }

    private var visibleWaves: [WaveViewModel] { model.visibleWaves }

    private var waveEntries: [WaveEntry] {
        visibleWaves.map { vm in
            let roadmap = roadmap(for: vm)
            let nodes = providerNodes(wave: vm)
            let hasRedTask = (roadmap?.projects.items ?? [])
                .flatMap(\.tasks)
                .contains { $0.condition.state == .blocked }
            return WaveEntry(
                vm: vm,
                roadmap: roadmap,
                // A Wave's agent is its Home resident: a serving listener
                // (live) counts as up even between runs, so the fader answers
                // the press that just started it.
                phase: ConsoleSignal.phase(
                    humanStop: hasRedTask,
                    agentRunning: vm.isRegistered && vm.api.live,
                    signal: signal(nodes)
                )
            )
        }
    }

    private var exploredWave: WaveEntry? {
        guard let exploredWaveId else { return nil }
        return waveEntries.first { $0.vm.id == exploredWaveId }
    }

    private func exploredProject(in entry: WaveEntry) -> RoadmapProject? {
        guard let exploredProjectId else { return nil }
        return projects(of: entry).first { $0.id == exploredProjectId }
    }

    private func projects(of entry: WaveEntry) -> [RoadmapProject] {
        entry.roadmap?.projects.items ?? []
    }

    /// The empty column tells the truth: an unavailable plan read shows its
    /// reason (e.g. "run `lf pm sync`"), never a false "no Projects".
    private func projectsEmptyReason(_ entry: WaveEntry) -> String {
        switch entry.roadmap?.projects {
        case .unavailable(let reason):
            return reason
        case .available, .none:
            return entry.roadmap == nil
                ? "Wave has no readable plan yet."
                : "No Projects in the plan."
        }
    }

    private func roadmap(for wave: WaveViewModel) -> WaveRoadmap? {
        model.visibleRoadmaps.first { roadmap in
            roadmap.wave.id == wave.id
                || (
                    roadmap.wave.name == wave.api.name
                        && model.repoIdentity(roadmap.wave.repo) == model.repoIdentity(wave.api.repo)
                )
        }
    }

    private func signal(_ nodes: [ActivityNode]) -> PodiumSignalState {
        model.processActivity.value == nil ? .unknown : .from(nodes: nodes)
    }

    private func providerNodes(wave: WaveViewModel) -> [ActivityNode] {
        guard let snapshot = model.processActivity.value else { return [] }
        return snapshot.nodes.filter { node in
            guard node.kind == .providerProcess,
                  node.wave == wave.api.name,
                  node.repo.map(model.repoIdentity) == model.repoIdentity(wave.api.repo) else {
                return false
            }
            return true
        }
    }

    // MARK: - Path derivation

    private struct PathNode: Identifiable {
        let id: String
        let kicker: String
        let title: String
        let shade: Color
        let fader: FaderModel?
    }

    private var pathNodes: [PathNode] {
        guard let selection = model.selection else { return [] }
        var nodes: [PathNode] = []
        var waveEntry: WaveEntry?
        var projectNode: RoadmapProject?
        var taskNode: RoadmapTask?

        switch selection.kind {
        case .wave:
            waveEntry = waveEntries.first { $0.vm.id == selection.id }
        case .project:
            if let found = model.project(id: selection.id) {
                waveEntry = waveEntries.first { $0.roadmap?.wave.id == found.wave.wave.id }
                projectNode = found.project
            }
        case .task:
            if let found = model.task(id: selection.id) {
                waveEntry = waveEntries.first { $0.roadmap?.wave.id == found.wave.wave.id }
                projectNode = found.project
                taskNode = found.task
            }
        }

        if let waveEntry {
            nodes.append(PathNode(
                id: "wave-\(waveEntry.vm.id)",
                kicker: "WAVE",
                title: waveEntry.vm.displayName,
                shade: ConsoleShade.wave,
                fader: waveFader(waveEntry)
            ))
            if let projectNode {
                nodes.append(PathNode(
                    id: "project-\(projectNode.id)",
                    kicker: "PROJECT",
                    title: projectNode.project.name,
                    shade: ConsoleShade.project,
                    fader: projectFader(projectNode, wave: waveEntry)
                ))
                if let taskNode {
                    nodes.append(PathNode(
                        id: "task-\(taskNode.id)",
                        kicker: "TASK",
                        title: "\(taskNode.task.identifier) · \(taskNode.task.name)",
                        shade: ConsoleShade.task,
                        fader: taskFader(taskNode, wave: waveEntry, project: projectNode)
                    ))
                }
            }
        }
        return nodes
    }

    // MARK: - Drawer state

    private func toggleDrawers(to depth: Int) {
        if openDepth == depth {
            closeDrawers()
            return
        }
        seedExplorationFromSelection()
        openDepth = depth
    }

    private func closeDrawers() {
        openDepth = 0
    }

    /// Opening from a path segment starts the cascade at the committed
    /// selection, so the columns come out already showing where you are.
    private func seedExplorationFromSelection() {
        guard let selection = model.selection else { return }
        switch selection.kind {
        case .wave:
            exploredWaveId = selection.id
        case .project:
            if let found = model.project(id: selection.id) {
                exploredWaveId = waveEntries.first { $0.roadmap?.wave.id == found.wave.wave.id }?.vm.id
                exploredProjectId = found.project.id
            }
        case .task:
            if let found = model.task(id: selection.id) {
                exploredWaveId = waveEntries.first { $0.roadmap?.wave.id == found.wave.wave.id }?.vm.id
                exploredProjectId = found.project.id
            }
        }
    }
}

/// The arrow-jointed segment: straight left edge, pointed right edge. Adjacent
/// segments overlap by the tip so the path reads as one continuous pour.
struct PathSegmentShape: Shape {
    var tip: CGFloat = segmentTip

    func path(in rect: CGRect) -> Path {
        Path { path in
            path.move(to: CGPoint(x: rect.minX, y: rect.minY))
            path.addLine(to: CGPoint(x: rect.maxX - tip, y: rect.minY))
            path.addLine(to: CGPoint(x: rect.maxX, y: rect.midY))
            path.addLine(to: CGPoint(x: rect.maxX - tip, y: rect.maxY))
            path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
            path.closeSubpath()
        }
    }
}
