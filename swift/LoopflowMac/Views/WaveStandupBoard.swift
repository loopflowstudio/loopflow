// The screen behind a product team's standup: what moved, what's moving, what's next.

#if os(macOS)
import Loopflow
import SwiftUI

/// A Wave's Tasks in the columns of Rust's `TaskSection`; the board only renders
/// them. Done shows the Tasks completed in the last `doneDays` days (0 = all ages).
struct WaveStandup: Equatable {
    struct Row: Equatable, Identifiable {
        let task: RoadmapTask
        /// Who owns the Task; nil when nobody does.
        let person: Person?
        /// Detail that follows the name: next move, waiting reason, or "merged".
        let note: String?
        /// A Working or Autonomous Task's Session: Available or Busy.
        let session: SessionAttention?
        var id: String { task.id }
    }

    enum SessionAttention: String, Equatable {
        case available = "Available", busy = "Busy"
    }

    /// One person per email; an owner recorded by name alone joins the person with
    /// that name. GitHub supplies the picture and name once the email resolves.
    struct Person: Equatable, Hashable {
        let key: String
        let name: String
        let github: GitHubPerson?
        var initials: String {
            let words = name.split(whereSeparator: { $0 == " " || $0 == "@" || $0 == "." }).prefix(2)
            return words.compactMap(\.first).map { String($0).uppercased() }.joined()
        }
    }

    let backlog: [Row]
    let working: [Row]
    let autonomous: [Row]
    let done: [Row]
    let doneDays: Int
    /// Pictures and names appear once the started and finished Tasks span two people.
    let showsPeople: Bool

    /// `tasks` fill Backlog, Working and Autonomous; `closed` fills Done. Today both come from the
    /// Wave roadmap; LOO-604 moves closed Tasks to `lf task list --done`, which then
    /// becomes `closed` alone. `github` maps lowercase owner emails to their users;
    /// `sessions` names each started Task's Session attention.
    init(tasks: [RoadmapTask], closed: [RoadmapTask], doneDays: Int, now: Date, github: [String: GitHubPerson],
         sessions: [String: SessionAttention]) {
        self.doneDays = doneDays
        let ranked = tasks.sorted { $0.task.rank < $1.task.rank }
        let since = now.addingTimeInterval(-Double(doneDays) * 86_400)
        var emails: [String: String] = [:]
        for owner in (tasks + closed).compactMap({ $0.runtime?.owner }) {
            if let name = owner.person, let email = owner.personKey { emails[name] = emails[name] ?? email.lowercased() }
        }
        func person(_ task: RoadmapTask) -> Person? {
            guard let owner = task.runtime?.owner,
                  let key = owner.personKey?.lowercased() ?? owner.person.map({ emails[$0] ?? $0 }) else { return nil }
            let user = github[key]
            return Person(key: key, name: user?.name ?? owner.person ?? user?.login ?? key, github: user)
        }
        done = closed.filter { task in
            guard task.section == .done else { return false }
            if doneDays == 0 { return true }
            guard let date = task.task.completedAt.flatMap(Self.date) else { return false }
            return date <= now && date >= since
        }.sorted { ($0.task.completedAt ?? "") > ($1.task.completedAt ?? "") }
            .map { Row(task: $0, person: person($0), note: nil, session: nil) }
        func started(_ section: TaskSection) -> [Row] {
            ranked.filter { $0.section == section }.map { task in
                let note = task.pr?.phase == .merged ? "merged" : task.nextMove.reason
                return Row(task: task, person: person(task), note: note.isEmpty ? nil : note,
                           session: sessions[task.id])
            }
        }
        working = started(.working)
        autonomous = started(.autonomous)
        backlog = ranked.filter { $0.section == .backlog }
            .map { Row(task: $0, person: nil, note: nil, session: nil) }
        showsPeople = Set((working + autonomous + done).compactMap { $0.person?.key }).count >= 2
    }

    func rows(_ section: WaveBoardSection) -> [Row] {
        switch section {
        case .backlog: backlog
        case .working: working
        case .autonomous: autonomous
        case .done: done
        }
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

    /// The board as an agent reads it with `lf desktop capture -p board`: every
    /// section's count, the shown one bracketed, then its rows.
    func text(heading: [String], section: WaveBoardSection) -> String {
        var lines = heading
        lines.append("")
        lines.append(WaveBoardSection.allCases.map { item in
            let label = "\(item.title) \(rows(item).count)"
            return item == section ? "[\(label)]" : label
        }.joined(separator: " · "))
        if section == .done, doneDays > 0 { lines.append("  last \(doneDays) days") }
        let shown = rows(section)
        if shown.isEmpty { lines.append("  None") }
        lines += shown.map { row in
            var line = "  \(row.task.task.identifier) \(row.task.task.name)"
            if let person = row.person, showsPeople { line += " · \(person.name)" }
            if let session = row.session { line += " · \(session.rawValue)" }
            if let note = row.note { line += " · \(note)" }
            return line
        }
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

/// Large enough to read across a room, one section at a time, Backlog first. A row
/// opens its Task through the Wave, as a tile does, so Backlog is where unstarted
/// Tasks are seen and started. The repository page is the same board over every
/// repository Task, without a chapter.
struct WaveStandupBoard: View {
    let model: WorkModel
    /// `.wave(id)` or `.repository`.
    let view: WorkNavigation.Board
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
                HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                    Text(model.repositoryName).font(Typography.strong(24))
                        .accessibilityIdentifier("wave-title")
                    Spacer(minLength: 0)
                    ShowHiddenMenu(model: model, view: view)
                }
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
                section(model.standup(tasks, doneDays: doneDays))
                if truncated { Text("Planning is partial; more Tasks exist.").foregroundStyle(palette.warning) }
            case nil:
                Text("\(scope) planning unavailable").font(Typography.body(15)).foregroundStyle(palette.warning)
            }
        }
        .padding(Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        // A fill that ignores safe areas escapes the part's clip into the titlebar
        // and keeps a stale size there; this one is the board's frame exactly.
        .background(palette.background, ignoresSafeAreaEdges: [])
        .accessibilityIdentifier("wave-standup-board")
        .task(id: model.repoPath) { await model.loadGitHubPeople() }
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
            ShowHiddenMenu(model: model, view: view)
            Menu {
                Button("Project history") {
                    model.historyReference = nil
                    model.historyWave = roadmap.wave
                }.accessibilityIdentifier("wave-project-history")
                Button("Realign Projects…") { showRealignment = true }
                    .accessibilityIdentifier("wave-realign-projects")
                if let workflow = roadmap.graph {
                    Button("Workflow · \(workflow)") {
                        Task {
                            await model.loadGraphCatalog()
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

    /// The shown section's selector, its Done range when it is Done, and its rows.
    private func section(_ standup: WaveStandup) -> some View {
        let shown = model.boardSection(view)
        return VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .center, spacing: Spacing.lg) {
                Picker("Section", selection: Binding(get: { shown }, set: { model.navigation.boardSections[view] = $0 })) {
                    ForEach(WaveBoardSection.allCases, id: \.self) { item in
                        Text("\(item.title) · \(standup.rows(item).count)").tag(item)
                    }
                }
                .pickerStyle(.segmented).labelsHidden().fixedSize()
                .font(Typography.strong(15))
                .accessibilityIdentifier("wave-section-picker")
                if shown == .done { WaveDoneRange(days: $doneDays) }
            }
            WaveBoardList(rows: standup.rows(shown), people: shown != .backlog && standup.showsPeople) { row in
                withAnimation(FocusSizing.animation) { model.enter(.task(id: row.id), from: view) }
            }
            .id(shown)
            .accessibilityIdentifier("wave-section-\(shown.rawValue)")
        }
    }
}

/// A board section's rows. It scrolls, and reaching its end brings the next page.
struct WaveBoardList: View {
    static let pageSize = 20
    let rows: [WaveStandup.Row]
    let people: Bool
    let open: (WaveStandup.Row) -> Void
    @Environment(\.palette) private var palette
    @State private var limit = pageSize

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.sm) {
                if rows.isEmpty {
                    Text("None").font(Typography.body(15)).foregroundStyle(palette.textTertiary)
                }
                ForEach(rows.prefix(limit)) { row in
                    Button { open(row) } label: { WaveStandupRow(row: row, people: people) }
                        .buttonStyle(.plain)
                }
                if rows.count > limit {
                    ProgressView().controlSize(.small)
                        .onAppear { limit += Self.pageSize }
                        .accessibilityIdentifier("wave-next-page")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// A Wave view's header row while its board is hidden: the view's name and its
/// `Show…` menu, which the board header carries otherwise.
struct WaveViewHeader: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            Text(title).font(Typography.strong(15)).lineLimit(1)
            Spacer(minLength: 0)
            ShowHiddenMenu(model: model, view: view)
        }
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.sm)
        .background(palette.background, ignoresSafeAreaEdges: [])
        .accessibilityIdentifier("wave-view-header")
    }

    private var title: String {
        if case .wave(let id) = view { model.wave(id: id)?.wave.displayName ?? id } else { model.repositoryName }
    }
}

/// The Done section's range; clicking it edits the number of days in place.
struct WaveDoneRange: View {
    @Binding var days: Int
    @Environment(\.palette) private var palette
    @State private var editing = false
    @State private var text = ""
    @State private var message: String?
    @FocusState private var focused: Bool

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            if editing {
                TextField("days", text: $text).textFieldStyle(.plain).font(Typography.code(15))
                    .frame(width: 48).focused($focused)
                    .onSubmit(commit).onExitCommand { editing = false; message = nil }
                    .accessibilityIdentifier("wave-done-days-field")
            } else {
                Button(days == 0 ? "All ages" : "Last \(days) days") {
                    text = String(days)
                    editing = true
                    focused = true
                }
                .buttonStyle(.plain).font(Typography.body(13)).foregroundStyle(palette.actionInk)
                .help("Click to edit; 0 includes all ages.")
                .accessibilityIdentifier("wave-done-days")
            }
            if let message {
                Text(message).font(Typography.caption()).foregroundStyle(palette.warning)
            }
        }
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
                WavePersonPicture(person: person)
                    .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 4 }
                    .accessibilityIdentifier("wave-person-\(row.id)")
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(row.task.task.identifier).font(Typography.code(13)).foregroundStyle(palette.textSecondary)
                        .fixedSize()
                    Text(row.task.task.name).font(Typography.body(15)).lineLimit(1)
                    if let session = row.session {
                        Text(session.rawValue).font(Typography.caption(11))
                            .foregroundStyle(session == .available ? palette.available : palette.textSecondary)
                            .accessibilityIdentifier("wave-session-\(row.id)")
                    }
                }
                if let detail {
                    Text(detail).font(Typography.body(13)).foregroundStyle(palette.textSecondary).lineLimit(1)
                }
            }
        }
        .help(row.task.task.name)
        .accessibilityIdentifier("wave-row-\(row.id)")
    }

    /// The owner's name when the board shows people, then the row's note.
    private var detail: String? {
        let name = people ? row.person?.name ?? "Unowned" : nil
        let parts = [name, row.note].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }
}

/// A person's GitHub picture, or their initials until GitHub names them.
struct WavePersonPicture: View {
    let person: WaveStandup.Person
    @Environment(\.palette) private var palette

    var body: some View {
        Group {
            if let url = person.github?.avatarUrl {
                AsyncImage(url: url) { image in image.resizable().scaledToFill() } placeholder: { initials }
            } else {
                initials
            }
        }
        .frame(width: 20, height: 20)
        .clipShape(Circle())
        .help(person.github.map { "\(person.name) (@\($0.login))" } ?? person.name)
    }

    private var initials: some View {
        Text(person.initials).font(Typography.strong(10)).foregroundStyle(palette.text)
            .frame(width: 20, height: 20)
            .background(palette.surfaceMuted, in: Circle())
    }
}
#endif
