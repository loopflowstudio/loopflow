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
        Button { model.selectWorkingTasksDashboard() } label: {
            Label(WorkNavigation.workingTasksTitle, systemImage: "square.grid.3x2")
                .frame(maxWidth: .infinity, alignment: .leading).padding(10)
                .background(model.navigation.destination == .workingTasks ? palette.surface : .clear,
                            in: RoundedRectangle(cornerRadius: 6))
        }.buttonStyle(.plain).accessibilityIdentifier("dashboard-working-tasks")
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
    @State private var editingSlots = false
    @Environment(\.palette) private var palette
    var body: some View {
        VStack(spacing: 8) {
            if let plan = model.selectedViewPlan {
                HStack {
                    if workingTasks {
                        Text("\(WorkNavigation.workingTasksTitle) · \(model.workingTaskMembers.count)")
                            .font(Typography.code(11))
                    } else {
                        Text(plan.selection.queryText).font(Typography.code(11)).textSelection(.enabled)
                    }
                    Spacer()
                    if !workingTasks {
                        if plan.dashboardID != nil {
                            if editingSlots {
                                Button("Default Slots") { writeSlots(plan, nil) }.disabled(model.dashboardSaving)
                            }
                            Button(editingSlots ? "Done" : "Edit Slots") { editingSlots.toggle() }
                                .accessibilityIdentifier("dashboard-edit-slots")
                        }
                        Button(plan.dashboardID == nil ? "Save Dashboard" : "Edit Dashboard") { editing = true }
                    }
                }.padding(10)
                if let error = model.dashboardError ?? model.selectionPlans.errorMessage {
                    Text(error).foregroundStyle(palette.warning).textSelection(.enabled)
                }
                // One line: membership may be incomplete; every reason stays in its help.
                if let first = plan.resolution.unavailable.first {
                    let more = plan.resolution.unavailable.count - 1
                    Text(more == 0 ? first : "\(first) (+\(more) more)").foregroundStyle(palette.warning).lineLimit(1)
                        .help(plan.resolution.unavailable.joined(separator: "\n"))
                        .accessibilityIdentifier("dashboard-unavailable")
                }
                // Working Tasks fill in the shared Task order; a saved Dashboard's
                // visible occurrences fill in its arrangement order.
                DashboardSlotGrid(members: workingTasks ? model.workingTaskMembers : plan.projection.visible,
                                  focused: workspaces.selectedWork, layout: plan.slots,
                                  editing: editingSlots && plan.dashboardID != nil && !model.dashboardSaving
                                      ? DashboardSlotEditing { writeSlots(plan, $0) } : nil) { tile($0, plan: plan) }
                if !plan.arrangement.hidden.isEmpty {
                    HStack {
                        Text("Hidden")
                        ForEach(plan.arrangement.hidden, id: \.self) { member in
                            Button {
                                arrange(plan, hidden: plan.arrangement.hidden.filter { $0 != member })
                            } label: {
                                Text(model.task(id: member.work.id)?.task.task.name
                                    ?? plan.contents.first { $0.work == member.work }?.title ?? member.work.id)
                            }.disabled(model.dashboardSaving).help("Show this occurrence again")
                        }
                    }.font(.caption).padding(8)
                }
            } else {
                ContentUnavailableView("Dashboard unavailable", systemImage: "exclamationmark.triangle",
                    description: Text(model.selectionPlans.errorMessage ?? "This Dashboard is no longer available."))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top).background(palette.surfaceMuted)
            .sheet(isPresented: $editing) { DashboardEditor(model: model, plan: model.selectedViewPlan) }
            .onChange(of: model.navigation.dashboardID) { _, _ in editingSlots = false }
    }

    private var workingTasks: Bool { model.navigation.destination == .workingTasks }

    private func writeSlots(_ plan: WorkViewPlan, _ slots: DashboardSlotLayout?) {
        guard let id = plan.dashboardID, let revision = plan.savedRevision else { return }
        Task { await model.setDashboardSlots(id, slots: slots, revision: revision) }
    }

    private func tile(_ member: WorkMemberRef, plan: WorkViewPlan) -> some View {
        let content = plan.contents.first { $0.work == member.work }
        let entry = workspaces.retained(member.work)
        let task = model.task(id: member.work.id)
        // Only a saved Dashboard has an arrangement to write.
        let arrangeable = plan.dashboardID != nil
        return VStack(spacing: 0) {
            HStack(spacing: Spacing.sm) {
                Button { zoom(member, plan: plan) } label: {
                    if let task { TaskHeading(task: task.task) }
                    else { Text(content?.title ?? member.work.id).lineLimit(1) }
                }.buttonStyle(.plain)
                Spacer(minLength: 0)
                if let task {
                    Menu { TaskActionItems(model: model, task: task.task, wave: task.wave?.wave) }
                        label: { Image(systemName: "ellipsis") }
                        .menuStyle(.button).buttonStyle(.plain).fixedSize().help("Task actions")
                }
                Button { zoom(member, plan: plan) } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .help("Open this Work in full")
                if arrangeable {
                    Button { arrange(plan, hidden: plan.arrangement.hidden + [member]) } label: { Image(systemName: "eye.slash") }
                        .help("Hide this occurrence").disabled(model.dashboardSaving)
                }
            }.padding(8).background(palette.surface)
                .foregroundStyle(palette.text, palette.textSecondary)
                .draggable(workingTasks ? TaskOrderDrag.token(member.work.id) : member.token)
                .dropDestination(for: String.self) { values, _ in
                    guard let value = values.first else { return false }
                    if workingTasks, let task = TaskOrderDrag.taskID(value) {
                        Task { await model.reorderWorkingTask(task, to: member.work.id) }; return true
                    }
                    guard arrangeable, let id = plan.dashboardID, let revision = plan.savedRevision, !model.dashboardSaving,
                          let other = plan.resolution.members.first(where: { $0.token == value }), other.work != member.work else { return false }
                    Task { await model.reorderDashboard(id, member: other.work, to: member.work, revision: revision) }; return true
                }
            if let entry, let content {
                if let error = entry.persistenceError { Text(error).foregroundStyle(palette.warning).textSelection(.enabled) }
                // The plan read in this frame decides; saved planning may still say otherwise.
                if let task, content.states.contains(.backlog) {
                    TaskStartOffer(model: model, task: task.task)
                } else if let pane = entry.multiplexer.zoomedPaneId.flatMap({ entry.multiplexer.layout.pane(for: $0) })
                    ?? content.primarySession.flatMap({ entry.multiplexer.pane(forSessionId: $0) })
                    // A conversation attached in this tile shows before the plan names it primary.
                    ?? entry.multiplexer.layout.allPanes.first(where: { $0.sessionId != nil }) {
                    WorkspacePanesView(workspace: entry, isFocused: workspaces.selectedWork == member.work, sessions: sessions, model: model,
                        onFocus: { pane, _ in
                            workspaces.focus(entry.key)
                            entry.multiplexer.setFocusedPane(pane)
                        }, face: pane)
                } else if let task = task?.task {
                    // Only an unstarted Task offers Start. A started one opens its conversation,
                    // which `lf session connect --task` creates when only headless Runs exist.
                    if task.awaitsStart {
                        TaskStartOffer(model: model, task: task)
                    } else {
                        DashboardConversationOffer(model: model, task: task, workspace: entry)
                    }
                } else {
                    PaneEmptyState(title: "No primary Session", systemImage: "bubble.left")
                }
            } else { PaneEmptyState(title: "Work unavailable", systemImage: "exclamationmark.triangle") }
        }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            // The slot grid clips each tile at its frame.
            .overlay(RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius)
                .strokeBorder(workspaces.selectedWork == member.work ? palette.borderActive : palette.border))
            .contentShape(Rectangle())
            .renderFlash()
            .simultaneousGesture(TapGesture().onEnded { workspaces.focus(member.work) })
    }

    private func zoom(_ member: WorkMemberRef, plan: WorkViewPlan) {
        guard let id = plan.dashboardID, let work = member.work.reference else { return model.select(member.work.reference) }
        model.zoom(into: work, from: id)
    }

    /// Hide or restore occurrences. Rust decides the mode; hiding never touches the order.
    private func arrange(_ plan: WorkViewPlan, hidden: [WorkMemberRef]) {
        guard let id = plan.dashboardID, let revision = plan.savedRevision else {
            model.dashboardError = "Save this Dashboard before arranging its Work."; return
        }
        Task { await model.arrangeDashboard(id, order: plan.arrangement.order, hidden: hidden, revision: revision) }
    }
}

/// A tile's Task with a recorded conversation and none attached here. Opening
/// attaches it in the tile; the zoom control is the way into the Task.
struct DashboardConversationOffer: View {
    let model: WorkModel
    let task: RoadmapTask
    let workspace: SessionsWorkspace

    var body: some View {
        if workspace.openingConversation {
            ProgressView("Opening conversation…").frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            PaneEmptyState(title: "No conversation open", systemImage: "bubble.left",
                           description: workspace.conversationError ?? "Open this Task’s conversation here.") {
                Button("Open conversation") { Task { await model.attachTaskConversation(task.id, in: workspace) } }
                    .accessibilityIdentifier("dashboard-open-conversation-\(task.id)")
            }
        }
    }
}

extension WorkMemberRef {
    fileprivate var token: String { String(decoding: (try? JSONEncoder().encode(self)) ?? Data(), as: UTF8.self) }
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
