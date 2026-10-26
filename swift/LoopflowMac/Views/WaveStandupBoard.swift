// The screen behind a product team's standup: what moved, what's moving, what's next.

#if os(macOS)
import Loopflow
import SwiftUI

/// A Wave's Tasks bucketed for standup. The one bucketing rule; the board only renders it.
/// Done holds Tasks completed in the last `doneDays` days (0 = all ages); Active the
/// started and waiting Tasks; Backlog the rest of the plan, not started first.
struct WaveStandup: Equatable {
    struct Row: Equatable, Identifiable {
        let task: RoadmapTask
        /// Who owns the Task; nil when nobody does.
        let person: Person?
        /// Detail that follows the name: next move, waiting reason, or "merged".
        let note: String?
        var id: String { task.id }
    }

    struct Person: Equatable, Hashable {
        let key: String
        let name: String
        var initials: String {
            let words = name.split(whereSeparator: { $0 == " " || $0 == "@" || $0 == "." }).prefix(2)
            return words.compactMap(\.first).map { String($0).uppercased() }.joined()
        }
    }

    let done: [Row]
    let active: [Row]
    let backlog: [Row]
    let doneDays: Int
    /// Pictures and names appear once the Wave's Done and Active span two people.
    let showsPeople: Bool

    /// `tasks` fill Active and Backlog; `closed` fills Done. Today both come from the
    /// Wave roadmap; LOO-604 moves closed Tasks to `lf task list --done`, which then
    /// becomes `closed` alone.
    init(tasks: [RoadmapTask], closed: [RoadmapTask], doneDays: Int, now: Date) {
        self.doneDays = doneDays
        let ranked = tasks.sorted { $0.task.rank < $1.task.rank }
        let since = now.addingTimeInterval(-Double(doneDays) * 86_400)
        done = closed.filter { task in
            guard task.task.isSuccessful, !task.condition.unresolvedExecution else { return false }
            if doneDays == 0 { return true }
            guard let date = task.task.completedAt.flatMap(Self.date) else { return false }
            return date <= now && date >= since
        }.sorted { ($0.task.completedAt ?? "") > ($1.task.completedAt ?? "") }
            .map { Row(task: $0, person: Self.person($0), note: nil) }
        active = ranked.filter { task in
            (task.section == .now || task.section == .waiting) && task.task.isCurrent(task.condition)
        }.map { task in
            let note = task.pr?.phase == .merged ? "merged" : task.nextMove.reason
            return Row(task: task, person: Self.person(task), note: note.isEmpty ? nil : note)
        }
        backlog = (ranked.filter { $0.section == .notStarted }
            + ranked.filter { $0.section == .later && !$0.task.isTerminal })
            .map { Row(task: $0, person: nil, note: nil) }
        showsPeople = Set((done + active).compactMap(\.person)).count >= 2
    }

    private static func person(_ task: RoadmapTask) -> Person? {
        guard let owner = task.runtime?.owner, let key = owner.personKey ?? owner.person else { return nil }
        return Person(key: key, name: owner.person ?? key)
    }

    private static func date(_ timestamp: String) -> Date? {
        if let date = try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(timestamp) { return date }
        return try? Date.ISO8601FormatStyle().parse(timestamp)
    }

    /// A Wave's heading lines: name and status, objective, chapter Project and KRs.
    static func heading(_ roadmap: WaveRoadmap) -> [String] {
        var lines = ["\(roadmap.wave.displayName) · \(roadmap.wave.status.label)"]
        if let objective = roadmap.wave.goal.split(separator: "\n").first { lines.append(String(objective)) }
        if let project = roadmap.currentProject {
            lines.append("\(project.name) · \(project.status.rawValue.capitalized)")
            lines += project.krs.map { "\($0.holds ? "✓" : "✗") \($0.text)" }
        }
        return lines
    }

    /// The board as an agent reads it with `lf desktop capture -p board`.
    func text(heading: [String]) -> String {
        var lines = heading
        func column(_ title: String, _ rows: [Row]) {
            lines.append("")
            lines.append("\(title) · \(rows.count)")
            if rows.isEmpty { lines.append("  None") }
            lines += rows.map { row in
                var line = "  \(row.task.task.identifier) \(row.task.task.name)"
                if let person = row.person, showsPeople { line += " · \(person.name)" }
                if let note = row.note { line += " · \(note)" }
                return line
            }
        }
        column(doneDays == 0 ? "Done" : "Done (last \(doneDays) days)", done)
        column("Active", active)
        column("Backlog", backlog)
        return lines.joined(separator: "\n")
    }
}

/// The Done window, one sticky number on this Mac for every Wave (0 = all ages).
enum WaveStandupDefaults {
    static let doneDaysKey = "waveStandupDoneDays"
    static var doneDays: Int {
        UserDefaults.standard.object(forKey: doneDaysKey) as? Int ?? TaskHistoryFilter().days
    }
}

/// Large enough to read across a room. A row opens its Task through the Wave, as a
/// tile does, so Backlog is where unstarted Tasks are seen and started. The
/// repository page is the same board over every repository Task, without a chapter.
struct WaveStandupBoard: View {
    let model: WorkModel
    /// `.wave(id)` or `.repository`.
    let view: WorkNavigation.Board
    /// Rows a column shows before `+N more`; a 1440×900 window fits six without scrolling.
    static let visibleRows = 6
    @AppStorage(WaveStandupDefaults.doneDaysKey) private var doneDays = TaskHistoryFilter().days
    @Environment(\.palette) private var palette
    @State private var showRealignment = false

    private var roadmap: WaveRoadmap? {
        if case .wave(let id) = view { model.wave(id: id) } else { nil }
    }

    var body: some View {
        let scope = view == .repository ? "Repository" : "Wave"
        VStack(alignment: .leading, spacing: Spacing.lg) {
            if let roadmap {
                header(roadmap)
                chapter(roadmap)
                ForEach(roadmap.unavailableTasks, id: \.taskId) { WavePlanningNotice(model: model, task: $0) }
            } else if view == .repository {
                Text(model.repositoryName).font(Typography.strong(24))
                    .accessibilityIdentifier("wave-title")
            }
            if let error = model.roadmap.errorMessage {
                Text("Refresh failed · \(error)").foregroundStyle(palette.warning)
                    .accessibilityIdentifier("wave-refresh-failed")
            }
            switch model.standupTasks(view) {
            case .unavailable(let reason):
                Text("\(scope) planning unavailable · \(reason)").font(Typography.body(15))
                    .foregroundStyle(palette.warning)
            case .available(let tasks, let truncated):
                columns(WaveStandup(tasks: tasks, closed: tasks, doneDays: doneDays, now: .now))
                if truncated { Text("Planning is partial; more Tasks exist.").foregroundStyle(palette.warning) }
            case nil:
                Text("\(scope) planning unavailable").font(Typography.body(15)).foregroundStyle(palette.warning)
            }
        }
        .padding(Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .accessibilityIdentifier("wave-standup-board")
        .sheet(isPresented: $showRealignment) {
            if let roadmap { ProjectRealignmentView(repo: roadmap.wave.repo) }
        }
    }

    private func header(_ roadmap: WaveRoadmap) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
            Text(roadmap.wave.displayName).font(Typography.strong(24))
                .accessibilityIdentifier("wave-title")
            Text(roadmap.wave.status.label.capitalized).font(Typography.body(15))
                .foregroundStyle(palette.textSecondary).accessibilityIdentifier("wave-status")
            Text(roadmap.wave.goal.split(separator: "\n").first.map(String.init) ?? "No objective recorded.")
                .font(Typography.body(15)).foregroundStyle(palette.textSecondary).lineLimit(1)
                .help(roadmap.wave.goal)
                .accessibilityIdentifier("wave-objective")
            Spacer(minLength: 0)
            Menu {
                Button("Project history") {
                    model.historyReference = nil
                    model.historyWave = roadmap.wave
                }.accessibilityIdentifier("wave-project-history")
                Button("Realign Projects…") { showRealignment = true }
                    .accessibilityIdentifier("wave-realign-projects")
                if let workflow = roadmap.workflow {
                    Button("Workflow · \(workflow)") {
                        Task {
                            await model.loadWorkflowCatalog()
                            model.navigation.palette = .workflow(workflow)
                        }
                    }.accessibilityIdentifier("wave-workflow")
                }
            } label: { Image(systemName: "ellipsis.circle") }
                .menuStyle(.borderlessButton).fixedSize()
                .accessibilityIdentifier("wave-board-menu")
        }
    }

    @ViewBuilder
    private func chapter(_ roadmap: WaveRoadmap) -> some View {
        if let error = roadmap.projects.unavailableReason {
            Text("Project unavailable · \(error)").foregroundStyle(palette.warning)
        } else if let project = roadmap.currentProject {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                HStack(spacing: Spacing.md) {
                    Text("\(project.name) · \(project.status.rawValue.capitalized)")
                        .font(Typography.strong(15)).accessibilityIdentifier("wave-current-project")
                    Text(WaveMetricPortfolioPresentation(portfolio: roadmap.metricPortfolio).headline)
                        .font(Typography.body(13)).foregroundStyle(palette.textSecondary).lineLimit(1)
                        .accessibilityIdentifier("wave-metric-headline")
                    PlanningSyncView(sync: project.sync)
                }
                ForEach(project.krs) { kr in
                    Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "xmark.circle")
                        .font(Typography.body(15)).lineLimit(1)
                        .foregroundStyle(kr.holds ? palette.success : palette.textSecondary)
                }
            }
        } else {
            Text("No current chapter Project").font(Typography.body(15)).foregroundStyle(palette.textSecondary)
                .accessibilityIdentifier("wave-no-chapter")
        }
    }

    private func columns(_ standup: WaveStandup) -> some View {
        HStack(alignment: .top, spacing: Spacing.xl) {
            column(standup.done, people: standup.showsPeople, id: "done") { WaveDoneHeading(count: standup.done.count, days: $doneDays) }
            column(standup.active, people: standup.showsPeople, id: "active") { heading("Active", standup.active.count) }
            column(standup.backlog, people: false, id: "backlog") { heading("Backlog", standup.backlog.count) }
        }
    }

    private func open(_ row: WaveStandup.Row) {
        withAnimation(FocusSizing.animation) { model.enter(.task(id: row.id), from: view) }
    }

    private func heading(_ title: String, _ count: Int) -> some View {
        Text("\(title) · \(count)").font(Typography.strong(20))
    }

    private func column<Heading: View>(_ rows: [WaveStandup.Row], people: Bool, id: String,
                                       @ViewBuilder heading: () -> Heading) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            heading()
            if rows.isEmpty {
                Text("None").font(Typography.body(15)).foregroundStyle(palette.textTertiary)
            }
            ForEach(rows.prefix(Self.visibleRows)) { row in
                Button { open(row) } label: { WaveStandupRow(row: row, people: people) }
                    .buttonStyle(.plain)
            }
            if rows.count > Self.visibleRows {
                Text("+\(rows.count - Self.visibleRows) more").font(Typography.body(13))
                    .foregroundStyle(palette.textTertiary)
                    .accessibilityIdentifier("wave-\(id)-more")
            }
        }
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .accessibilityIdentifier("wave-column-\(id)")
    }
}

/// "Done" reads plainly; hovering discloses its range, and clicking the range edits it.
struct WaveDoneHeading: View {
    let count: Int
    @Binding var days: Int
    @Environment(\.palette) private var palette
    @State private var hovered = false
    @State private var editing = false
    @State private var text = ""
    @State private var message: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text("Done · \(count)").font(Typography.strong(20))
                if editing {
                    TextField("days", text: $text).textFieldStyle(.plain).font(Typography.code(15))
                        .frame(width: 48).focused($focused)
                        .onSubmit(commit).onExitCommand { editing = false; message = nil }
                        .accessibilityIdentifier("wave-done-days-field")
                } else if hovered || message != nil {
                    Button(days == 0 ? "· all ages" : "· last \(days) days") {
                        text = String(days)
                        editing = true
                        focused = true
                    }
                    .buttonStyle(.plain).font(Typography.body(13)).foregroundStyle(palette.actionInk)
                    .help("Click to edit; 0 includes all ages.")
                    .accessibilityIdentifier("wave-done-days")
                }
            }
            if let message {
                Text(message).font(Typography.caption()).foregroundStyle(palette.warning)
            }
        }
        .onHover { hovered = $0 }
        .onChange(of: focused) { _, focused in if !focused, editing { commit() } }
    }

    /// The same validation as Task history ranges: a bad entry keeps the previous range.
    private func commit() {
        var filter = TaskHistoryFilter()
        filter.editDays(text)
        if filter.validationMessage == nil { days = filter.days }
        message = filter.validationMessage
        editing = false
    }
}

/// One quiet line names a Task outside the current plan; Details discloses reason and recovery.
struct WavePlanningNotice: View {
    let model: WorkModel
    let task: UnavailableTaskEvidence
    @Environment(\.palette) private var palette

    var body: some View {
        let open = model.navigation.expandedNotices.contains(task.taskId)
        return VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Image(systemName: "exclamationmark.circle")
                    .foregroundStyle(palette.warning)
                Text("\(task.taskIdentifier) is outside the current plan.")
                    .foregroundStyle(WorkTone.warning.text(in: palette))
                Spacer(minLength: Spacing.sm)
                Button(open ? "Hide details" : "Details") {
                    if open { model.navigation.expandedNotices.remove(task.taskId) } else { model.navigation.expandedNotices.insert(task.taskId) }
                }
                .buttonStyle(WorkPlainButtonStyle())
                .foregroundStyle(palette.actionInk)
                .accessibilityIdentifier("wave-notice-details-\(task.taskId)")
            }
            if open {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("\(task.taskIdentifier): \(task.reason)").textSelection(.enabled)
                    Text("Recover with").font(Typography.caption(11)).foregroundStyle(palette.textTertiary)
                    Text(task.recovery)
                        .font(Typography.code(11.5))
                        .textSelection(.enabled)
                        .padding(Spacing.sm)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border))
                }
                .foregroundStyle(palette.text)
                .padding(.leading, 22)
            }
        }
        .font(Typography.body(12.5))
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(WorkTone.warning.fill(in: palette).opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("wave-notice-\(task.taskId)")
    }
}

struct WaveStandupRow: View {
    let row: WaveStandup.Row
    let people: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            if people, let person = row.person {
                Text(person.initials).font(Typography.strong(10)).foregroundStyle(palette.text)
                    .frame(width: 20, height: 20)
                    .background(palette.surfaceMuted, in: Circle())
                    .help(person.name)
                    .accessibilityIdentifier("wave-person-\(row.id)")
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(row.task.task.identifier).font(Typography.code(13)).foregroundStyle(palette.textSecondary)
                        .fixedSize()
                    Text(row.task.task.name).font(Typography.body(15)).lineLimit(1)
                }
                if let note = row.note {
                    Text(people ? "\(row.person?.name ?? "Unowned") · \(note)" : note)
                        .font(Typography.body(13)).foregroundStyle(palette.textSecondary).lineLimit(1)
                }
            }
        }
        .help(row.task.task.name)
        .accessibilityIdentifier("wave-row-\(row.id)")
    }
}
#endif
