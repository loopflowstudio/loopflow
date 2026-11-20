// A Wave's backlog: its unstarted Tasks as one ranked list beside its conversation.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

// MARK: - Projection

/// Which backlog Tasks the list shows: all of them, the ones you claimed, or the
/// ones nobody else has claimed.
enum BacklogFilter: String, CaseIterable, Hashable {
    case all = "All", mine = "Mine", available = "Available"
}

/// Who holds a backlog Task, as Rust decides it; the row draws it as a mark.
enum BacklogOwner: Equatable {
    case you, someone, nobody

    init(_ task: RoadmapTask) {
        if task.ownedByYou { self = .you } else if task.runtime?.owner != nil { self = .someone } else { self = .nobody }
    }

    /// The mark `lf desktop capture -p board` prints at a row's end.
    var mark: String {
        switch self {
        case .you: "●"
        case .someone: "○"
        case .nobody: "◌"
        }
    }
}

/// What a person has done to one view's backlog; it outlives entering a Task.
struct BacklogState: Equatable {
    var selection: Set<String> = []
    /// Where ⇧-click and ⇧↑↓ extend from.
    var anchor: String?
    /// The row the keyboard last moved to; arrows step from here.
    var cursor: String?
    var filter: BacklogFilter = .all
    var query = ""
    /// Whether the opened brief shows past its first lines.
    var showsAll = false
    /// The Task whose brief is being edited, and the text typed so far.
    var editing: String?
    var draft = ""
    var showsObjective = false
    /// What the last action did, or why it didn't.
    var notice: String?

    /// Select one row alone, or clear it when it is the only one selected.
    mutating func select(_ id: String) {
        selection = selection == [id] ? [] : [id]
        mark(id)
    }

    mutating func toggle(_ id: String) {
        if selection.contains(id) { selection.remove(id) } else { selection.insert(id) }
        mark(id)
    }

    /// From the anchor through `id`, as ⇧-click and ⇧-arrow do in a Mac list.
    mutating func extend(to id: String, rows: [String]) {
        let start = anchor ?? id
        guard let from = rows.firstIndex(of: start), let to = rows.firstIndex(of: id) else { return }
        selection = Set(rows[min(from, to)...max(from, to)])
        cursor = id
        anchor = start
        settle()
    }

    mutating func step(_ offset: Int, extending: Bool, rows: [String]) {
        guard !rows.isEmpty else { return }
        let from = cursor.flatMap(rows.firstIndex(of:))
        let next = rows[from.map { min(max($0 + offset, 0), rows.count - 1) } ?? (offset > 0 ? 0 : rows.count - 1)]
        if extending { extend(to: next, rows: rows) } else {
            selection = [next]
            mark(next)
        }
    }

    mutating func selectAll(_ rows: [String]) {
        selection = Set(rows)
        settle()
    }

    mutating func clear() {
        selection = []
        settle()
    }

    private mutating func mark(_ id: String) {
        anchor = id
        cursor = id
        settle()
    }

    private mutating func settle() {
        showsAll = false
        if let editing, selection != [editing] { self.editing = nil }
    }
}

/// A Graph as Start offers it, read from the catalog: its description, how many
/// times it stops for you and whether it merges before the first stop.
struct BacklogFlow: Equatable, Identifiable {
    let name: String
    let description: String?
    let stops: Int
    let merges: Bool
    var id: String { name }

    init(name: String, description: String?, stops: Int, merges: Bool) {
        self.name = name; self.description = description; self.stops = stops; self.merges = merges
    }

    init?(_ entry: GraphCatalogEntry) {
        guard let compiled = entry.compiled else { return nil }
        func nodes(_ steps: [GraphNode]) -> [GraphNode] { steps.flatMap { [$0] + $0.paths.flatMap { nodes($0.steps) } } }
        let all = nodes(compiled.steps)
        let firstStop = all.firstIndex(where: \.interactive) ?? all.count
        name = entry.name
        description = entry.description
        stops = all.count(where: \.interactive)
        merges = all[..<firstStop].contains { node in
            node.kind == .op && (node.label.hasPrefix("land") || node.label.hasPrefix("pr land"))
        }
    }

    /// What starting it commits you to, shown at the menu item's end.
    var consequence: String {
        if merges { return "merges without asking" }
        return switch stops {
        case 0: "never stops"
        case 1: "1 stop"
        default: "\(stops) stops"
        }
    }

    /// The description's first sentence, which fits a menu line.
    var summary: String? {
        guard let description else { return nil }
        return description.split(separator: ". ", maxSplits: 1).first.map { String($0).trimmingCharacters(in: CharacterSet(charactersIn: ".")) }
    }
}

/// A view's backlog: its open, unstarted Tasks in rank order, filtered and
/// searched, with the selection the view holds. Built from Rust's reading on
/// every render; it owns no order or ownership of its own.
struct BacklogList {
    /// Every backlog Task, in rank order, before filtering.
    let all: [RoadmapTask]
    /// What the list shows.
    let rows: [RoadmapTask]
    let truncated: Bool
    /// The Wave this backlog belongs to; nil over a whole repository.
    let wave: String?
    /// The Wave's own default flow, for Start's label with nothing selected.
    let defaultFlow: String?
    let waveNames: [String: String]
    let state: BacklogState

    init(tasks: [RoadmapTask], truncated: Bool, wave: String?, defaultFlow: String?, waveNames: [String: String],
         state: BacklogState) {
        all = tasks.filter { $0.section == .backlog }.sorted { $0.task.rank < $1.task.rank }
        self.truncated = truncated
        self.wave = wave
        self.defaultFlow = defaultFlow
        self.waveNames = waveNames
        self.state = state
        let needle = state.query.trimmingCharacters(in: .whitespacesAndNewlines)
        rows = all.filter { task in
            Self.passes(task, state.filter) && (needle.isEmpty
                || "\(task.task.identifier) \(task.task.name) \(task.task.description)".localizedCaseInsensitiveContains(needle))
        }
    }

    static func passes(_ task: RoadmapTask, _ filter: BacklogFilter) -> Bool {
        switch filter {
        case .all: true
        case .mine: BacklogOwner(task) == .you
        case .available: BacklogOwner(task) != .someone
        }
    }

    func count(_ filter: BacklogFilter) -> Int { all.count { Self.passes($0, filter) } }

    /// Rank belongs to each Project, so only a Wave's backlog reorders.
    var canReorder: Bool { wave != nil }

    var ids: [String] { rows.map(\.id) }

    /// The selected Tasks in list order; selected rows a filter hides still count.
    var selected: [RoadmapTask] { all.filter { state.selection.contains($0.id) } }

    /// The one selected Task, whose brief opens under its row.
    var opened: RoadmapTask? {
        guard state.selection.count == 1, let id = state.selection.first else { return nil }
        return rows.first { $0.id == id }
    }

    /// The Tasks an action on `id` applies to: the selection when the row is in it, else the row alone.
    func targets(_ id: String) -> [RoadmapTask] {
        state.selection.contains(id) ? selected : all.filter { $0.id == id }
    }

    /// The Wave a row names when it isn't the one being looked at.
    func waveLabel(_ task: RoadmapTask) -> String? {
        guard let id = task.task.waveId, id != wave else { return nil }
        return waveNames[id]
    }

    // MARK: Start

    /// Each Task's flow: the Graph Rust says its first run takes up.
    static func flow(_ task: RoadmapTask) -> String { task.graphName ?? "code" }

    /// The one flow every Task would run by default, or nil when they differ.
    static func sharedFlow(_ tasks: [RoadmapTask]) -> String? {
        let names = Set(tasks.map(flow))
        return names.count == 1 ? names.first : nil
    }

    /// "feature 2 · code 1": the most common flow first.
    static func breakdown(_ tasks: [RoadmapTask]) -> String {
        let counts: [(name: String, count: Int)] = Dictionary(grouping: tasks, by: flow).map { ($0.key, $0.value.count) }
        let ordered = counts.sorted { left, right in
            left.count != right.count ? left.count > right.count : left.name < right.name
        }
        return ordered.map { "\($0.name) \($0.count)" }.joined(separator: " · ")
    }

    /// What Start says it will do: "Start · feature", or "Start 3 · feature 2 · code 1".
    var startLabel: String {
        let tasks = selected
        let count = tasks.count > 1 ? " \(tasks.count)" : ""
        let flow = tasks.isEmpty ? defaultFlow ?? "code" : Self.sharedFlow(tasks) ?? Self.breakdown(tasks)
        return "Start\(count) · \(flow)"
    }

    // MARK: Order

    /// The rank writes that move `ids`, in list order, to sit just before
    /// `target`, or after the last row with no target. Each write is the
    /// position after removal, as `lf task update --rank` takes it, and each is
    /// computed against the order the writes before it leave. A Task moves only
    /// among Tasks of its own Project.
    func moves(_ ids: [String], before target: String?) -> [(task: RoadmapTask, rank: Int)] {
        let moving = all.filter { ids.contains($0.id) }
        guard !moving.isEmpty, target.map({ !ids.contains($0) }) ?? true else { return [] }
        var order = all.filter { !ids.contains($0.id) }
        let index = target.flatMap { id in order.firstIndex { $0.id == id } } ?? order.count
        order.insert(contentsOf: moving, at: index)
        guard order.map(\.id) != all.map(\.id) else { return [] }
        var ranks = Dictionary(all.map { ($0.id, Int($0.task.rank)) }, uniquingKeysWith: { first, _ in first })
        var writes: [(RoadmapTask, Int)] = []
        // Last first, so each Task's next neighbour is already where it belongs.
        for task in moving.reversed() {
            guard let from = ranks[task.id] else { continue }
            let project = order.filter { $0.task.projectId == task.task.projectId }
            guard let at = project.firstIndex(where: { $0.id == task.id }) else { continue }
            let rank: Int
            if at + 1 < project.count, let next = ranks[project[at + 1].id] {
                rank = next - (from < next ? 1 : 0)
            } else if at > 0, let previous = ranks[project[at - 1].id] {
                rank = previous - (from < previous ? 1 : 0) + 1
            } else { continue }
            guard rank != from else { continue }
            writes.append((task, rank))
            Self.apply(&ranks, moving: task, from: from, to: rank, among: all.filter { $0.task.projectId == task.task.projectId })
        }
        return writes
    }

    /// One row up or down among the shown rows, with the selection it belongs to.
    func nudge(_ ids: [String], by offset: Int) -> [(task: RoadmapTask, rank: Int)] {
        let shown = self.ids
        let places = shown.indices.filter { ids.contains(shown[$0]) }
        guard let first = places.first, let last = places.last else { return [] }
        if offset < 0 {
            return first > 0 ? moves(ids, before: shown[first - 1]) : []
        }
        guard last + 1 < shown.count else { return [] }
        return moves(ids, before: last + 2 < shown.count ? shown[last + 2] : nil)
    }

    /// Before the first row that isn't moving.
    func toTop(_ ids: [String]) -> [(task: RoadmapTask, rank: Int)] {
        moves(ids, before: all.first { !ids.contains($0.id) }?.id)
    }

    /// The writes that put `writes`' Tasks back: their earlier ranks, lowest first.
    static func inverse(_ writes: [(task: RoadmapTask, rank: Int)]) -> [(task: RoadmapTask, rank: Int)] {
        var seen = Set<String>()
        return writes.reversed().filter { seen.insert($0.task.id).inserted }
            .map { ($0.task, Int($0.task.task.rank)) }.sorted { $0.1 < $1.1 }
    }

    private static func apply(_ ranks: inout [String: Int], moving task: RoadmapTask, from: Int, to: Int,
                              among siblings: [RoadmapTask]) {
        for sibling in siblings where sibling.id != task.id {
            guard var rank = ranks[sibling.id] else { continue }
            if rank > from { rank -= 1 }
            if rank >= to { rank += 1 }
            ranks[sibling.id] = rank
        }
        ranks[task.id] = to
    }

    // MARK: Text

    /// A Wave's heading lines: name and status, objective, Project and KRs.
    static func heading(_ roadmap: WaveRoadmap) -> [String] {
        var lines = ["\(roadmap.wave.displayName) · \(roadmap.wave.status.label)"]
        if let objective = roadmap.wave.goal.split(separator: "\n").first { lines.append(String(objective)) }
        if let project = roadmap.currentProject {
            lines.append("\(project.name) · \(project.status.rawValue.capitalized)")
            lines += project.krs.map { "\($0.holds ? "✓" : "✗") \($0.text)" }
        }
        return lines
    }

    /// The backlog as an agent reads it with `lf desktop capture -p board`:
    /// the filter counts, the shown one bracketed, then the rows in order with owners.
    func text(heading: [String]) -> String {
        var lines = heading
        lines.append("")
        lines.append(BacklogFilter.allCases.map { filter in
            let label = "\(filter.rawValue) \(count(filter))"
            // The default shows everything; brackets mark a narrower filter.
            return filter == state.filter && filter != .all ? "[\(label)]" : label
        }.joined(separator: " · "))
        if !state.query.isEmpty { lines.append("  search “\(state.query)”") }
        if rows.isEmpty { lines.append("  \(empty.title)") }
        lines += rows.map { task in
            var line = "  \(task.task.identifier) \(task.task.name)"
            if let wave = waveLabel(task) { line += " · \(wave)" }
            if state.selection.contains(task.id) { line = "›" + line.dropFirst() }
            return line + " \(BacklogOwner(task).mark)"
        }
        if truncated { lines.append("  Partial: searching the \(all.count) loaded") }
        return lines.joined(separator: "\n")
    }

    // MARK: Empty

    /// Why the list is empty, and the one step that fills it.
    enum EmptyStep: Equatable { case clearSearch, showAvailable, showAll, new }

    var empty: (title: String, step: EmptyStep) {
        if !state.query.trimmingCharacters(in: .whitespaces).isEmpty {
            return ("Nothing in \(state.filter.rawValue) matches “\(state.query)”", .clearSearch)
        }
        switch state.filter {
        case .mine: return ("You haven't claimed anything. \(count(.available)) are available.", .showAvailable)
        case .available where !all.isEmpty: return ("Everything here is claimed by someone else", .showAll)
        default: return ("No unstarted work in this \(wave == nil ? "repository" : "Wave")", .new)
        }
    }
}

/// What ⌘Z puts back: a reorder, a claim or a brief edit. Start and Abandon register nothing.
enum BacklogChange: Sendable {
    case ranks([(task: RoadmapTask, rank: Int)])
    case claims([RoadmapTask])
    case brief(task: RoadmapTask, previous: String, written: String)
}

// MARK: - Actions

extension WorkModel {
    private func backlogNotice(_ view: WorkNavigation.Board, _ notice: String?) {
        navigation.backlogs[view, default: BacklogState()].notice = notice
    }

    /// Write each rank in turn; the first refusal stops the rest. Returns the inverse when any landed.
    @discardableResult
    func rankBacklog(_ writes: [(task: RoadmapTask, rank: Int)], in view: WorkNavigation.Board) async -> BacklogChange? {
        guard !writes.isEmpty else { return nil }
        var landed: [(task: RoadmapTask, rank: Int)] = []
        for write in writes {
            do { try await rankTask(write.task, rank: write.rank) } catch {
                backlogNotice(view, "Order not saved · \(write.task.task.identifier): \(error.localizedDescription)")
                break
            }
            landed.append(write)
        }
        if landed.count == writes.count {
            let moved = Set(writes.map(\.task.id)).count
            backlogNotice(view, moved == 1 ? "Moved \(writes[0].task.task.identifier)" : "Moved \(moved) Tasks")
        }
        return landed.isEmpty ? nil : .ranks(BacklogList.inverse(landed))
    }

    // Claim, Start, Move and Abandon show in the click; a refusal posts its own
    // Notice and puts that Task's row back.

    /// Claim each unclaimed Task without starting it; returns the inverse.
    @discardableResult
    func claimBacklog(_ tasks: [RoadmapTask], in view: WorkNavigation.Board) -> BacklogChange? {
        let claimed = tasks.filter { BacklogOwner($0) == .nobody }
        for task in claimed { assignTask(task, claimed: true) }
        let skipped = tasks.count - claimed.count
        backlogNotice(view, "Claimed \(claimed.count)" + (skipped > 0 ? " · \(skipped) already claimed" : ""))
        return claimed.isEmpty ? nil : .claims(claimed)
    }

    /// Start each Task, on `graph` or its own flow.
    func startBacklog(_ tasks: [RoadmapTask], graph: String?, in view: WorkNavigation.Board) {
        for task in tasks { startTask(graph, task: task) }
        navigation.backlogs[view, default: BacklogState()].selection.subtract(tasks.map(\.id))
        backlogNotice(view, "Started \(tasks.count)")
    }

    func moveBacklog(_ tasks: [RoadmapTask], toWave wave: String?, in view: WorkNavigation.Board) {
        for task in tasks { moveTask(task, toWave: wave) }
        backlogNotice(view, "Moved \(tasks.count)")
    }

    func abandonBacklog(_ tasks: [RoadmapTask], in view: WorkNavigation.Board) {
        for task in tasks { abandonTask(task) }
        navigation.backlogs[view, default: BacklogState()].selection.subtract(tasks.map(\.id))
        backlogNotice(view, "Abandoned \(tasks.count)")
    }

    /// Save the brief being edited; returns the inverse when it landed.
    @discardableResult
    func saveBacklogBrief(_ task: RoadmapTask, text: String, in view: WorkNavigation.Board) async -> BacklogChange? {
        do {
            try await updateTaskDirective(task: task, text: text)
            navigation.backlogs[view, default: BacklogState()].editing = nil
            backlogNotice(view, "Saved \(task.task.identifier)'s brief")
            return .brief(task: task, previous: task.task.description, written: text)
        } catch {
            backlogNotice(view, "Brief not saved · \(error.localizedDescription)")
            return nil
        }
    }

    /// Put a change back. A brief someone changed since stays as it is.
    func revertBacklog(_ change: BacklogChange, in view: WorkNavigation.Board) async {
        switch change {
        case .ranks(let writes):
            await rankBacklog(writes, in: view)
            backlogNotice(view, "Order restored")
        case .claims(let tasks):
            for task in tasks { assignTask(task, claimed: false) }
            backlogNotice(view, tasks.count == 1 ? "Unclaimed \(tasks[0].task.identifier)" : "Unclaimed \(tasks.count) Tasks")
        case .brief(let task, let previous, let written):
            guard let current = self.task(id: task.id)?.task, current.task.description == written else {
                backlogNotice(view, "Brief changed since; not undone")
                return
            }
            do {
                try await updateTaskDirective(task: current, text: previous)
                backlogNotice(view, "Brief restored")
            } catch { backlogNotice(view, "Brief not restored · \(error.localizedDescription)") }
        }
    }
}

// MARK: - Views

/// A Wave view's backlog, the repository page included. A row's brief opens in
/// place; nothing starts until Start.
struct BacklogBoard: View {
    let model: WorkModel
    /// `.wave(id)` or `.repository`.
    let view: WorkNavigation.Board
    @Environment(\.palette) private var palette
    @Environment(\.undoManager) private var undoManager
    @State private var hovered: String?
    @State private var dropTarget: String?
    @State private var abandoning: [RoadmapTask]?
    @FocusState private var focused: Bool

    private var roadmap: WaveRoadmap? {
        if case .wave(let id) = view { model.wave(id: id) } else { nil }
    }

    private var list: BacklogList { model.backlog(view) }

    private var state: Binding<BacklogState> {
        Binding(get: { model.navigation.backlogs[view] ?? BacklogState() },
                set: { model.navigation.backlogs[view] = $0 })
    }

    var body: some View {
        let list = list
        VStack(alignment: .leading, spacing: 0) {
            header(list)
            Divider()
            BacklogToolbar(model: model, view: view, list: list, perform: perform)
            Divider()
            notices
            content(list)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        // A fill that ignores safe areas escapes the part's clip into the titlebar
        // and keeps a stale size there; this one is the board's frame exactly.
        .background(palette.background, ignoresSafeAreaEdges: [])
        .accessibilityIdentifier("wave-backlog")
        .task(id: model.repoPath) { await model.loadGraphCatalog() }
        .confirmationDialog(abandonTitle, isPresented: Binding(get: { abandoning != nil }, set: { if !$0 { abandoning = nil } })) {
            Button("Abandon", role: .destructive) {
                if let tasks = abandoning { model.abandonBacklog(tasks, in: view) }
                abandoning = nil
            }
        } message: {
            Text("Each keeps its ID and history; its PRs and branches are removed.")
        }
    }

    private var abandonTitle: String {
        guard let tasks = abandoning else { return "" }
        return tasks.count == 1 ? "Abandon \(tasks[0].task.identifier)?" : "Abandon \(tasks.count) Tasks?"
    }

    // MARK: Header

    private func header(_ list: BacklogList) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                Text(roadmap?.wave.displayName ?? model.repositoryName).font(Typography.strong(17))
                    .accessibilityIdentifier("wave-title")
                if let roadmap {
                    if let error = roadmap.projects.unavailableReason {
                        Text("Project unavailable · \(error)").font(Typography.body(12)).foregroundStyle(palette.warning)
                    } else if let project = roadmap.currentProject {
                        Text(project.name).font(Typography.body(12)).foregroundStyle(palette.textSecondary)
                            .lineLimit(1).accessibilityIdentifier("wave-current-project")
                    }
                    Button {
                        state.wrappedValue.showsObjective.toggle()
                    } label: {
                        HStack(spacing: Spacing.xs) {
                            Text("Objective")
                            Image(systemName: "chevron.right").font(.system(size: 9, weight: .bold))
                                .rotationEffect(.degrees(list.state.showsObjective ? 90 : 0))
                        }
                        .font(Typography.body(12)).foregroundStyle(palette.actionInk)
                    }
                    .buttonStyle(WorkPlainButtonStyle())
                    .help(roadmap.wave.goal)
                    .accessibilityIdentifier("wave-objective")
                }
                Spacer(minLength: 0)
                ShowHiddenMenu(model: model, view: view)
                if let roadmap { waveMenu(roadmap) }
                Button { model.navigation.palette = .newTask } label: { Label("New", systemImage: "plus") }
                    .buttonStyle(WorkOutlineButtonStyle()).help("File a Task (⌘N)")
                    .accessibilityIdentifier("backlog-new")
            }
            if let roadmap, list.state.showsObjective { objective(roadmap) }
            HStack(spacing: Spacing.md) {
                BacklogFilterPicker(selection: state.filter, count: list.count)
                Spacer(minLength: Spacing.sm)
                HStack(spacing: Spacing.sm) {
                    Image(systemName: "magnifyingglass").foregroundStyle(palette.textTertiary)
                    TextField("Search titles and briefs", text: state.query).textFieldStyle(.plain)
                        .accessibilityIdentifier("backlog-search")
                    if !list.state.query.isEmpty {
                        Button { state.wrappedValue.query = "" } label: { Image(systemName: "xmark.circle.fill") }
                            .buttonStyle(.plain).foregroundStyle(palette.textTertiary).help("Clear search")
                    }
                }
                .font(Typography.body(13))
                .padding(.horizontal, Spacing.sm).padding(.vertical, 5)
                .frame(maxWidth: 280)
                .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: 6))
            }
        }
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.md)
    }

    private func waveMenu(_ roadmap: WaveRoadmap) -> some View {
        Menu {
            Button("Project history") {
                model.historyReference = nil
                model.historyWave = roadmap.wave
            }.accessibilityIdentifier("wave-project-history")
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

    private func objective(_ roadmap: WaveRoadmap) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(roadmap.wave.goal.isEmpty ? "No objective recorded." : roadmap.wave.goal)
                .font(Typography.body(13)).foregroundStyle(palette.textSecondary).textSelection(.enabled)
            if let project = roadmap.currentProject {
                HStack(spacing: Spacing.md) {
                    Text(WaveMetricPortfolioPresentation(portfolio: roadmap.metricPortfolio).headline)
                        .font(Typography.body(12)).foregroundStyle(palette.textSecondary).lineLimit(1)
                        .accessibilityIdentifier("wave-metric-headline")
                    TrackerSyncView(sync: project.sync, repo: roadmap.wave.repo)
                }
                ForEach(project.krs) { kr in
                    Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "xmark.circle")
                        .font(Typography.body(12)).lineLimit(1)
                        .foregroundStyle(kr.holds ? palette.success : palette.textSecondary)
                }
            }
        }
        .accessibilityIdentifier("wave-objective-detail")
    }

    // MARK: Notices

    /// What the last backlog action did. Read failures, refusals and Tasks
    /// outside the plan go to the window's Notice.
    @ViewBuilder
    private var notices: some View {
        if let notice = list.state.notice {
            Text(notice).font(Typography.body(12.5)).foregroundStyle(palette.textSecondary).lineLimit(2)
                .textSelection(.enabled)
                .accessibilityIdentifier("backlog-notice")
                .padding(.horizontal, Spacing.xl)
                .padding(.vertical, Spacing.sm)
        }
    }

    // MARK: Rows

    private func content(_ list: BacklogList) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if list.rows.isEmpty, model.viewTasks(view) != nil { emptyState(list) }
                    ForEach(list.rows) { task in
                        VStack(alignment: .leading, spacing: 0) {
                            row(task, list)
                            if list.opened?.id == task.id { BacklogBrief(model: model, view: view, task: task, state: state, perform: perform) }
                        }
                        .id(task.id)
                        Divider().padding(.leading, Spacing.xl)
                    }
                    if list.truncated {
                        Text("Partial: searching the \(list.all.count) loaded").font(Typography.body(12))
                            .foregroundStyle(palette.warning).padding(Spacing.xl)
                    }
                    if list.canReorder, !list.rows.isEmpty {
                        Color.clear.frame(height: 24).contentShape(Rectangle())
                            .dropDestination(for: String.self) { items, _ in drop(items, before: nil, list) }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .focusable()
            .focused($focused)
            .focusEffectDisabled()
            .onKeyPress(phases: .down) { press in key(press, list) }
            .onChange(of: list.state.cursor) { _, row in if let row { proxy.scrollTo(row) } }
        }
        .accessibilityIdentifier("backlog-list")
    }

    private func row(_ task: RoadmapTask, _ list: BacklogList) -> some View {
        let selected = list.state.selection.contains(task.id)
        let isOpen = list.opened?.id == task.id
        return HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Toggle("Select \(task.task.identifier)", isOn: Binding(get: { selected }, set: { _ in state.wrappedValue.toggle(task.id) }))
                .toggleStyle(.checkbox).labelsHidden()
                .opacity(hovered == task.id || !list.state.selection.isEmpty ? 1 : 0)
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(task.task.identifier).font(Typography.code(11)).foregroundStyle(palette.textSecondary)
                    .frame(minWidth: 62, alignment: .leading).fixedSize()
                Text(task.task.name).font(isOpen ? Typography.strong(14) : Typography.body(14))
                    .lineLimit(isOpen ? nil : 1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if list.state.editing == task.id {
                    Text("draft").font(Typography.body(11)).foregroundStyle(palette.accentInk)
                }
                if let wave = list.waveLabel(task) {
                    Text(wave).font(Typography.body(11)).foregroundStyle(palette.textSecondary).lineLimit(1)
                }
                BacklogOwnerMark(owner: BacklogOwner(task), holder: task.runtime?.owner)
                Menu { BacklogActions(model: model, view: view, list: list, tasks: list.targets(task.id), perform: perform) } label: {
                    Image(systemName: "ellipsis")
                }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .foregroundStyle(palette.textTertiary).help("Actions")
                .opacity(hovered == task.id || selected ? 1 : 0.4)
                .accessibilityIdentifier("backlog-row-menu-\(task.id)")
            }
            .contentShape(Rectangle())
            .onTapGesture { click(task, list) }
        }
        .padding(.leading, Spacing.lg).padding(.trailing, Spacing.xl).padding(.vertical, 9)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isOpen ? palette.surfaceMuted : selected ? palette.selectionFill : .clear)
        .onHover { inside in
            if inside { hovered = task.id } else if hovered == task.id { hovered = nil }
        }
        .modifier(BacklogDrag(enabled: list.canReorder, id: task.id, title: list.targets(task.id).count > 1
                              ? "\(list.targets(task.id).count) Tasks" : task.task.name,
                              targeted: Binding(get: { dropTarget == task.id },
                                                set: { if $0 { dropTarget = task.id } else if dropTarget == task.id { dropTarget = nil } }),
                              drop: { drop($0, before: task.id, list) }))
        .overlay(alignment: .top) {
            if dropTarget == task.id { Rectangle().fill(palette.accent).frame(height: 2).allowsHitTesting(false) }
        }
        .contextMenu { BacklogActions(model: model, view: view, list: list, tasks: list.targets(task.id), perform: perform) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityAction(named: Text("Move up")) { perform(.nudge(-1, list.targets(task.id).map(\.id))) }
        .accessibilityAction(named: Text("Move down")) { perform(.nudge(1, list.targets(task.id).map(\.id))) }
        .accessibilityIdentifier("wave-row-\(task.id)")
    }

    private func click(_ task: RoadmapTask, _ list: BacklogList) {
        focused = true
        let keys = NSEvent.modifierFlags
        if keys.contains(.command) { state.wrappedValue.toggle(task.id) }
        else if keys.contains(.shift) { state.wrappedValue.extend(to: task.id, rows: list.ids) }
        else { state.wrappedValue.select(task.id) }
    }

    private func drop(_ items: [String], before target: String?, _ list: BacklogList) -> Bool {
        dropTarget = nil
        guard let source = items.first, list.all.contains(where: { $0.id == source }) else { return false }
        perform(.reorder(list.moves(list.targets(source).map(\.id), before: target)))
        return true
    }

    @ViewBuilder
    private func emptyState(_ list: BacklogList) -> some View {
        let empty = list.empty
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text(empty.title).font(Typography.strong(14))
            switch empty.step {
            case .clearSearch: Button("Clear search") { state.wrappedValue.query = "" }
            case .showAvailable: Button("Show available") { state.wrappedValue.filter = .available }
            case .showAll: Button("Show all") { state.wrappedValue.filter = .all }
            case .new: Button("New") { model.navigation.palette = .newTask }
            }
        }
        .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
        .padding(Spacing.xl).frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("backlog-empty")
    }

    // MARK: Keys

    private func key(_ press: KeyPress, _ list: BacklogList) -> KeyPress.Result {
        let modifiers = press.modifiers.intersection([.command, .shift, .option, .control])
        let selection = list.selected.map(\.id)
        switch press.key {
        case .downArrow, .upArrow:
            let offset = press.key == .downArrow ? 1 : -1
            if modifiers == .command { perform(.nudge(offset, selection)) }
            else if modifiers.isEmpty || modifiers == .shift { state.wrappedValue.step(offset, extending: modifiers == .shift, rows: list.ids) }
            else { return .ignored }
        case .escape where !selection.isEmpty: state.wrappedValue.clear()
        case .return where modifiers == .command && !selection.isEmpty: perform(.start(nil))
        case .return where modifiers.isEmpty && list.opened != nil: state.wrappedValue.showsAll.toggle()
        case .delete where modifiers == .command && !selection.isEmpty: perform(.abandon)
        default:
            switch (press.characters, modifiers) {
            case ("c", []) where !selection.isEmpty: perform(.claim)
            case ("e", []) where list.opened != nil: perform(.edit)
            case ("a", .command): state.wrappedValue.selectAll(list.ids)
            case ("z", .command):
                guard undoManager?.canUndo == true else { return .ignored }
                undoManager?.undo()
            default: return .ignored
            }
        }
        return .handled
    }

    // MARK: Perform

    /// Every backlog action goes through here, so the toolbar, row menu and keys agree.
    enum Action {
        case start(String?), claim, edit, abandon, toTop
        case nudge(Int, [String])
        case reorder([(task: RoadmapTask, rank: Int)])
        case move(String?)
        case save(RoadmapTask, String)
    }

    private func perform(_ action: Action) {
        let list = list
        let tasks = list.selected
        switch action {
        case .start(let graph): model.startBacklog(tasks, graph: graph, in: view)
        case .claim: register(model.claimBacklog(tasks, in: view), "Claim")
        case .edit:
            guard let task = list.opened else { return }
            state.wrappedValue.editing = task.id
            state.wrappedValue.draft = task.task.description
        case .abandon: abandoning = tasks.isEmpty ? nil : tasks
        case .toTop: reorder(list.toTop(tasks.map(\.id)))
        case .nudge(let offset, let ids):
            if list.canReorder { reorder(list.nudge(ids, by: offset)) }
            else { state.wrappedValue.notice = "Order lives in each Wave" }
        case .reorder(let writes): reorder(writes)
        case .move(let wave): model.moveBacklog(tasks, toWave: wave, in: view)
        case .save(let task, let text): Task { register(await model.saveBacklogBrief(task, text: text, in: view), "Edit Brief") }
        }
    }

    private func reorder(_ writes: [(task: RoadmapTask, rank: Int)]) {
        guard !writes.isEmpty else { return }
        Task { register(await model.rankBacklog(writes, in: view), "Reorder") }
    }

    private func register(_ change: BacklogChange?, _ name: String) {
        guard let change, let undoManager else { return }
        let (model, view) = (model, view)
        undoManager.registerUndo(withTarget: model) { model in
            Task { @MainActor in await model.revertBacklog(change, in: view) }
        }
        undoManager.setActionName(name)
    }
}

/// Drag a row to reorder; only a Wave's backlog takes drops.
private struct BacklogDrag: ViewModifier {
    let enabled: Bool
    let id: String
    let title: String
    @Binding var targeted: Bool
    let drop: ([String]) -> Bool
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        if enabled {
            content
                .draggable(id) {
                    Text(title).font(Typography.strong(13)).lineLimit(1)
                        .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
                        .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
                }
                .dropDestination(for: String.self) { items, _ in drop(items) } isTargeted: { targeted = $0 }
        } else {
            content
        }
    }
}

/// Filled for you, muted for someone else, a dashed ring for nobody.
struct BacklogOwnerMark: View {
    let owner: BacklogOwner
    let holder: TaskOwner?
    @Environment(\.palette) private var palette

    var body: some View {
        Group {
            switch owner {
            case .you: Image(systemName: "person.crop.circle.fill").foregroundStyle(palette.accentInk)
            case .someone: Image(systemName: "person.crop.circle.fill").foregroundStyle(palette.textTertiary)
            case .nobody:
                Circle().strokeBorder(palette.textTertiary, style: StrokeStyle(lineWidth: 1, dash: [2, 2]))
                    .frame(width: 13, height: 13)
            }
        }
        .font(.system(size: 14)).frame(width: 18)
        .help(help)
        .accessibilityLabel(help)
    }

    private var help: String {
        switch owner {
        case .you: "Claimed by you"
        case .someone: "Claimed by \(holder?.person ?? holder?.personKey ?? "someone else")"
        case .nobody: "Unclaimed"
        }
    }
}

/// All · Mine · Available in the palette's accent.
struct BacklogFilterPicker: View {
    @Binding var selection: BacklogFilter
    let count: (BacklogFilter) -> Int
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 2) {
            ForEach(BacklogFilter.allCases, id: \.self) { filter in
                let chosen = filter == selection
                Button { selection = filter } label: {
                    Text("\(filter.rawValue) \(count(filter))")
                        .font(chosen ? Typography.strong(12.5) : Typography.body(12.5))
                        .monospacedDigit()
                        .foregroundStyle(chosen ? palette.accentText : palette.textSecondary)
                        .padding(.horizontal, 10).padding(.vertical, 4)
                        .background(chosen ? palette.accent : .clear, in: Capsule())
                        .contentShape(Capsule())
                }
                .buttonStyle(WorkPlainButtonStyle())
                .accessibilityAddTraits(chosen ? .isSelected : [])
                .accessibilityIdentifier("backlog-filter-\(filter.rawValue.lowercased())")
            }
        }
        .padding(2)
        .background(palette.surfaceMuted, in: Capsule())
    }
}

/// The toolbar acts on the selection and is disabled with none. Start is its one
/// filled shape; the position, then ↑↓, give way before anything clips.
struct BacklogToolbar: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    let list: BacklogList
    let perform: (BacklogBoard.Action) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        ViewThatFits(in: .horizontal) {
            bar(position: true, arrows: true)
            bar(position: false, arrows: true)
            bar(position: false, arrows: false)
        }
        .disabled(list.selected.isEmpty)
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.sm)
        .background(palette.surfaceMuted.opacity(0.6))
        .accessibilityIdentifier("backlog-toolbar")
    }

    private var count: String { list.selected.count > 1 ? " \(list.selected.count)" : "" }

    private func bar(position: Bool, arrows: Bool) -> some View {
        let tasks = list.selected
        return HStack(spacing: Spacing.sm) {
            BacklogStartButton(model: model, list: list, perform: perform)
            Button("Claim\(count)") { perform(.claim) }
                .disabled(!tasks.contains { BacklogOwner($0) == .nobody })
                .help("Make it yours without starting it (C)").accessibilityIdentifier("backlog-claim")
            Button("Edit") { perform(.edit) }.disabled(list.opened == nil).help("Edit the brief (E)")
                .accessibilityIdentifier("backlog-edit")
            Menu("Move") { BacklogMoveItems(model: model, list: list, tasks: tasks, perform: perform) }
                .menuStyle(.borderlessButton).fixedSize().accessibilityIdentifier("backlog-move")
            Spacer(minLength: Spacing.sm)
            if position {
                Text(positionText).font(Typography.body(12)).foregroundStyle(palette.textSecondary).monospacedDigit()
                    .fixedSize().accessibilityIdentifier("backlog-position")
            }
            if arrows {
                Group {
                    Button { perform(.nudge(-1, tasks.map(\.id))) } label: { Image(systemName: "arrow.up") }.help("Move up (⌘↑)")
                    Button { perform(.nudge(1, tasks.map(\.id))) } label: { Image(systemName: "arrow.down") }.help("Move down (⌘↓)")
                }
                .disabled(!list.canReorder)
            }
            Button("Abandon\(count)…") { perform(.abandon) }.help("⌘⌫").accessibilityIdentifier("backlog-abandon")
        }
        .buttonStyle(BacklogQuietButtonStyle())
        .font(Typography.body(12.5))
        .fixedSize(horizontal: false, vertical: true)
    }

    private var positionText: String {
        if list.selected.count > 1 { return "\(list.selected.count) selected" }
        guard let task = list.selected.first, let index = list.all.firstIndex(where: { $0.id == task.id }) else { return "" }
        return "\(index + 1) of \(list.all.count)"
    }
}

/// The secondary actions' one quiet style: ink only, a fill on hover.
struct BacklogQuietButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(isEnabled ? palette.text : palette.textTertiary)
            .padding(.horizontal, Spacing.sm).padding(.vertical, 3)
            .background(RoundedRectangle(cornerRadius: 5)
                .fill(configuration.isPressed ? palette.surfaceElement : hovering && isEnabled ? palette.surfaceMuted : .clear))
            .contentShape(RoundedRectangle(cornerRadius: 5))
            .onHover { hovering = $0 }
            .workFocusRing()
    }
}

/// Start, a split control: the main part runs each Task's flow, the chevron lists the others.
struct BacklogStartButton: View {
    let model: WorkModel
    let list: BacklogList
    let perform: (BacklogBoard.Action) -> Void
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    var body: some View {
        let fill = isEnabled ? palette.accent : palette.surfaceElement
        let ink = isEnabled ? palette.accentText : palette.textTertiary
        HStack(spacing: 0) {
            Button { perform(.start(nil)) } label: {
                Text(list.startLabel).font(Typography.strong(12.5)).lineLimit(1)
                    .padding(.leading, Spacing.md).padding(.trailing, Spacing.sm).padding(.vertical, 4)
                    .contentShape(Rectangle())
            }
            .buttonStyle(WorkPlainButtonStyle())
            .help(list.selected.count > 1 && BacklogList.sharedFlow(list.selected) == nil
                  ? "Each Task on its Wave's flow (⌘↩)" : "⌘↩")
            .accessibilityIdentifier("backlog-start")
            Rectangle().fill(ink.opacity(0.35)).frame(width: 1).padding(.vertical, 4)
            Menu {
                BacklogStartItems(model: model, tasks: list.selected, perform: perform)
            } label: {
                Image(systemName: "chevron.down").font(.system(size: 10, weight: .bold))
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .tint(ink)
            .padding(.horizontal, Spacing.xs)
            .accessibilityIdentifier("backlog-start-menu")
        }
        .foregroundStyle(ink)
        .background(fill, in: Capsule())
        .fixedSize()
    }
}

/// Start's flow menu. With Waves whose defaults differ, the first item keeps
/// each Task's own flow, and any flow below it runs all of them on that one.
struct BacklogStartItems: View {
    let model: WorkModel
    let tasks: [RoadmapTask]
    let perform: (BacklogBoard.Action) -> Void

    var body: some View {
        let flows = (model.graphCatalog.value ?? []).compactMap(BacklogFlow.init)
        let described = flows.filter { $0.description != nil }
        let shared = BacklogList.sharedFlow(tasks)
        if shared == nil, !tasks.isEmpty {
            Toggle(isOn: Binding(get: { true }, set: { _ in perform(.start(nil)) })) {
                Text("Each Wave's flow")
                Text(BacklogList.breakdown(tasks))
            }
            Divider()
        }
        ForEach(described.filter { $0.stops > 0 }) { item($0, shared) }
        let undescribed = flows.filter { $0.description == nil }
        if !undescribed.isEmpty {
            Menu("More") { ForEach(undescribed) { item($0, shared) } }
        }
        Section("Run without stopping") {
            ForEach(described.filter { $0.stops == 0 }) { item($0, shared) }
        }
        if model.graphCatalog.value == nil {
            Text(model.graphCatalog.failure.map { "Flows unavailable · \($0)" } ?? "Reading flows…")
        }
    }

    private func item(_ flow: BacklogFlow, _ shared: String?) -> some View {
        Toggle(isOn: Binding(get: { flow.name == shared }, set: { _ in perform(.start(flow.name)) })) {
            Text("\(flow.name)  ·  \(flow.consequence)")
            if let summary = flow.summary { Text(summary) }
        }
    }
}

struct BacklogMoveItems: View {
    let model: WorkModel
    let list: BacklogList
    let tasks: [RoadmapTask]
    let perform: (BacklogBoard.Action) -> Void

    var body: some View {
        Menu("To Wave") {
            Button("No Wave") { perform(.move(nil)) }
            ForEach(model.visibleRoadmaps.filter { roadmap in tasks.allSatisfy { $0.task.repo == roadmap.wave.repo } },
                    id: \.wave.id) { roadmap in
                Button(roadmap.wave.displayName) { perform(.move(roadmap.wave.id)) }
                    .disabled(tasks.allSatisfy { $0.task.waveId == roadmap.wave.id })
            }
        }
        Button("To top") { perform(.toTop) }.disabled(!list.canReorder)
    }
}

/// A row's "···" and right-click menu, over the selection when the row is in it.
struct BacklogActions: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    let list: BacklogList
    let tasks: [RoadmapTask]
    let perform: (BacklogBoard.Action) -> Void

    var body: some View {
        // A row outside the selection becomes the selection, so every action below reads one set.
        let run = { (action: BacklogBoard.Action) in
            if Set(tasks.map(\.id)) != list.state.selection {
                model.navigation.backlogs[view, default: BacklogState()].selection = Set(tasks.map(\.id))
            }
            perform(action)
        }
        let flow = BacklogList.sharedFlow(tasks).map { " · \($0)" } ?? ""
        Button("Start\(flow)") { run(.start(nil)) }
        Menu("Start with") { BacklogStartItems(model: model, tasks: tasks, perform: run) }
        Button("Claim") { run(.claim) }.disabled(!tasks.contains { BacklogOwner($0) == .nobody })
        Button("Edit brief") { run(.edit) }.disabled(tasks.count != 1)
        Menu("Move") { BacklogMoveItems(model: model, list: list, tasks: tasks, perform: run) }
        Button("Move up") { run(.nudge(-1, tasks.map(\.id))) }.disabled(!list.canReorder)
        Button("Move down") { run(.nudge(1, tasks.map(\.id))) }.disabled(!list.canReorder)
        Button("Copy ID") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(tasks.map(\.task.identifier).joined(separator: " "), forType: .string)
        }
        Divider()
        Button("Abandon…", role: .destructive) { run(.abandon) }
    }
}

/// The opened Task's brief, cut at nine lines past 900 characters; Edit turns it into a field.
struct BacklogBrief: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    let task: RoadmapTask
    let state: Binding<BacklogState>
    let perform: (BacklogBoard.Action) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        let brief = task.task.description
        let long = brief.count > 900
        let editing = state.wrappedValue.editing == task.id
        VStack(alignment: .leading, spacing: Spacing.md) {
            if editing {
                TextEditor(text: state.draft)
                    .font(Typography.body(14)).scrollContentBackground(.hidden).frame(minHeight: 240)
                    .padding(Spacing.sm).background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border))
                    .accessibilityIdentifier("backlog-brief-editor")
                HStack(spacing: Spacing.sm) {
                    Button("Save") { perform(.save(task, state.wrappedValue.draft)) }
                        .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
                        .disabled(state.wrappedValue.draft == brief)
                        .keyboardShortcut("s", modifiers: .command)
                    Button("Cancel") { state.wrappedValue.editing = nil }.buttonStyle(WorkOutlineButtonStyle())
                        .keyboardShortcut(.cancelAction)
                }
            } else if brief.isEmpty {
                Text("No brief yet.").foregroundStyle(palette.textTertiary)
            } else {
                Text(Self.markdown(brief)).font(Typography.body(14)).lineSpacing(4).textSelection(.enabled)
                    .lineLimit(long && !state.wrappedValue.showsAll ? 9 : nil)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("backlog-brief-\(task.id)")
                if long {
                    Button(state.wrappedValue.showsAll ? "Show less" : "Show all \(Self.words(brief)) words") {
                        state.wrappedValue.showsAll.toggle()
                    }
                    .buttonStyle(WorkLinkStyle()).font(Typography.body(12.5)).help("Return")
                }
            }
        }
        .frame(maxWidth: 720, alignment: .leading)
        .padding(.leading, 96).padding(.trailing, Spacing.xl).padding(.top, Spacing.xs).padding(.bottom, Spacing.lg)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted)
    }

    static func words(_ text: String) -> Int { text.split(whereSeparator: \.isWhitespace).count }

    private static func markdown(_ source: String) -> AttributedString {
        (try? AttributedString(markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(source)
    }
}

// MARK: - Wave view chrome

/// A Wave view's header row while its backlog is hidden: the view's name and its
/// `Show…` menu, which the backlog header carries otherwise.
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

#endif
