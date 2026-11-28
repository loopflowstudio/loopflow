// A Wave's backlog: its open work nobody is running, as one ranked list beside its conversation.

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

/// Who holds a backlog Task, as Rust decides it.
enum BacklogOwner: Equatable {
    case you, someone, nobody

    init(_ task: RoadmapTask) {
        if task.ownedByYou { self = .you } else if task.runtime?.owner != nil { self = .someone } else { self = .nobody }
    }
}

/// What the last backlog action did, or what failed with the Machine's message.
enum BacklogNotice: Equatable {
    case did(String)
    case failed(Failure)
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
    var notice: BacklogNotice?

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

    /// Drop selected rows the list no longer shows, as a changed filter or search leaves it.
    mutating func keep(_ rows: [String]) {
        selection.formIntersection(rows)
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

/// A Graph as Start and New offer it for a Task, read from the catalog: its
/// description, how many times it stops for you and whether it merges before
/// the first stop.
struct StartFlow: Equatable, Identifiable {
    let name: String
    let description: String?
    let stops: Int
    let merges: Bool
    /// Another Graph includes it with `graph:`: a piece, not somewhere to start.
    let isPiece: Bool
    var id: String { name }

    /// Nil for a Graph that does not load, or that runs for a Wave and not a Task.
    init?(_ entry: GraphCatalogEntry) {
        guard entry.runs == .task, let compiled = entry.compiled, let workflow = entry.workflow else { return nil }
        func nodes(_ steps: [GraphNode]) -> [GraphNode] { steps.flatMap { [$0] + $0.paths.flatMap { nodes($0.steps) } } }
        let all = nodes(compiled.steps)
        let firstStop = all.firstIndex(where: { $0.kind == .breakpoint }) ?? all.count
        name = entry.name
        description = entry.description
        stops = workflow.breakpoints.count
        merges = all[..<firstStop].contains { node in
            node.kind == .op && (node.label.hasPrefix("land") || node.label.hasPrefix("pr land"))
        }
        isPiece = !entry.includedBy.isEmpty
    }

    /// Offered first. A piece, or a Graph with no description, sits under More.
    var isFirstChoice: Bool { description != nil && !isPiece }

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

/// The workflows a Task can start on, as Start's menu and New Task's lay
/// them out: those that stop for you, the pieces and undescribed ones under
/// More, and those that never stop.
struct StartFlows: Equatable {
    let stopping: [StartFlow]
    let more: [StartFlow]
    let unattended: [StartFlow]

    init(_ catalog: [GraphCatalogEntry]) {
        let flows = catalog.compactMap(StartFlow.init)
        stopping = flows.filter { $0.isFirstChoice && $0.stops > 0 }
        more = flows.filter { !$0.isFirstChoice }
        unattended = flows.filter { $0.isFirstChoice && $0.stops == 0 }
    }
}

/// A view's backlog: its open work nobody is running, in rank order, filtered
/// and searched, with the selection the view holds. Most rows are unstarted;
/// a started Task nobody holds is here too and says where it stands. Built
/// from Rust's reading on every render; it owns no order or ownership of its own.
struct BacklogList {
    /// Every backlog Task, in rank order, before filtering.
    let all: [RoadmapTask]
    /// What the list shows.
    let rows: [RoadmapTask]
    let truncated: Bool
    /// The Wave this backlog belongs to; nil over a whole repository.
    let wave: String?
    let waveNames: [String: String]
    let state: BacklogState
    /// The selected rows the list shows; a row the filter or search hides is not selected.
    let selection: Set<String>

    init(tasks: [RoadmapTask], truncated: Bool, wave: String?, waveNames: [String: String], state: BacklogState) {
        all = tasks.filter { $0.section == .backlog }.sorted { $0.task.rank < $1.task.rank }
        self.truncated = truncated
        self.wave = wave
        self.waveNames = waveNames
        self.state = state
        let needle = state.query.trimmingCharacters(in: .whitespacesAndNewlines)
        rows = all.filter { task in
            Self.passes(task, state.filter) && (needle.isEmpty
                || "\(task.task.identifier) \(task.task.name) \(task.task.description)".localizedCaseInsensitiveContains(needle))
        }
        selection = state.selection.intersection(rows.map(\.id))
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

    /// The selected Tasks in list order: what every action reads.
    var selected: [RoadmapTask] { rows.filter { selection.contains($0.id) } }

    /// The one selected Task, whose brief opens under its row.
    var opened: RoadmapTask? { selection.count == 1 ? selected.first : nil }

    /// The Tasks an action on `id` applies to: the selection when the row is in it, else the row alone.
    func targets(_ id: String) -> [RoadmapTask] {
        selection.contains(id) ? selected : rows.filter { $0.id == id }
    }

    /// The Wave a row names when it isn't the one being looked at.
    func waveLabel(_ task: RoadmapTask) -> String? {
        guard let id = task.task.waveId, id != wave else { return nil }
        return waveNames[id]
    }

    /// "claimed by you" or "claimed by Ana". An unclaimed Task says nothing:
    /// the Available count already does.
    static func claim(_ task: RoadmapTask) -> String? {
        switch BacklogOwner(task) {
        case .you: "claimed by you"
        case .someone: "claimed by \(task.runtime?.owner?.person ?? task.runtime?.owner?.personKey ?? "someone else")"
        case .nobody: nil
        }
    }

    /// "Growth, claimed by Ana": a row's Wave when it isn't this one, then whose it is.
    func ownership(_ task: RoadmapTask) -> String? {
        let parts = [waveLabel(task), Self.claim(task)].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: ", ")
    }

    // MARK: Start

    /// The one workflow every Task would run, the Graph Rust says its first run
    /// takes up, or nil when they differ.
    static func sharedWorkflow(_ tasks: [RoadmapTask]) -> String? {
        let names = Set(tasks.map(\.graphName))
        return names.count == 1 ? names.first ?? nil : nil
    }

    /// "feature 2, code 1": the most common workflow first.
    static func breakdown(_ tasks: [RoadmapTask]) -> String {
        let counts: [(name: String, count: Int)] = Dictionary(grouping: tasks.compactMap(\.graphName), by: { $0 })
            .map { ($0.key, $0.value.count) }
        let ordered = counts.sorted { left, right in
            left.count != right.count ? left.count > right.count : left.name < right.name
        }
        return ordered.map { "\($0.name) \($0.count)" }.joined(separator: ", ")
    }

    /// "PR #12" once the Task's PR is on GitHub.
    static func pullRequest(_ task: RoadmapTask) -> String? {
        task.pr?.publication?.github.map { "PR #\($0.number)" }
    }

    /// A Task's step and its PR, for the ones it has: "at demo", "PR #12".
    static func position(_ task: RoadmapTask) -> [String] {
        let step = task.execution?.step ?? task.latestGraphProcess?.steps.last?.label
        return [step.map { "at \($0)" }, pullRequest(task)].compactMap { $0 }
    }

    /// The status word a row shows: a started Task's, and "Blocked" or "Waiting"
    /// on one that has not started. Most rows have never run and show none.
    static func status(_ task: RoadmapTask) -> TaskRowStatus? {
        let standing = TaskRowStatus(task: task)
        return !task.awaitsStart || standing.source == .condition ? standing : nil
    }

    /// "Idle, at demo, PR #12": the word the Task's row shows everywhere, then where it is.
    static func standing(_ task: RoadmapTask) -> String? {
        guard let word = status(task)?.word else { return nil }
        return ([word] + position(task)).joined(separator: ", ")
    }

    /// The started Task that Start's place opens: one, selected alone. The
    /// backlog runs unstarted Tasks only; a started one continues from its own view.
    static func opens(_ tasks: [RoadmapTask]) -> RoadmapTask? {
        tasks.count == 1 && !tasks[0].awaitsStart ? tasks[0] : nil
    }

    /// What Start says it will do: "Start with feature", "Start 2 with feature",
    /// "Start 3: feature 2, code 1", and "Open" for one started Task. Started
    /// Tasks among others are not counted.
    static func startLabel(_ tasks: [RoadmapTask]) -> String {
        if opens(tasks) != nil { return "Open" }
        let fresh = tasks.filter(\.awaitsStart)
        guard !fresh.isEmpty else { return "Start" }
        let start = "Start" + (tasks.count > 1 ? " \(fresh.count)" : "")
        if let shared = sharedWorkflow(fresh) { return "\(start) with \(shared)" }
        let names = breakdown(fresh)
        return names.isEmpty ? start : "\(start): \(names)"
    }

    var startLabel: String { Self.startLabel(selected) }

    /// "1 started Task: open it to continue", for the started Tasks Start leaves alone.
    static func startedNote(_ tasks: [RoadmapTask]) -> String? {
        let started = tasks.count { !$0.awaitsStart }
        guard started > 0, opens(tasks) == nil else { return nil }
        return started == 1 ? "1 started Task: open it to continue" : "\(started) started Tasks: open each to continue"
    }

    /// Why Start is off: only started Tasks are selected, or Rust would refuse
    /// to run every unstarted one.
    static func startUnavailable(_ tasks: [RoadmapTask]) -> String? {
        let fresh = tasks.filter(\.awaitsStart)
        guard !fresh.isEmpty else { return startedNote(tasks) }
        let reasons = fresh.compactMap(\.runControl.unavailable)
        return reasons.count == fresh.count ? reasons.first : nil
    }

    // MARK: Abandon

    /// What the Abandon dialog says before writing: which Tasks, and which have a PR.
    static func abandonMessage(_ tasks: [RoadmapTask]) -> String {
        func named(_ items: [String]) -> String {
            items.count > 5 ? items.prefix(5).joined(separator: ", ") + " and \(items.count - 5) more"
                : items.joined(separator: ", ")
        }
        var lines: [String] = []
        if tasks.count > 1 { lines.append(named(tasks.map(\.task.identifier))) }
        let published = tasks.compactMap { task in pullRequest(task).map { "\(task.task.identifier) (\($0))" } }
        if !published.isEmpty { lines.append("With a PR: \(named(published))") }
        lines.append("Each keeps its ID and history; its PRs and branches are removed.")
        return lines.joined(separator: "\n")
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

    /// A Wave's heading lines: name and status, objective, then its Project.
    static func heading(_ roadmap: WaveRoadmap) -> [String] {
        var lines = ["\(roadmap.wave.displayName) · \(roadmap.wave.status.label)"]
        if let objective = roadmap.wave.goal.split(separator: "\n").first { lines.append(String(objective)) }
        if let project = roadmap.project.present {
            lines.append("\(project.name) · \(project.status.rawValue.capitalized)")
        }
        return lines
    }

    /// The backlog as an agent reads it with `lf desktop capture -p backlog`:
    /// the filter counts, the shown one bracketed, then the rows in order with
    /// where a started one stands, its Wave when it is another, and who claimed it.
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
            if let standing = Self.standing(task) { line += " · \(standing)" }
            if let ownership = ownership(task) { line += " · \(ownership)" }
            if selection.contains(task.id) { line = "›" + line.dropFirst() }
            return line
        }
        if truncated { lines.append("  Partial: searching the \(all.count) loaded") }
        return lines.joined(separator: "\n")
    }

    // MARK: Empty

    /// Why the list is empty, and the one step that fills it.
    enum EmptyStep: Equatable { case clearSearch, showAvailable, showAll, new }

    var empty: (title: String, step: EmptyStep) {
        if !state.query.trimmingCharacters(in: .whitespaces).isEmpty {
            return ("Nothing in \(state.filter.rawValue) matches “\(state.query)”.", .clearSearch)
        }
        switch state.filter {
        case .mine: return ("You haven't claimed anything. \(count(.available)) are available.", .showAvailable)
        case .available where !all.isEmpty: return ("Everything here is claimed by someone else.", .showAll)
        default: return ("Nothing is waiting in this \(wave == nil ? "repository" : "Wave").", .new)
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
    private func backlogNotice(_ view: WorkNavigation.Board, _ did: String) {
        navigation.backlogs[view, default: BacklogState()].notice = .did(did)
    }

    private func backlogFailure(_ view: WorkNavigation.Board, _ what: String, _ error: any Error) {
        navigation.backlogs[view, default: BacklogState()].notice = .failed(Failure(what: what, machine: error.localizedDescription))
    }

    /// Write each rank in turn; the first refusal stops the rest and says so.
    /// `landed` is the notice when every write lands. Returns the inverse when any landed.
    @discardableResult
    func rankBacklog(_ writes: [(task: RoadmapTask, rank: Int)], in view: WorkNavigation.Board,
                     landed notice: String? = nil) async -> BacklogChange? {
        guard !writes.isEmpty else { return nil }
        var landed: [(task: RoadmapTask, rank: Int)] = []
        for write in writes {
            do { try await rankTask(write.task, rank: write.rank) } catch {
                backlogFailure(view, "\(write.task.task.identifier)'s order not saved.", error)
                break
            }
            landed.append(write)
        }
        if landed.count == writes.count {
            let moved = Set(writes.map(\.task.id)).count
            backlogNotice(view, notice ?? (moved == 1 ? "Moved \(writes[0].task.task.identifier)" : "Moved \(moved) Tasks"))
        }
        return landed.isEmpty ? nil : .ranks(BacklogList.inverse(landed))
    }

    // Claim, Start, Move and Abandon show in the click; a refusal posts its own
    // Notice and puts that Task's row back.

    /// Claim each unclaimed Task without running it; returns the inverse. A
    /// started Task you claim is yours to continue, so it leaves for Working.
    @discardableResult
    func claimBacklog(_ tasks: [RoadmapTask], in view: WorkNavigation.Board) -> BacklogChange? {
        let claimed = tasks.filter { BacklogOwner($0) == .nobody }
        for task in claimed { assignTask(task, claimed: true) }
        let skipped = tasks.count - claimed.count
        let started = claimed.count { !$0.awaitsStart }
        backlogNotice(view, [
            "Claimed \(claimed.count == 1 ? claimed[0].task.identifier : String(claimed.count))",
            started == 0 ? nil : started == claimed.count ? "now in Working" : "\(started) now in Working",
            skipped > 0 ? "\(skipped) already claimed" : nil,
        ].compactMap { $0 }.joined(separator: ", "))
        return claimed.isEmpty ? nil : .claims(claimed)
    }

    /// Start each unstarted Task, on `graph` or its own workflow. Started Tasks stay as they are.
    func startBacklog(_ tasks: [RoadmapTask], graph: String?, in view: WorkNavigation.Board) {
        let fresh = tasks.filter(\.awaitsStart)
        guard !fresh.isEmpty else { return }
        for task in fresh { startTask(graph, task: task) }
        navigation.backlogs[view, default: BacklogState()].selection.subtract(fresh.map(\.id))
        backlogNotice(view, "Started \(fresh.count)")
    }

    /// Go to a Task's own view, where its position, PR and run action live.
    func openBacklog(_ task: RoadmapTask) {
        openTaskDestination(wave: self.task(id: task.id)?.wave, task: task)
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
            backlogFailure(view, "Brief not saved.", error)
            return nil
        }
    }

    /// Put a change back. A brief someone changed since stays as it is.
    func revertBacklog(_ change: BacklogChange, in view: WorkNavigation.Board) async {
        switch change {
        case .ranks(let writes):
            await rankBacklog(writes, in: view, landed: "Order restored")
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
            } catch { backlogFailure(view, "Brief not restored.", error) }
        }
    }
}

// MARK: - Views

/// A Wave view's backlog, the repository page included. A row's brief opens in
/// place; nothing starts until Start.
struct BacklogView: View {
    let model: WorkModel
    /// `.wave(id)` or `.repository`.
    let view: WorkNavigation.Board
    @Environment(\.palette) private var palette
    @Environment(\.undoManager) private var undoManager
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
        // and keeps a stale size there; this one is the backlog's frame exactly.
        .background(palette.background, ignoresSafeAreaEdges: [])
        .accessibilityIdentifier("wave-backlog")
        .task(id: model.repoPath) { await model.loadGraphCatalog() }
        .onChange(of: [list.state.filter.rawValue, list.state.query]) { state.wrappedValue.keep(model.backlog(view).ids) }
        .confirmationDialog(abandonTitle, isPresented: Binding(get: { abandoning != nil }, set: { if !$0 { abandoning = nil } })) {
            Button("Abandon", role: .destructive) {
                if let tasks = abandoning { model.abandonBacklog(tasks, in: view) }
                abandoning = nil
            }
        } message: {
            Text(BacklogList.abandonMessage(abandoning ?? []))
        }
    }

    private var abandonTitle: String {
        guard let tasks = abandoning else { return "" }
        return tasks.count == 1 ? "Abandon \(tasks[0].task.identifier)?" : "Abandon \(tasks.count) Tasks?"
    }

    // MARK: Header

    private func header(_ list: BacklogList) -> some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            // The objective's disclosure gives way before the name does.
            ViewThatFits(in: .horizontal) {
                titleLine(list, objective: true)
                titleLine(list, objective: false)
            }
            if let roadmap, list.state.showsObjective { objective(roadmap) }
            // Side by side, or the search under the filter when the backlog is narrow.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: Spacing.md) {
                    BacklogFilterPicker(selection: state.filter, count: list.count)
                    Spacer(minLength: Spacing.sm)
                    search(list).frame(minWidth: 180, maxWidth: 280)
                }
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    BacklogFilterPicker(selection: state.filter, count: list.count)
                    search(list)
                }
            }
        }
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.md)
    }

    private func titleLine(_ list: BacklogList, objective showsObjective: Bool) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
            Text(roadmap?.wave.displayName ?? model.repositoryName).serif(.display)
                .foregroundStyle(palette.emphasisInk).lineLimit(1)
                .layoutPriority(1).accessibilityIdentifier("wave-title")
            if let roadmap {
                if let error = roadmap.project.unavailableReason {
                    FailureLine("Project unavailable.", machine: error).lineLimit(1)
                } else if let project = roadmap.project.present {
                    WaveHeaderProject(project: project)
                }
            }
            if let roadmap, showsObjective {
                Button {
                    state.wrappedValue.showsObjective.toggle()
                } label: {
                    HStack(spacing: Spacing.xs) {
                        Text("Objective")
                        Image(systemName: "chevron.right").font(Typography.meta).fontWeight(.bold)
                            .rotationEffect(.degrees(list.state.showsObjective ? 90 : 0))
                    }
                    .font(Typography.text).foregroundStyle(palette.actionInk)
                }
                .buttonStyle(WorkPlainButtonStyle()).fixedSize()
                .help(roadmap.wave.goal)
                .accessibilityIdentifier("wave-objective")
            }
            Spacer(minLength: 0)
            ShowHiddenMenu(model: model, view: view)
            if let workflow = roadmap?.graph { workflowMenu(workflow) }
            Button { model.navigation.palette = .newTask } label: { Label("New", systemImage: "plus") }
                .buttonStyle(LoopflowButtonStyle()).fixedSize().help("File a Task (⌘N)")
                .accessibilityIdentifier("backlog-new")
        }
    }

    private func search(_ list: BacklogList) -> some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: "magnifyingglass").foregroundStyle(palette.textTertiary)
            TextField("Search titles and briefs", text: state.query).textFieldStyle(.plain)
                .accessibilityIdentifier("backlog-search")
            if !list.state.query.isEmpty {
                Button { state.wrappedValue.query = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.plain).foregroundStyle(palette.textTertiary).help("Clear search")
            }
        }
        .font(Typography.text)
        .padding(.horizontal, Spacing.sm).frame(minHeight: HitTarget.minimum)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.control))
        .overlay(RoundedRectangle(cornerRadius: CornerRadius.control).strokeBorder(palette.borderStrong))
    }

    private func workflowMenu(_ workflow: String) -> some View {
        Menu {
            Button("Workflow: \(workflow)") {
                Task {
                    await model.loadGraphCatalog()
                    model.navigation.palette = .workflow(workflow)
                }
            }.accessibilityIdentifier("wave-workflow")
        } label: { Image(systemName: "ellipsis.circle") }
            .menuStyle(.borderlessButton).fixedSize()
            .accessibilityIdentifier("wave-backlog-menu")
    }

    /// The Wave's objective, then its Project as the Portfolio draws it.
    private func objective(_ roadmap: WaveRoadmap) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(roadmap.wave.goal.isEmpty ? "No objective recorded." : roadmap.wave.goal)
                .font(Typography.text).foregroundStyle(palette.textSecondary).textSelection(.enabled)
            if let project = roadmap.project.present { WaveProjectView(project: project, repo: roadmap.wave.repo) }
        }
        .accessibilityIdentifier("wave-objective-detail")
    }

    // MARK: Notices

    /// What the last backlog action did. Read failures, refusals and Tasks
    /// outside the plan go to the window's Notice.
    @ViewBuilder
    private var notices: some View {
        if let notice = list.state.notice {
            Group {
                switch notice {
                case .did(let did):
                    Text(did).font(Typography.text).foregroundStyle(palette.textSecondary).lineLimit(2)
                        .textSelection(.enabled)
                case .failed(let failure):
                    FailureLine(failure)
                }
            }
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
                        let isOpen = list.opened?.id == task.id
                        VStack(alignment: .leading, spacing: 0) {
                            row(task, list)
                            if let reason = TaskRowStatus(task: task).line {
                                BacklogUnderTitle(identifier: task.task.identifier) {
                                    Text(verbatim: reason).font(Typography.text).foregroundStyle(palette.textSecondary)
                                        .lineLimit(2).frame(maxWidth: .infinity, alignment: .leading)
                                        .accessibilityIdentifier("backlog-reason-\(task.id)")
                                }
                            }
                            if isOpen {
                                BacklogUnderTitle(identifier: task.task.identifier) {
                                    BacklogBrief(model: model, view: view, task: task, state: state, perform: perform)
                                }
                            }
                        }
                        .background(isOpen ? palette.surfaceMuted : .clear)
                        .id(task.id)
                        Divider()
                    }
                    if list.truncated {
                        Text("Partial: searching the \(list.all.count) loaded").font(Typography.text)
                            .foregroundStyle(palette.warning).padding(Spacing.xl)
                    }
                    if list.canReorder, !list.rows.isEmpty {
                        Color.clear.frame(height: HitTarget.minimum).contentShape(Rectangle())
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
        let selected = list.selection.contains(task.id)
        let isOpen = list.opened?.id == task.id
        return HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Toggle("Select \(task.task.identifier)", isOn: Binding(get: { selected }, set: { _ in state.wrappedValue.toggle(task.id) }))
                .toggleStyle(.checkbox).labelsHidden()
                .frame(width: HitTarget.minimum)
            Button { state.wrappedValue.select(task.id) } label: {
                Image(systemName: "chevron.right").font(Typography.meta).fontWeight(.bold)
                    .rotationEffect(.degrees(isOpen ? 90 : 0))
                    .frame(width: HitTarget.minimum, height: HitTarget.minimum).contentShape(Rectangle())
            }
            .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
            .help(isOpen ? "Hide brief" : "Show brief")
            .accessibilityLabel(isOpen ? "Hide brief" : "Show brief")
            .accessibilityIdentifier("backlog-row-brief-\(task.id)")
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                BacklogIdentifier(identifier: task.task.identifier)
                Text(task.task.name).font(isOpen ? Typography.textStrong : Typography.text)
                    .lineLimit(isOpen ? nil : 1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if list.state.editing == task.id {
                    Text("draft").font(Typography.meta).foregroundStyle(palette.accentInk)
                }
                if let standing = BacklogList.status(task), let word = standing.word {
                    Text("\(Text(verbatim: word).foregroundStyle(standing.tone?.ink(in: palette) ?? palette.textSecondary))\(BacklogList.position(task).map { ", \($0)" }.joined())")
                        .font(Typography.meta).foregroundStyle(palette.textSecondary).lineLimit(1)
                        .help(Text(verbatim: standing.detail ?? word))
                        .accessibilityIdentifier("backlog-standing-\(task.id)")
                }
                if let ownership = list.ownership(task) {
                    Text(verbatim: ownership).font(Typography.meta).foregroundStyle(palette.textSecondary).lineLimit(1)
                        .accessibilityIdentifier("backlog-ownership-\(task.id)")
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { click(task, list) }
            Menu { actions(task, list) } label: {
                Image(systemName: "ellipsis")
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden)
            .frame(width: HitTarget.minimum)
            .foregroundStyle(palette.textSecondary).help("Actions")
            .accessibilityLabel("More for \(task.task.identifier)")
            .accessibilityIdentifier("backlog-row-menu-\(task.id)")
        }
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.xs)
        .frame(maxWidth: .infinity, minHeight: HitTarget.comfortable, alignment: .leading)
        .background(selected && !isOpen ? palette.selectionFill : .clear)
        .modifier(BacklogDrag(enabled: list.canReorder, id: task.id, title: list.targets(task.id).count > 1
                              ? "\(list.targets(task.id).count) Tasks" : task.task.name,
                              targeted: Binding(get: { dropTarget == task.id },
                                                set: { if $0 { dropTarget = task.id } else if dropTarget == task.id { dropTarget = nil } }),
                              drop: { drop($0, before: task.id, list) }))
        .overlay(alignment: .top) {
            if dropTarget == task.id { Rectangle().fill(palette.accent).frame(height: Spacing.xxs).allowsHitTesting(false) }
        }
        .contextMenu { actions(task, list) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityAction(named: Text("Move up")) { perform(.nudge(-1), on: list.targets(task.id)) }
        .accessibilityAction(named: Text("Move down")) { perform(.nudge(1), on: list.targets(task.id)) }
        .accessibilityIdentifier("wave-row-\(task.id)")
    }

    /// A row's "···" and right-click menu.
    private func actions(_ task: RoadmapTask, _ list: BacklogList) -> BacklogActions {
        let tasks = list.targets(task.id)
        return BacklogActions(model: model, list: list, tasks: tasks) { perform($0, on: tasks) }
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
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(empty.title).emptyLine()
            switch empty.step {
            case .clearSearch: Button("Clear search") { state.wrappedValue.query = "" }
            case .showAvailable: Button("Show available") { state.wrappedValue.filter = .available }
            case .showAll: Button("Show all") { state.wrappedValue.filter = .all }
            case .new: Button("New") { model.navigation.palette = .newTask }
            }
        }
        .buttonStyle(LoopflowButtonStyle())
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
            if modifiers == .command { perform(.nudge(offset)) }
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
        case nudge(Int)
        case reorder([(task: RoadmapTask, rank: Int)])
        case move(String?)
        case save(RoadmapTask, String)
    }

    private func perform(_ action: Action) {
        let list = list
        let tasks = list.selected
        switch action {
        case .start(let graph):
            guard BacklogList.startUnavailable(tasks) == nil else { return }
            if let task = BacklogList.opens(tasks) { model.openBacklog(task) }
            else { model.startBacklog(tasks, graph: graph, in: view) }
        case .claim: register(model.claimBacklog(tasks, in: view), "Claim")
        case .edit:
            guard let task = list.opened else { return }
            state.wrappedValue.editing = task.id
            state.wrappedValue.draft = task.task.description
        case .abandon: abandoning = tasks.isEmpty ? nil : tasks
        case .toTop: reorder(list.toTop(tasks.map(\.id)))
        case .nudge(let offset):
            if list.canReorder { reorder(list.nudge(tasks.map(\.id), by: offset)) }
            else { state.wrappedValue.notice = .did("Order lives in each Wave") }
        case .reorder(let writes): reorder(writes)
        case .move(let wave): model.moveBacklog(tasks, toWave: wave, in: view)
        case .save(let task, let text): Task { register(await model.saveBacklogBrief(task, text: text, in: view), "Edit Brief") }
        }
    }

    /// A row's own menu acts on `tasks`: a row outside the selection becomes the
    /// selection, so every action reads one set.
    private func perform(_ action: Action, on tasks: [RoadmapTask]) {
        let ids = Set(tasks.map(\.id))
        if ids != list.selection { state.wrappedValue.selection = ids }
        perform(action)
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
                    Text(title).font(Typography.textStrong).lineLimit(1)
                        .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
                        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
                }
                .dropDestination(for: String.self) { items, _ in drop(items) } isTargeted: { targeted = $0 }
        } else {
            content
        }
    }
}

/// A row's ID, in the column every row gives it.
private struct BacklogIdentifier: View {
    let identifier: String
    @Environment(\.palette) private var palette

    var body: some View {
        Text(verbatim: identifier).font(Typography.mono).foregroundStyle(palette.textSecondary)
            .frame(minWidth: 64, alignment: .leading).fixedSize()
    }
}

/// What sits under a row's title: the row's own columns, empty, come first, so
/// the content starts where the title does.
private struct BacklogUnderTitle<Content: View>: View {
    let identifier: String
    @ViewBuilder var content: Content

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Color.clear.frame(width: HitTarget.minimum, height: 0)
            Color.clear.frame(width: HitTarget.minimum, height: 0)
            BacklogIdentifier(identifier: identifier).frame(height: 0).hidden()
            content
            Color.clear.frame(width: HitTarget.minimum, height: 0)
        }
        .padding(.horizontal, Spacing.xl).padding(.bottom, Spacing.sm)
    }
}

/// All, Mine, Available: one control of three segments, the chosen one in the
/// selection fill.
struct BacklogFilterPicker: View {
    @Binding var selection: BacklogFilter
    let count: (BacklogFilter) -> Int
    @Environment(\.palette) private var palette

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: CornerRadius.control)
        HStack(spacing: 0) {
            ForEach(BacklogFilter.allCases, id: \.self) { filter in
                let chosen = filter == selection
                if filter != BacklogFilter.allCases.first { Rectangle().fill(palette.borderStrong).frame(width: 1) }
                Button { selection = filter } label: {
                    HStack(spacing: Spacing.xs) {
                        Text(filter.rawValue).font(chosen ? Typography.textStrong : Typography.text)
                            .foregroundStyle(chosen ? palette.workSelectedText : palette.text)
                        Text("\(count(filter))").font(Typography.text).monospacedDigit()
                            .foregroundStyle(chosen ? palette.workSelectedSecondaryText : palette.textSecondary)
                    }
                    .padding(.horizontal, Spacing.md).frame(minHeight: HitTarget.minimum)
                    .background(chosen ? palette.workSelectedFill : palette.surface)
                    .contentShape(Rectangle())
                }
                .buttonStyle(WorkPlainButtonStyle())
                .accessibilityAddTraits(chosen ? .isSelected : [])
                .accessibilityIdentifier("backlog-filter-\(filter.rawValue.lowercased())")
            }
        }
        .clipShape(shape)
        .overlay(shape.strokeBorder(palette.borderStrong))
        .fixedSize()
    }
}

/// The action bar says what it acts on, then acts on it; with nothing selected
/// its buttons are off. Every action is the one button and Start is the
/// accent. The position, then ↑↓, give way, then Edit, Move and Abandon fold
/// into one menu. No label wraps.
struct BacklogToolbar: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    let list: BacklogList
    let perform: (BacklogView.Action) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        ViewThatFits(in: .horizontal) {
            bar(position: true, arrows: true)
            bar(position: false, arrows: true)
            bar(position: false, arrows: false)
            folded
        }
        .disabled(list.selected.isEmpty)
        .padding(.horizontal, Spacing.xl).padding(.vertical, Spacing.sm)
        .background(palette.surfaceMuted)
        .accessibilityIdentifier("backlog-toolbar")
    }

    private var selected: some View {
        Text(list.selected.isEmpty ? "Nothing selected" : "\(list.selected.count) selected")
            .font(Typography.text).foregroundStyle(palette.textSecondary).monospacedDigit().fixedSize()
            .accessibilityIdentifier("backlog-selected")
    }

    private var claim: some View {
        Button("Claim") { perform(.claim) }
            .disabled(!list.selected.contains { BacklogOwner($0) == .nobody })
            .help("Make it yours without starting it (C)").accessibilityIdentifier("backlog-claim")
    }

    private func bar(position: Bool, arrows: Bool) -> some View {
        let tasks = list.selected
        return HStack(spacing: Spacing.sm) {
            selected
            BacklogStartButton(model: model, list: list, perform: perform)
            claim
            Button("Edit") { perform(.edit) }.disabled(list.opened == nil).help("Edit the brief (E)")
                .accessibilityIdentifier("backlog-edit")
            Menu { BacklogMoveItems(model: model, list: list, tasks: tasks, perform: perform) } label: {
                HStack(spacing: Spacing.xs) {
                    Text("Move")
                    Image(systemName: "chevron.down").font(Typography.meta)
                }
            }
            .menuStyle(.button).menuIndicator(.hidden).fixedSize().accessibilityIdentifier("backlog-move")
            Button("Abandon…") { perform(.abandon) }.help("⌘⌫").accessibilityIdentifier("backlog-abandon")
            Spacer(minLength: Spacing.sm)
            if position, let positionText {
                Text(positionText).font(Typography.text).foregroundStyle(palette.textSecondary).monospacedDigit()
                    .fixedSize().accessibilityIdentifier("backlog-position")
            }
            if arrows {
                Group {
                    Button { perform(.nudge(-1)) } label: { Image(systemName: "arrow.up") }
                        .help("Move up (⌘↑)").accessibilityLabel("Move up")
                    Button { perform(.nudge(1)) } label: { Image(systemName: "arrow.down") }
                        .help("Move down (⌘↓)").accessibilityLabel("Move down")
                }
                .disabled(!list.canReorder)
            }
        }
        .buttonStyle(LoopflowButtonStyle())
        .fixedSize(horizontal: false, vertical: true)
    }

    /// The narrowest bar: Start, Claim and the rest under "···".
    private var folded: some View {
        let tasks = list.selected
        return HStack(spacing: Spacing.sm) {
            selected
            BacklogStartButton(model: model, list: list, perform: perform)
            claim
            Spacer(minLength: 0)
            Menu {
                Button("Edit brief") { perform(.edit) }.disabled(list.opened == nil)
                Menu("Move") { BacklogMoveItems(model: model, list: list, tasks: tasks, perform: perform) }
                Button("Move up") { perform(.nudge(-1)) }.disabled(!list.canReorder)
                Button("Move down") { perform(.nudge(1)) }.disabled(!list.canReorder)
                Divider()
                Button("Abandon…", role: .destructive) { perform(.abandon) }
            } label: { Image(systemName: "ellipsis") }
                .menuStyle(.button).menuIndicator(.hidden).fixedSize().help("More actions")
                .accessibilityLabel("More actions")
                .accessibilityIdentifier("backlog-more")
        }
        .buttonStyle(LoopflowButtonStyle())
    }

    /// "8 of 46" for the one selected Task; several have no single place.
    private var positionText: String? {
        guard list.selected.count == 1, let task = list.selected.first,
              let index = list.all.firstIndex(where: { $0.id == task.id }) else { return nil }
        return "\(index + 1) of \(list.all.count)"
    }
}

/// Start, the bar's accent: the button runs each unstarted Task's own workflow
/// and the menu beside it lists the others. One started Task alone makes it Open.
struct BacklogStartButton: View {
    let model: WorkModel
    let list: BacklogList
    let perform: (BacklogView.Action) -> Void

    var body: some View {
        let tasks = list.selected
        let unavailable = BacklogList.startUnavailable(tasks)
        let fresh = tasks.filter(\.awaitsStart)
        HStack(spacing: Spacing.xxs) {
            Button(list.startLabel) { perform(.start(nil)) }
                .help(unavailable ?? BacklogList.startedNote(tasks) ?? (BacklogList.opens(tasks) != nil
                    ? "Open the Task to continue it (⌘↩)"
                    : fresh.count > 1 && BacklogList.sharedWorkflow(fresh) == nil ? "Each Task on its own workflow (⌘↩)" : "⌘↩"))
                .accessibilityIdentifier("backlog-start")
            // Only unstarted Tasks choose a workflow here.
            if tasks.isEmpty || !fresh.isEmpty {
                Menu {
                    BacklogStartItems(model: model, tasks: tasks, perform: perform)
                } label: {
                    Image(systemName: "chevron.down").font(Typography.meta)
                }
                .menuStyle(.button).menuIndicator(.hidden).fixedSize()
                .help("Start with another workflow")
                .accessibilityLabel("Start with another workflow")
                .accessibilityIdentifier("backlog-start-menu")
            }
        }
        .buttonStyle(LoopflowButtonStyle(.accent))
        .disabled(unavailable != nil)
        .fixedSize()
    }
}

/// Start's workflow menu for the unstarted Tasks among `tasks`. When their
/// workflows differ, the first item keeps each Task's own, and any workflow
/// below it runs all of them on that one.
struct BacklogStartItems: View {
    let model: WorkModel
    let tasks: [RoadmapTask]
    let perform: (BacklogView.Action) -> Void

    var body: some View {
        let fresh = tasks.filter(\.awaitsStart)
        let shared = BacklogList.sharedWorkflow(fresh)
        if shared == nil, fresh.count > 1 {
            Toggle(isOn: Binding(get: { true }, set: { _ in perform(.start(nil)) })) {
                Text("Each Task's own workflow")
                Text(BacklogList.breakdown(fresh))
            }
            Divider()
        }
        StartFlowItems(model: model, chosen: shared) { perform(.start($0)) }
    }
}

/// The workflows a Task can start on, as menu items; `chosen` is checked.
struct StartFlowItems: View {
    let model: WorkModel
    let chosen: String?
    let choose: (String) -> Void

    var body: some View {
        if let catalog = model.graphCatalog.value {
            let flows = StartFlows(catalog)
            ForEach(flows.stopping) { item($0) }
            if !flows.more.isEmpty {
                Menu("More") { ForEach(flows.more) { item($0) } }
            }
            Section("Run without stopping") {
                ForEach(flows.unattended) { item($0) }
            }
        } else {
            Text(model.graphCatalog.failure.map { "Workflows unavailable. \($0)" } ?? "Reading workflows…")
        }
    }

    private func item(_ flow: StartFlow) -> some View {
        Toggle(isOn: Binding(get: { flow.name == chosen }, set: { _ in choose(flow.name) })) {
            Text("\(flow.name), \(flow.consequence)")
            if let summary = flow.summary { Text(summary) }
        }
    }
}

struct BacklogMoveItems: View {
    let model: WorkModel
    let list: BacklogList
    let tasks: [RoadmapTask]
    let perform: (BacklogView.Action) -> Void

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
    let list: BacklogList
    let tasks: [RoadmapTask]
    /// Makes `tasks` the selection, then acts on it.
    let run: (BacklogView.Action) -> Void

    var body: some View {
        Button("Open") { if let task = tasks.first { model.openBacklog(task) } }.disabled(tasks.count != 1)
        if tasks.contains(where: \.awaitsStart) {
            Button(BacklogList.startLabel(tasks)) { run(.start(nil)) }.disabled(BacklogList.startUnavailable(tasks) != nil)
            Menu("Start with") { BacklogStartItems(model: model, tasks: tasks, perform: run) }
        }
        Button("Claim") { run(.claim) }.disabled(!tasks.contains { BacklogOwner($0) == .nobody })
        Button("Edit brief") { run(.edit) }.disabled(tasks.count != 1)
        Menu("Move") { BacklogMoveItems(model: model, list: list, tasks: tasks, perform: run) }
        Button("Move up") { run(.nudge(-1)) }.disabled(!list.canReorder)
        Button("Move down") { run(.nudge(1)) }.disabled(!list.canReorder)
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
    let perform: (BacklogView.Action) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        let brief = task.task.description
        let long = brief.count > 900
        let editing = state.wrappedValue.editing == task.id
        VStack(alignment: .leading, spacing: Spacing.md) {
            if editing {
                TextEditor(text: state.draft)
                    .font(Typography.text).scrollContentBackground(.hidden).frame(minHeight: 240)
                    .padding(Spacing.sm).background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
                    .overlay(RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border))
                    .accessibilityIdentifier("backlog-brief-editor")
                HStack(spacing: Spacing.sm) {
                    Button("Save") { perform(.save(task, state.wrappedValue.draft)) }
                        .buttonStyle(LoopflowButtonStyle(.accent))
                        .disabled(state.wrappedValue.draft == brief)
                        .keyboardShortcut("s", modifiers: .command)
                    Button("Cancel") { state.wrappedValue.editing = nil }.buttonStyle(LoopflowButtonStyle())
                        .keyboardShortcut(.cancelAction)
                }
            } else if brief.isEmpty {
                Text("No brief yet.").emptyLine()
            } else {
                Text(Self.markdown(brief)).font(Typography.text).lineSpacing(Spacing.xs).textSelection(.enabled)
                    .lineLimit(long && !state.wrappedValue.showsAll ? 9 : nil)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("backlog-brief-\(task.id)")
                if long {
                    Button(state.wrappedValue.showsAll ? "Show less" : "Show all \(Self.words(brief)) words") {
                        state.wrappedValue.showsAll.toggle()
                    }
                    .buttonStyle(WorkLinkStyle()).font(Typography.text).help("Return")
                }
            }
        }
        .frame(maxWidth: 720, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.bottom, Spacing.sm)
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
            Text(title).serif(.display).foregroundStyle(palette.emphasisInk).lineLimit(1)
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
