import Loopflow
import SwiftUI
import UniformTypeIdentifiers

struct DashboardEditor: View {
    let model: WorkModel
    let plan: WorkViewPlan
    @Environment(\.dismiss) private var dismiss
    @State private var id = UUID().uuidString
    @State private var name = ""
    @State private var expression = "state:available"
    @State private var pinned = true
    @State private var revision: Int64?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(plan.dashboardID == nil ? "Save Dashboard" : "Edit Dashboard").font(.headline)
            TextField("Name", text: $name)
            TextField("Query", text: $expression, axis: .vertical).font(.system(.body, design: .monospaced))
            Text("For example: (wave:product state:open), state:available or state:shipping")
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
            id = plan.dashboardID ?? id
            name = plan.name ?? id
            expression = plan.selection.queryText
            pinned = plan.pinned
            revision = plan.savedRevision
        }
    }
}

/// The board's query, edited in place. Return saves a Dashboard's query; on
/// Working Tasks or an unsaved view it opens the edited query as a view.
struct DashboardQueryField: View {
    let model: WorkModel
    let plan: WorkViewPlan
    @State private var draft = ""

    var body: some View {
        TextField("Query", text: $draft)
            .textFieldStyle(.plain).font(Typography.code(11))
            .disabled(model.dashboardSaving)
            .onSubmit(submit)
            .onChange(of: plan.selection.queryText, initial: true) { _, query in draft = query }
            .accessibilityIdentifier("dashboard-query")
    }

    private func submit() {
        let expression = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !expression.isEmpty, expression != plan.selection.queryText else { return }
        Task {
            if let id = plan.dashboardID {
                await model.saveDashboard(id: id, expression: expression, name: plan.name ?? id,
                                          pinned: plan.pinned, revision: plan.savedRevision)
            } else {
                await model.openView(expression: expression)
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
                    DashboardQueryField(model: model, plan: plan)
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
                                      ? DashboardSlotEditing { writeSlots(plan, $0) } : nil) { member in
                    DashboardTile(model: model, workspaces: workspaces, sessions: sessions, member: member, plan: plan,
                                  workingTasks: workingTasks, focused: workspaces.selectedWork == member.work)
                }
                if !plan.arrangement.hidden.isEmpty {
                    HStack {
                        Text("Hidden")
                        ForEach(plan.arrangement.hidden, id: \.self) { member in
                            Button {
                                model.setTile(plan, member, hidden: false)
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
            .sheet(isPresented: $editing) {
                if let plan = model.selectedViewPlan { DashboardEditor(model: model, plan: plan) }
            }
            .onChange(of: model.navigation.dashboardID) { _, _ in editingSlots = false }
    }

    private var workingTasks: Bool { model.navigation.destination == .workingTasks }

    private func writeSlots(_ plan: WorkViewPlan, _ slots: DashboardSlotLayout?) {
        guard let id = plan.dashboardID, let revision = plan.savedRevision else { return }
        Task { await model.setDashboardSlots(id, slots: slots, revision: revision) }
    }
}

/// A Wave view's footer: its Tasks as tiles filling the Wave plan's strip in the shared
/// order; the repository page's are its Working Tasks. `lf desktop move -p N` reorders
/// them. The page lives in navigation, so it follows an entered tile and survives back.
struct WaveTiles: View {
    let model: WorkModel
    let workspaces: SessionsWorkspaceRegistry
    let sessions: SessionsStore
    /// `.wave(id)` or `.repository`.
    let view: WorkNavigation.Board

    var body: some View {
        if let plan = model.boardPlan(view) {
            let members = model.boardTiles(view)
            if members.isEmpty {
                Text("No planned or started Tasks").font(Typography.body(15))
                    .foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
                    .accessibilityIdentifier("wave-tiles-empty")
            } else {
                DashboardSlotGrid(members: members, focused: nil, layout: plan.slots,
                                  heldPage: Binding(get: { model.navigation.wavePages[view] ?? 0 },
                                                    set: { model.navigation.wavePages[view] = $0 }),
                                  pagerKeys: false) { member in
                    DashboardTile(model: model, workspaces: workspaces, sessions: sessions, member: member, plan: plan,
                                  workingTasks: false, focused: false, takesFocus: false)
                }.padding(8).accessibilityIdentifier("wave-tiles")
            }
        }
    }
}

/// One Dashboard slot. A view of its own, with value inputs, so a change re-runs
/// the tiles it touches instead of every tile and its terminals.
struct DashboardTile: View {
    let model: WorkModel
    let workspaces: SessionsWorkspaceRegistry
    let sessions: SessionsStore
    let member: WorkMemberRef
    let plan: WorkViewPlan
    let workingTasks: Bool
    let focused: Bool
    /// A Wave view keeps the Wave selected, so its tiles never take the window's focus.
    var takesFocus = true
    @Environment(\.palette) private var palette
    @State private var frame = TileFrame()

    var body: some View {
        let content = plan.contents.first { $0.work == member.work }
        let entry = workspaces.retained(member.work)
        let task = model.task(id: member.work.id)
        // Only a saved Dashboard has an arrangement to write.
        let arrangeable = plan.dashboardID != nil
        return VStack(spacing: 0) {
            HStack(spacing: Spacing.sm) {
                Button { enter() } label: {
                    if let task { TaskHeading(task: task.task) }
                    else { Text(content?.title ?? member.work.id).lineLimit(1) }
                }.buttonStyle(.plain)
                Spacer(minLength: 0)
                if let task {
                    Menu { TaskActionItems(model: model, task: task.task, wave: task.wave?.wave) }
                        label: { Image(systemName: "ellipsis") }
                        .menuStyle(.button).buttonStyle(.plain).fixedSize().help("Task actions")
                }
                Button { enter() } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .help("Open this Work in full")
                if arrangeable {
                    Button { model.setTile(plan, member, hidden: true) } label: { Image(systemName: "eye.slash") }
                        .help("Hide this occurrence").disabled(model.dashboardSaving)
                }
            }.padding(8).background(palette.surface)
                .foregroundStyle(palette.text, palette.textSecondary)
                .draggable(workingTasks ? TaskOrderDrag.token(member.work.id) : member.token)
                .dropDestination(for: String.self) { values, _ in
                    guard let value = values.first else { return false }
                    if workingTasks, let task = TaskOrderDrag.taskID(value) {
                        model.reorderWorkingTask(task, to: member.work.id); return true
                    }
                    guard arrangeable, let id = plan.dashboardID, !model.dashboardSaving,
                          let other = plan.resolution.members.first(where: { $0.token == value }), other.work != member.work else { return false }
                    Task { await model.reorderDashboard(id, member: other.work, to: member.work) }; return true
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
                    WorkspacePanesView(workspace: entry, isFocused: focused, sessions: sessions, model: model,
                        onFocus: { pane, _ in
                            if takesFocus { workspaces.focus(entry.key) }
                            entry.multiplexer.setFocusedPane(pane)
                        }, face: pane)
                        .equatable()
                } else if let task = task?.task {
                    // Only an unstarted Task offers Start. A started one opens its conversation,
                    // which `lf resume --task` creates when only headless Runs exist.
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
                .strokeBorder(focused ? palette.borderActive : palette.border))
            .contentShape(Rectangle())
            .renderFlash()
            .simultaneousGesture(TapGesture().onEnded { if takesFocus { workspaces.focus(member.work) } })
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(ZoomFromTile.space)) } action: { frame.rect = $0 }
    }

    private func enter() {
        model.enterTile(member.work, origin: frame.rect)
    }
}

/// Where a tile sits, kept for the zoom transition. Not observed: the frame
/// moves on every focus-animation frame and no body reads it.
final class TileFrame {
    var rect: CGRect?
}

/// Grows an entered member's full Work out of its tile. A transform, so the
/// terminals inside lay out once at full size rather than on every frame.
struct ZoomFromTile: ViewModifier, Animatable {
    nonisolated static let space = "work-content"
    let origin: CGRect
    var progress: Double
    nonisolated var animatableData: Double {
        get { progress }
        set { progress = newValue }
    }

    func body(content: Content) -> some View {
        content.visualEffect { [origin, progress] effect, proxy in
            let full = proxy.frame(in: .named(Self.space))
            let rest = 1 - CGFloat(progress)
            let scaleX = full.width > 0 ? origin.width / full.width : 1
            let scaleY = full.height > 0 ? origin.height / full.height : 1
            return effect
                .scaleEffect(x: scaleX + (1 - scaleX) * (1 - rest), y: scaleY + (1 - scaleY) * (1 - rest), anchor: .topLeading)
                .offset(x: (origin.minX - full.minX) * rest, y: (origin.minY - full.minY) * rest)
        }
    }
}

extension AnyTransition {
    /// Inserted views grow out of `origin`; without one they simply appear.
    static func zoom(from origin: CGRect?) -> AnyTransition {
        guard let origin else { return .identity }
        return .asymmetric(
            insertion: .modifier(active: ZoomFromTile(origin: origin, progress: 0), identity: ZoomFromTile(origin: origin, progress: 1)),
            removal: .identity)
    }
}

extension WorkModel {
    /// Enter a tile as full Work from the view showing it; an unsaved query has
    /// no trail to return through, so its tile is plainly selected.
    func enterTile(_ key: WorkWorkspaceKey, origin: CGRect?) {
        let board: WorkNavigation.Board? = switch navigation.destination {
        case .dashboard(let id): .saved(id)
        case .workingTasks: .workingTasks
        case .work: navigation.waveView
        case .view, .entered: nil
        }
        guard let board, let work = key.reference else { return select(key.reference) }
        navigation.zoomOrigin = origin
        withAnimation(FocusSizing.animation) { enter(work, from: board) }
    }

    /// Hide or restore a tile. Rust decides the mode; hiding never touches the order.
    func setTile(_ plan: WorkViewPlan, _ member: WorkMemberRef, hidden: Bool) {
        guard let id = plan.dashboardID else {
            dashboardError = "Save this Dashboard before arranging its Work."; return
        }
        Task { await setDashboardTile(id, member: member.work, hidden: hidden) }
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
