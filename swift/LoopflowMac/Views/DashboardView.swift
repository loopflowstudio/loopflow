import Loopflow
import SwiftUI
import UniformTypeIdentifiers

struct DashboardSidebar: View {
    @Bindable var model: WorkModel
    @State private var creating = false
    @Environment(\.palette) private var palette
    var body: some View {
        HStack {
            Text("DASHBOARDS").font(Typography.code(10)).foregroundStyle(palette.textSecondary)
            Spacer()
            Button { creating = true } label: { Image(systemName: "plus") }
                .accessibilityLabel("New Dashboard")
        }.padding(10)
            .sheet(isPresented: $creating) { DashboardEditor(model: model, plan: nil) }
        ForEach(model.dashboards.filter(\.pinned), id: \.dashboardID) { plan in
            if let id = plan.dashboardID {
                Button { model.selectDashboard(id) } label: {
                    Label(plan.name ?? id, systemImage: "rectangle.split.2x2")
                        .frame(maxWidth: .infinity, alignment: .leading).padding(10)
                        .background(model.navigation.dashboardID == id ? palette.surface : .clear,
                                    in: RoundedRectangle(cornerRadius: 6))
                }.buttonStyle(.plain).accessibilityIdentifier("dashboard-" + id)
            }
        }
    }
}

struct DashboardEditor: View {
    let model: WorkModel
    let plan: WorkViewPlan?
    @Environment(\.dismiss) private var dismiss
    @State private var id = UUID().uuidString
    @State private var name = ""
    @State private var expression = "state:available"
    @State private var pinned = true
    @State private var revision: Int64?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(plan == nil ? "New Dashboard" : "Edit Dashboard").font(.headline)
            TextField("Name", text: $name)
            TextField("Query", text: $expression, axis: .vertical).font(.system(.body, design: .monospaced))
            Text("For example: (wave:product state:open) or state:available")
                .font(.caption).foregroundStyle(.secondary)
            Toggle("Pin in sidebar", isOn: $pinned)
            if let error = model.dashboardError { Text(error).foregroundStyle(.red).textSelection(.enabled) }
            HStack {
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer()
                Button("Save") {
                    Task {
                        await model.saveDashboard(id: id, expression: expression, name: name, pinned: pinned, revision: revision)
                        if model.dashboardError == nil { dismiss() }
                    }
                }.keyboardShortcut(.defaultAction).disabled(model.dashboardSaving || expression.isEmpty || name.isEmpty)
            }
        }.padding(24).frame(width: 540)
        .onAppear {
            if let plan {
                id = plan.dashboardID ?? id
                name = plan.name ?? id
                expression = plan.selection.queryText
                pinned = plan.pinned
                revision = plan.savedRevision
            }
        }
    }
}

struct DashboardView: View {
    let model: WorkModel
    let workspaces: SessionsWorkspaceRegistry
    let sessions: SessionsStore
    @State private var editing = false
    @Environment(\.palette) private var palette
    var body: some View {
        VStack(spacing: 8) {
            if let plan = model.selectedViewPlan {
                HStack {
                    Text(plan.selection.queryText).font(Typography.code(11)).textSelection(.enabled)
                    Spacer()
                    Button(plan.dashboardID == nil ? "Save Dashboard" : "Edit Dashboard") { editing = true }
                }.padding(10)
                if let error = model.dashboardError ?? model.selectionPlans.errorMessage {
                    Text(error).foregroundStyle(palette.waiting).textSelection(.enabled)
                }
                ForEach(plan.resolution.unavailable, id: \.self) { Text($0).foregroundStyle(palette.waiting) }
                if let tree = plan.projection.visible { node(tree, plan: plan) }
                else { ContentUnavailableView("No visible Work", systemImage: "rectangle.split.2x2") }
                if !plan.arrangement.hidden.isEmpty {
                    HStack {
                        Text("Hidden")
                        ForEach(plan.arrangement.hidden, id: \.self) { member in
                            Button(plan.contents.first { $0.work == member.work }?.title ?? member.work.id) {
                                save(plan, hidden: plan.arrangement.hidden.filter { $0 != member })
                            }.disabled(model.dashboardSaving)
                        }
                    }.font(.caption).padding(8)
                }
            } else {
                ContentUnavailableView("Dashboard unavailable", systemImage: "exclamationmark.triangle",
                    description: Text(model.selectionPlans.errorMessage ?? "This Dashboard is no longer available."))
            }
        }.padding(10).background(palette.surfaceMuted)
            .sheet(isPresented: $editing) { DashboardEditor(model: model, plan: model.selectedViewPlan) }
    }

    private func node(_ tree: WorkArrangementTree, plan: WorkViewPlan) -> AnyView {
        switch tree {
        case .member(let member):
            return AnyView(tile(member, plan: plan).id(member.work))
        case .split(let axis, let ratio, let first, let second):
            return AnyView(DashboardSplit(axis: axis, ratio: ratio,
                first: node(first, plan: plan), second: node(second, plan: plan), saving: model.dashboardSaving) { value in
                    let updated = plan.arrangement.tree?.resizing(first: first.firstMember, second: second.firstMember, ratio: value)
                    save(plan, tree: updated)
                })
        }
    }

    private func tile(_ member: WorkMemberRef, plan: WorkViewPlan) -> some View {
        let content = plan.contents.first { $0.work == member.work }
        let entry = workspaces.retained(member.work)
        return VStack(spacing: 0) {
            HStack {
                Button { zoom(member, plan: plan) } label: {
                    if let task = model.task(id: member.work.id)?.task { TaskHeading(task: task) }
                    else { Text(content?.title ?? member.work.id).lineLimit(1) }
                }.buttonStyle(.plain)
                Spacer()
                if let task = model.task(id: member.work.id) {
                    TaskLifecycleActions(model: model, task: task.task, wave: task.wave?.wave)
                        .menuStyle(.button).buttonStyle(.plain).fixedSize()
                }
                Button { zoom(member, plan: plan) } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .help("Open this Work in full")
                Button { save(plan, hidden: plan.arrangement.hidden + [member]) } label: { Image(systemName: "eye.slash") }
                    .help("Hide this occurrence").disabled(model.dashboardSaving)
            }.padding(8).background(palette.surface)
                .draggable(member.token)
                .dropDestination(for: String.self) { values, _ in
                    guard !model.dashboardSaving, let other = plan.resolution.members.first(where: { $0.token == values.first }), other != member else { return false }
                    save(plan, tree: plan.arrangement.tree?.swapping(member, other)); return true
                }
            if let entry, let content {
                if let error = entry.persistenceError { Text(error).foregroundStyle(palette.waiting).textSelection(.enabled) }
                if content.states.contains(.backlog), let task = model.task(id: member.work.id) {
                    ScrollView { Text(task.task.task.description).frame(maxWidth: .infinity, alignment: .leading).padding() }
                } else if let pane = entry.multiplexer.zoomedPaneId.flatMap({ entry.multiplexer.layout.pane(for: $0) })
                    ?? content.primarySession.flatMap({ entry.multiplexer.pane(forSessionId: $0) }) {
                    WorkspacePanesView(workspace: entry, isFocused: workspaces.selectedWork == member.work, sessions: sessions, model: model,
                        onFocus: { pane, _ in
                            workspaces.focus(entry.key)
                            entry.multiplexer.setFocusedPane(pane)
                        }, face: pane)
                } else {
                    Label("No primary Session", systemImage: "bubble.left")
                        .foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            } else { Text("Work unavailable").frame(maxWidth: .infinity, maxHeight: .infinity) }
        }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .clipShape(RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius))
            .overlay(RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius).strokeBorder(palette.border))
    }

    private func zoom(_ member: WorkMemberRef, plan: WorkViewPlan) {
        guard let id = plan.dashboardID, let work = member.work.reference else { return model.select(member.work.reference) }
        model.zoom(into: work, from: id)
    }

    private func save(_ plan: WorkViewPlan, tree: WorkArrangementTree? = nil, hidden: [WorkMemberRef]? = nil) {
        guard let id = plan.dashboardID, let revision = plan.savedRevision else {
            model.dashboardError = "Save this Dashboard before arranging its Work."; return
        }
        let arrangement = WorkArrangement(tree: tree ?? plan.arrangement.tree, hidden: hidden ?? plan.arrangement.hidden,
            mode: tree == nil ? plan.arrangement.mode : .touched)
        Task { await model.arrangeDashboard(id, arrangement: arrangement, revision: revision) }
    }
}

extension WorkMemberRef {
    fileprivate var token: String { String(decoding: (try? JSONEncoder().encode(self)) ?? Data(), as: UTF8.self) }
}
extension WorkArrangementTree {
    fileprivate var firstMember: WorkMemberRef {
        switch self { case .member(let ref): ref; case .split(_, _, let first, _): first.firstMember }
    }
    fileprivate func swapping(_ a: WorkMemberRef, _ b: WorkMemberRef) -> Self {
        switch self {
        case .member(let ref): return .member(ref == a ? b : ref == b ? a : ref)
        case .split(let axis, let ratio, let first, let second):
            return .split(axis: axis, ratio: ratio, first: first.swapping(a, b), second: second.swapping(a, b))
        }
    }
    private func contains(_ member: WorkMemberRef) -> Bool {
        switch self {
        case .member(let ref): ref == member
        case .split(_, _, let first, let second): first.contains(member) || second.contains(member)
        }
    }
    fileprivate func resizing(first a: WorkMemberRef, second b: WorkMemberRef, ratio: Double) -> Self {
        switch self {
        case .member: return self
        case .split(let axis, let old, let first, let second):
            return .split(axis: axis, ratio: first.contains(a) && second.contains(b) ? ratio : old,
                first: first.resizing(first: a, second: b, ratio: ratio), second: second.resizing(first: a, second: b, ratio: ratio))
        }
    }
}

extension WorkSelection {
    /// Display authored syntax; membership and validation stay in Rust.
    var queryText: String {
        func value(_ text: String) -> String { text.contains(where: { $0.isWhitespace || $0 == "(" || $0 == ")" }) ? "\"" + text + "\"" : text }
        switch expression {
        case .repository: return "state:open"
        case .wave(let id): return "wave:" + value(id)
        case .work(let work): return work.kind.rawValue + ":" + value(work.id)
        case .query(let id): return "query:" + value(id)
        case .compose(let items): return items.map { "(" + $0.queryText + ")" }.joined(separator: " or ")
        case .intersect(let items): return items.map { "(" + $0.queryText + ")" }.joined(separator: " ")
        case .filter(let filter):
            switch filter {
            case .wave(let name): return "wave:" + value(name)
            case .task(let name): return "task:" + value(name)
            case .kind(let kind): return "kind:" + kind.rawValue
            case .state(let state): return "state:" + state.rawValue
            }
        }
    }
}


private struct DashboardSplit: View {
    let axis: SplitAxis
    let ratio: Double
    let first: AnyView
    let second: AnyView
    let saving: Bool
    let commit: (Double) -> Void
    @State private var dragRatio: Double?
    var body: some View {
        GeometryReader { geometry in
            let vertical = axis == .vertical
            let extent = vertical ? geometry.size.width : geometry.size.height
            let sizing = SplitSizing(extent: extent, firstMinimum: 180, secondMinimum: 180)
            let displayed = sizing.constrain(dragRatio ?? ratio)
            let stack = vertical ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
            stack {
                first.frame(width: vertical ? max(0, extent * displayed - sizing.divider / 2) : nil,
                            height: vertical ? nil : max(0, extent * displayed - sizing.divider / 2))
                SplitDivider(axis: axis, ratio: displayed, sizing: sizing, update: { dragRatio = $0 },
                    commit: { value in commit(value); dragRatio = nil }).disabled(saving)
                second
            }
        }
    }
}
