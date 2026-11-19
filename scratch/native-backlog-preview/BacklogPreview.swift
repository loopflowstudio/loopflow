import AppKit
import SwiftUI
import Observation

struct PreviewTask: Codable, Identifiable {
    let id: String
    let identifier: String
    let title: String
    var brief: String
    let rank: Int
}

/// A Graph as Start offers it: its name, the menu line drafted from its header comment,
/// how many steps wait for a person, and whether it merges on its own.
struct PreviewFlow: Identifiable {
    let name: String
    let summary: String
    let stops: Int
    let merges: Bool
    var id: String { name }
    var consequence: String {
        merges ? "merges without asking" : stops == 0 ? "never stops for you" : stops == 1 ? "1 stop" : "\(stops) stops"
    }
}

let previewFlows = [
    PreviewFlow(name: "feature", summary: "You choose what to build, try it, then check it on main", stops: 3, merges: false),
    PreviewFlow(name: "code", summary: "Builds it; you try it, then check it on main", stops: 2, merges: false),
    PreviewFlow(name: "research", summary: "Findings, no PR", stops: 1, merges: false),
    PreviewFlow(name: "auto", summary: "Builds and merges without a person", stops: 0, merges: true),
    PreviewFlow(name: "incident", summary: "Restore first, then fix the cause", stops: 0, merges: false),
]

/// Each Wave's `graph:` from `.lf/wave/*/config.yaml`.
let waveFlows = ["Product": "feature", "Growth": "feature", "Infrastructure": "code"]

@Observable @MainActor
final class PreviewLibrary {
    var tasks: [PreviewTask]
    var picked: Set<String> = []
    var anchor: String?
    /// The row the keyboard last moved to; arrows step from here.
    var cursor: String?
    /// Whether the opened brief shows past its first lines.
    var showsAll = false
    /// Illustrative: the snapshot has no owners. Three Tasks start as someone else's so Available has something to hide.
    var owners: [String: String] = ["LOO-423": "someone else", "LOO-567": "someone else", "LOO-753": "someone else"]
    /// Illustrative: the snapshot is all Product. Two Tasks sit in other Waves so a selection can mix flows.
    var waves: [String: String] = ["LOO-753": "Infrastructure", "LOO-423": "Growth"]
    var filter = "All"
    var hovered: String?
    var scope = "Product"
    var query = ""
    var destination = "Library"
    var drafts: [String: String] = [:]
    var editing: String?
    var reply = ""
    var newIdea = ""
    var capturing = false
    var notice: String?
    var dark = false
    var showsObjective = false
    var dragTarget: String?
    /// The Wave view's parts, as `WaveViewLayout` holds them: the conversation's share,
    /// whether it is hidden, and which part fills the view.
    var sessionRatio = backlogSessionShare
    var sessionHidden = false
    var zoomed: String?

    init() {
        let url = Bundle.main.url(forResource: "backlog", withExtension: "json")!
        tasks = try! JSONDecoder().decode([PreviewTask].self, from: Data(contentsOf: url))
        _ = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            MainActor.assumeIsolated { self?.handle(event) ?? false } ? nil : event
        }
    }

    var hasSnapshot: Bool { scope == "loopflow" || scope == "Product" }

    func passes(_ task: PreviewTask, _ filter: String) -> Bool {
        let owner = owners[task.identifier]
        return filter == "All" || (filter == "Mine" && owner == "you") || (filter == "Available" && owner == nil)
    }

    func count(_ filter: String) -> Int { hasSnapshot ? tasks.filter { passes($0, filter) }.count : 0 }

    var visible: [PreviewTask] {
        guard hasSnapshot else { return [] }
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return tasks.filter { task in
            passes(task, filter) && (needle.isEmpty || "\(task.identifier) \(task.title) \(task.brief)".localizedCaseInsensitiveContains(needle))
        }
    }

    // Undo covers the cheap changes: order, claims, brief edits. Start and Abandon stay preview-then-confirm.
    private var history: [(label: String, tasks: [PreviewTask], owners: [String: String])] = []
    var undoLabel: String? { history.last?.label }

    private func record(_ label: String) { history.append((label, tasks, owners)) }

    func undo() {
        guard let last = history.popLast() else { return }
        tasks = last.tasks
        owners = last.owners
        notice = "Undid \(last.label)"
    }

    /// Moves Tasks, kept in their current order, to sit before `target`, or to the end without one.
    func move(_ ids: [String], before target: String?) {
        let moving = tasks.filter { ids.contains($0.identifier) }
        guard !moving.isEmpty, target.map({ !ids.contains($0) }) ?? true else { return }
        var next = tasks.filter { !ids.contains($0.identifier) }
        let index = target.flatMap { id in next.firstIndex { $0.identifier == id } } ?? next.count
        next.insert(contentsOf: moving, at: index)
        guard next.map(\.identifier) != tasks.map(\.identifier) else { return }
        let what = moving.count == 1 ? moving[0].identifier : "\(moving.count) Tasks"
        record("move of \(what)")
        tasks = next
        notice = "Preview only · moved \(what)"
    }

    func moveToTop(_ ids: [String]) {
        move(ids, before: tasks.first { !ids.contains($0.identifier) }?.identifier)
    }

    /// Moves the row, or the selection it belongs to, one place among the visible rows.
    func nudge(_ id: String, by offset: Int) {
        let ids = targets(id)
        let rows = visible.map(\.identifier)
        let places = rows.indices.filter { ids.contains(rows[$0]) }
        guard let first = places.first, let last = places.last else { return }
        if offset < 0 {
            if first > 0 { move(ids, before: rows[first - 1]) }
        } else if last + 1 < rows.count {
            move(ids, before: last + 2 < rows.count ? rows[last + 2] : nil)
        }
    }

    /// The one selected Task shows its brief. With several selected, none does.
    var opened: String? { picked.count == 1 ? picked.first : nil }

    private func settle() {
        editing = nil
        showsAll = false
    }

    func select(_ id: String) {
        picked = picked == [id] ? [] : [id]
        anchor = id
        cursor = id
        notice = nil
        settle()
    }

    func togglePick(_ id: String) {
        if picked.contains(id) { picked.remove(id) } else { picked.insert(id) }
        anchor = id
        cursor = id
        settle()
    }

    /// Selects from the anchor through this row, as Shift-click and Shift-arrow do in a Mac list.
    func extend(to id: String) {
        let rows = visible.map(\.identifier)
        let start = anchor ?? id
        guard let from = rows.firstIndex(of: start), let to = rows.firstIndex(of: id) else { return }
        picked = Set(rows[min(from, to)...max(from, to)])
        anchor = start
        cursor = id
        settle()
    }

    func step(_ offset: Int, extending: Bool) {
        let rows = visible.map(\.identifier)
        guard !rows.isEmpty else { return }
        let from = cursor.flatMap(rows.firstIndex(of:))
        let next = rows[from.map { min(max($0 + offset, 0), rows.count - 1) } ?? (offset > 0 ? 0 : rows.count - 1)]
        if extending {
            extend(to: next)
        } else {
            picked = [next]
            anchor = next
            cursor = next
            settle()
        }
    }

    /// The Tasks an action on this row applies to: the selection when the row is in it, else the row alone.
    func targets(_ id: String) -> [String] {
        picked.contains(id) ? selection : [id]
    }

    var selection: [String] { tasks.map(\.identifier).filter(picked.contains) }

    func claim(_ ids: [String]) {
        let free = ids.filter { owners[$0] == nil }
        if !free.isEmpty { record(free.count == 1 ? "claim of \(free[0])" : "claim of \(free.count) Tasks") }
        free.forEach { owners[$0] = "you" }
        let skipped = ids.count - free.count
        notice = "Preview only · claimed \(free.count), nothing started" + (skipped > 0 ? " · \(skipped) already claimed" : "")
    }

    func saveDraft(_ id: String) {
        if let index = tasks.firstIndex(where: { $0.identifier == id }), let draft = drafts[id] {
            record("edit of \(id)")
            tasks[index].brief = draft
        }
        drafts[id] = nil
        editing = nil
        notice = "Brief saved in preview only"
    }

    func preview(_ action: String, _ ids: [String]) {
        notice = "Preview only · \(action): \(ids.count == 1 ? ids[0] : "\(ids.count) Tasks")"
    }

    func wave(_ id: String) -> String { waves[id] ?? "Product" }
    func defaultFlow(_ id: String) -> String { waveFlows[wave(id)] ?? "code" }

    /// The one flow every selected Task would run by default, or nil when their Waves differ.
    func sharedFlow(_ ids: [String]) -> String? {
        let names = Set(ids.map(defaultFlow))
        return names.count == 1 ? names.first : nil
    }

    /// "feature 2 · code 1", in flow order.
    func breakdown(_ ids: [String]) -> String {
        previewFlows.compactMap { flow in
            let n = ids.filter { defaultFlow($0) == flow.name }.count
            return n == 0 ? nil : "\(flow.name) \(n)"
        }.joined(separator: " · ")
    }

    /// What Start's button names: the shared flow, or the breakdown when Waves differ.
    func startLabel(_ ids: [String]) -> String { ids.isEmpty ? "feature" : sharedFlow(ids) ?? breakdown(ids) }

    /// Starts each Task with its Wave's flow, or every one with `flow`.
    func start(_ ids: [String], flow: String? = nil) {
        let how = flow.map { "run with \($0)" } ?? sharedFlow(ids).map { "run with \($0)" } ?? "run with its Wave's flow (\(breakdown(ids)))"
        preview("each would be claimed and \(how)", ids)
    }
    func abandon(_ ids: [String]) { preview("one reason asked, recorded on each; reopenable", ids) }

    /// Keys for the list. Typing in a brief or the capture sheet keeps every key; the search field gives up the arrows.
    func handle(_ event: NSEvent) -> Bool {
        guard destination == "Library", !capturing, editing == nil else { return false }
        let typing = event.window?.firstResponder is NSTextView
        let flags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        let plain = flags.isEmpty, shift = flags == .shift, command = flags == .command
        switch event.keyCode {
        case 125, 126:
            let offset = event.keyCode == 125 ? 1 : -1
            if command, !typing, let id = selection.first { nudge(id, by: offset) }
            else if plain || shift {
                if typing { event.window?.makeFirstResponder(nil) }
                step(offset, extending: shift)
            } else { return false }
        case 53 where plain && !picked.isEmpty && !typing:
            picked = []
            settle()
        case 53 where plain && zoomed != nil && !typing: zoomed = nil
        case 36 where plain && !typing && opened != nil: showsAll.toggle()
        case 36 where command && !typing && !picked.isEmpty: start(selection)
        case 8 where plain && !typing && !picked.isEmpty: claim(selection)
        case 14 where plain && !typing && opened != nil: editing = opened
        case 51 where command && !typing && !picked.isEmpty: abandon(selection)
        case 6 where command && !typing && undoLabel != nil: undo()
        case 0 where command && !typing:
            picked = Set(visible.map(\.identifier))
            settle()
        default: return false
        }
        return true
    }

    func openScope(_ scope: String) {
        self.scope = scope
        destination = "Library"
    }
}

struct BacklogPreview: View {
    @State private var model = PreviewLibrary()
    private var palette: LoopflowPalette { model.dark ? .dark : .light }

    var body: some View {
        HSplitView {
            sidebar.frame(minWidth: 215, idealWidth: 240, maxWidth: 300)
            VStack(spacing: 0) {
                if model.destination == "Library" { waveView } else { conversation }
                Divider()
                HStack {
                    Text("Design preview · planning snapshot · edits stay here")
                    Spacer()
                    if let notice = model.notice { Text(notice).lineLimit(1) }
                    else { Text("↑↓ select · ⇧ extend · ⌘↑↓ reorder · Return show all · ⌘Return start · ⌘Z undo · Esc clear").lineLimit(1) }
                    if let label = model.undoLabel {
                        Button("Undo \(label)") { model.undo() }.buttonStyle(.link).help("⌘Z")
                    }
                }
                .font(Typography.body(11)).foregroundStyle(palette.textSecondary)
                .padding(.horizontal, 20).padding(.vertical, 9)
                .background(palette.surfaceMuted)
            }
        }
        .font(Typography.body(14))
        .foregroundStyle(palette.text).background(palette.background)
        .frame(minWidth: 1060, minHeight: 680)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                HStack(spacing: 12) {
                    Image(systemName: "sidebar.left")
                    Button("loopflow") { model.openScope("loopflow") }
                    Image(systemName: "chevron.right").font(.system(size: 10, weight: .bold))
                    Text(model.destination == "Library" ? model.scope : model.destination)
                    if model.destination == "Library" {
                        Image(systemName: "chevron.right").font(.system(size: 10, weight: .bold))
                        Text("Backlog").foregroundStyle(palette.titlebarSecondaryText)
                    }
                }.buttonStyle(.plain).font(Typography.strong(14)).foregroundStyle(palette.titlebarText)
            }
            ToolbarItem(placement: .primaryAction) {
                Button { model.dark.toggle() } label: { Image(systemName: model.dark ? "sun.max" : "moon") }
                    .help("Preview light or dark appearance")
            }
        }
        .toolbarBackground(palette.titlebarBackground, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .toolbarColorScheme(palette.titlebarColorScheme, for: .windowToolbar)
        .sheet(isPresented: $model.capturing) { captureSheet }
        .environment(\.palette, model.dark ? .dark : .light)
        .preferredColorScheme(model.dark ? .dark : .light)
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            Button { model.openScope("loopflow") } label: {
                HStack { Text("loopflow"); Spacer(); Text("3").font(Typography.code(11)) }
                    .padding(12).background(palette.surface, in: RoundedRectangle(cornerRadius: 7))
            }.buttonStyle(.plain).padding(8)
            Button { model.capturing = true } label: {
                HStack { Image(systemName: "plus"); Text("New idea"); Spacer(); Text("⌘N").foregroundStyle(palette.textSecondary) }
                    .padding(.horizontal, 17).padding(.vertical, 13)
            }.buttonStyle(.plain).keyboardShortcut("n")
            ScrollView {
                VStack(spacing: 7) {
                    workRow("LOO-903", "Redesign backlog management so choosing and shaping the next work feels clear", "Backlog exploration")
                    workRow("LOO-515", "Make Task PR, file-browser and brief views useful beside the conversation", "Task workspace")
                    workRow("LOO-434", "Make repeatable delivery approachable from the Task workspace", "Delivery exploration")
                }.padding(8)
            }
            Divider().padding(.horizontal, 15)
            VStack(spacing: 4) {
                ForEach(["Engbot", "Growth", "Infrastructure", "Intelligence", "List", "Product", "Spend"], id: \.self) { wave in
                    Button { model.openScope(wave) } label: {
                        HStack {
                            Image(systemName: "water.waves").font(.system(size: 12))
                            Text(wave)
                            Spacer()
                            if wave == "Product" { Text("\(model.tasks.count)").font(Typography.code(10)) }
                        }
                        .padding(10)
                        .background(model.destination == "Library" && model.scope == wave ? palette.workSelectedFill : .clear,
                                    in: RoundedRectangle(cornerRadius: 6))
                        .foregroundStyle(model.destination == "Library" && model.scope == wave ? palette.workSelectedText : palette.text)
                    }.buttonStyle(.plain)
                }
            }.padding(8)
        }.background(palette.surfaceMuted)
    }

    private func workRow(_ id: String, _ title: String, _ context: String) -> some View {
        Button { model.destination = id; model.notice = nil } label: {
            VStack(alignment: .leading, spacing: 7) {
                Text(title).font(Typography.strong(13)).lineLimit(2)
                Text(context).font(Typography.code(11)).foregroundStyle(palette.textSecondary).lineLimit(1)
                Text(id).font(Typography.code(10)).foregroundStyle(palette.textSecondary)
            }
            .padding(12).frame(maxWidth: .infinity, alignment: .leading)
            .background(model.destination == id ? palette.workSelectedFill : .clear, in: RoundedRectangle(cornerRadius: 7))
        }.buttonStyle(.plain)
    }

    private var library: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text(model.scope).font(Typography.strong(17))
                if model.scope == "Product" {
                    Text("Chapter demo-20260924").font(Typography.body(12)).foregroundStyle(palette.textSecondary)
                    Button { model.showsObjective.toggle() } label: {
                        Image(systemName: "chevron.down").rotationEffect(.degrees(model.showsObjective ? 180 : 0))
                    }.buttonStyle(.borderless).help("Show objective")
                }
                Spacer()
                if model.sessionHidden {
                    Button { model.sessionHidden = false } label: { Label("Show conversation", systemImage: "text.bubble") }.buttonStyle(.borderless)
                }
                Button { model.capturing = true } label: { Image(systemName: "plus") }.buttonStyle(.borderless).help("Save an idea")
            }.padding(.horizontal, 22).padding(.top, 12)
            HStack(spacing: 12) {
                Picker("Show", selection: $model.filter) {
                    ForEach(["All", "Mine", "Available"], id: \.self) { Text("\($0) \(model.count($0))") }
                }.pickerStyle(.segmented).labelsHidden().fixedSize()
                Text("unstarted").font(Typography.body(12)).foregroundStyle(palette.textSecondary)
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass").foregroundStyle(palette.textSecondary)
                    TextField("Find work by title or brief", text: $model.query).textFieldStyle(.plain)
                    if !model.query.isEmpty { Button { model.query = "" } label: { Image(systemName: "xmark.circle.fill") }.buttonStyle(.plain) }
                }.padding(.horizontal, 9).padding(.vertical, 6).frame(maxWidth: 320).background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: 6))
            }.padding(.horizontal, 22).padding(.top, 8).padding(.bottom, 12)
            if model.showsObjective {
                Text("Make selected work clear, useful, and easy to carry forward.")
                    .font(Typography.body(13)).foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, 22).padding(.bottom, 12)
            }
            Divider()
            selectionToolbar
            Divider()
            collection
        }
    }

    /// The actions for whatever is selected, one Task or many; disabled with nothing selected.
    private var selectionToolbar: some View {
        let ids = model.selection
        let none = ids.isEmpty
        let count = ids.count > 1 ? " \(ids.count)" : ""
        return HStack(spacing: 8) {
            Menu { startItems(ids) } label: {
                Text("Start\(count) · \(model.startLabel(ids))")
            } primaryAction: { model.start(ids) }
                .menuStyle(.button).buttonStyle(.borderedProminent).tint(palette.accent).fixedSize()
                .help(none || model.sharedFlow(ids) != nil ? "⌘↩" : "Each Task with its Wave's flow: \(model.breakdown(ids)) (⌘↩)")
            Button("Claim\(count)") { model.claim(ids) }.help("C")
            Button(model.opened.flatMap { model.drafts[$0] } == nil ? "Edit" : "Resume draft") { model.editing = model.opened }
                .disabled(model.opened == nil).help("E")
            Menu("Move") { moveItems(ids) }.fixedSize()
            Spacer()
            if ids.count > 1 { Text("\(ids.count) selected").font(Typography.body(12)).foregroundStyle(palette.textSecondary) }
            else if let id = model.opened, let index = model.tasks.firstIndex(where: { $0.identifier == id }) {
                Text("\(index + 1) of \(model.tasks.count)").font(Typography.body(12)).foregroundStyle(palette.textSecondary)
            }
            Button { if let id = ids.first { model.nudge(id, by: -1) } } label: { Image(systemName: "arrow.up") }.buttonStyle(.borderless).help("Move up (⌘↑)")
            Button { if let id = ids.first { model.nudge(id, by: 1) } } label: { Image(systemName: "arrow.down") }.buttonStyle(.borderless).help("Move down (⌘↓)")
            Button("Abandon\(count)…") { model.abandon(ids) }.buttonStyle(.borderless).help("⌘⌫")
        }
        .disabled(none)
        .controlSize(.small)
        .padding(.horizontal, 22).padding(.vertical, 7)
        .background(palette.surfaceMuted)
    }

    /// Start's flow menu. With Waves that differ, the first item keeps each Task's own flow
    /// and every flow below overrides all of them.
    @ViewBuilder private func startItems(_ ids: [String]) -> some View {
        let shared = model.sharedFlow(ids)
        if shared == nil {
            Toggle(isOn: Binding(get: { true }, set: { _ in model.start(ids) })) {
                Text("Each Wave's flow")
                Text(ids.map { "\($0) \(model.defaultFlow($0))" }.joined(separator: ", "))
            }
            Divider()
        }
        Section(shared == nil ? "Start all \(ids.count) with" : "") {
            ForEach(previewFlows.filter { $0.stops > 0 }) { flowItem($0, ids, shared) }
        }
        Section("Run without stopping") {
            ForEach(previewFlows.filter { $0.stops == 0 }) { flowItem($0, ids, shared) }
        }
        Divider()
        Button(shared == nil ? "Set each Wave's default…" : "Make code the default for \(ids.first.map(model.wave) ?? "Product")…") {
            model.preview("would open the Wave's settings", ids)
        }
    }

    private func flowItem(_ flow: PreviewFlow, _ ids: [String], _ shared: String?) -> some View {
        Toggle(isOn: Binding(get: { flow.name == shared }, set: { _ in model.start(ids, flow: flow.name) })) {
            Text("\(flow.name)  ·  \(flow.consequence)")
            Text(flow.summary)
        }
    }

    @ViewBuilder private func moveItems(_ ids: [String]) -> some View {
        Menu("To Wave") {
            ForEach(["Engbot", "Growth", "Infrastructure", "Intelligence", "List", "Spend"], id: \.self) { wave in
                Button(wave) { model.preview("would move to \(wave)", ids) }
            }
        }
        Button("To top") { model.moveToTop(ids) }
    }

    /// The Wave view: its conversation beside the backlog, as `WaveViewLayout` places them,
    /// with the backlog given the larger share. Hide and zoom work as they do there.
    @ViewBuilder private var waveView: some View {
        if let zoomed = model.zoomed {
            part(zoomed)
        } else if model.sessionHidden {
            part("Backlog")
        } else {
            PreviewSplit(ratio: model.sessionRatio, update: { model.sessionRatio = $0 }) {
                part("Conversation")
            } second: {
                part("Backlog")
            }
        }
    }

    private func part(_ name: String) -> some View {
        let zoomed = model.zoomed == name
        return Group {
            if name == "Backlog" { library } else { waveConversation }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .overlay(alignment: .topTrailing) {
            if model.hovered == "part-\(name)" {
                HStack(spacing: 2) {
                    Button { model.zoomed = zoomed ? nil : name } label: {
                        Image(systemName: zoomed ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right").frame(width: 22, height: 20)
                    }.help(zoomed ? "Return to the view (Esc)" : "Fill the view with the \(name.lowercased())")
                    if name == "Conversation", !zoomed {
                        Button { model.sessionHidden = true } label: { Image(systemName: "eye.slash").frame(width: 22, height: 20) }
                            .help("Hide the conversation")
                    }
                }.buttonStyle(.plain).font(.system(size: 11)).foregroundStyle(palette.textSecondary).padding(4)
            }
        }
        .onHover { inside in
            if inside { model.hovered = "part-\(name)" } else if model.hovered == "part-\(name)" { model.hovered = nil }
        }
    }

    /// Stands in for the Wave's live conversation pane.
    private var waveConversation: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Product conversation").font(Typography.strong(14))
            Divider()
            Text("› what should we pick up next?").font(Typography.code(12))
            Text("LOO-903 is the only Task here with a design ready. LOO-515 and LOO-434 both touch the Task workspace; reading them together before starting either would keep them from colliding.")
                .font(Typography.code(12)).foregroundStyle(palette.textSecondary)
            Spacer()
            Text("Live provider not connected").font(Typography.code(11)).foregroundStyle(palette.textTertiary)
            TextField("Ask the Wave", text: $model.reply, axis: .vertical).lineLimit(1...4).textFieldStyle(.roundedBorder)
        }
        .padding(16)
        .background(palette.surface)
    }

    /// The row menu: the toolbar's actions plus the ones too small for it, reached from "···" and right-click.
    @ViewBuilder private func actions(_ ids: [String]) -> some View {
        Button("Start · \(model.startLabel(ids))") { model.start(ids) }
        Menu("Start with") { startItems(ids) }
        Button("Claim") { model.claim(ids) }
        if ids.count == 1 { Button("Edit brief") { model.select(ids[0]); model.editing = ids[0] } }
        Divider()
        moveItems(ids)
        Button("Move up") { model.nudge(ids[0], by: -1) }
        Button("Move down") { model.nudge(ids[0], by: 1) }
        Divider()
        Button(ids.count == 1 ? "Copy ID" : "Copy \(ids.count) IDs") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(ids.joined(separator: " "), forType: .string)
        }
        Divider()
        Button("Abandon…", role: .destructive) { model.abandon(ids) }
    }

    private var collection: some View {
        ScrollViewReader { proxy in
        ScrollView {
            LazyVStack(spacing: 0) {
                if model.visible.isEmpty { emptyState }
                ForEach(model.visible) { task in
                    VStack(spacing: 0) {
                        row(task)
                        if model.opened == task.identifier { opened(task) }
                    }.id(task.identifier)
                    Divider().padding(.leading, 22)
                }
                Color.clear.frame(height: 20)
                    .contentShape(Rectangle())
                    .dropDestination(for: String.self) { items, _ in
                        guard let source = items.first, model.tasks.contains(where: { $0.identifier == source }) else { return false }
                        model.move(model.targets(source), before: nil)
                        return true
                    }
            }
        }
        .background(palette.background)
        .onChange(of: model.cursor) { _, row in if let row { proxy.scrollTo(row) } }
        }
    }

    /// Every empty list says why it is empty and offers the one step that fills it.
    private var emptyState: some View {
        let searching = !model.query.trimmingCharacters(in: .whitespaces).isEmpty
        let otherHits = searching ? model.tasks.filter { model.passes($0, "All") && "\($0.identifier) \($0.title) \($0.brief)".localizedCaseInsensitiveContains(model.query) }.count : 0
        return VStack(alignment: .leading, spacing: 12) {
            if !model.hasSnapshot {
                Text("No snapshot loaded for \(model.scope)").font(Typography.strong())
                Text("This preview carries the Product backlog only.").foregroundStyle(palette.textSecondary)
                Button("Open Product") { model.openScope("Product") }
            } else if searching {
                Text("Nothing in \(model.filter) matches “\(model.query)”").font(Typography.strong())
                HStack {
                    Button("Clear search") { model.query = "" }
                    if model.filter != "All" && otherHits > 0 { Button("Search all \(model.count("All")) (\(otherHits) match)") { model.filter = "All" } }
                }
            } else if model.filter == "Mine" {
                Text("You haven't claimed anything").font(Typography.strong())
                Text("Claiming keeps a Task yours without starting an agent. \(model.count("Available")) are available.")
                    .foregroundStyle(palette.textSecondary)
                Button("Show available") { model.filter = "Available" }.buttonStyle(.borderedProminent).tint(palette.accent)
            } else {
                Text(model.filter == "Available" ? "Everything here is claimed" : "No unstarted work in \(model.scope)").font(Typography.strong())
                Button("Save an idea") { model.capturing = true }
            }
        }.padding(24).frame(maxWidth: .infinity, alignment: .leading)
    }

    private func row(_ task: PreviewTask) -> some View {
        let isOpen = model.opened == task.identifier
        return HStack(alignment: .firstTextBaseline, spacing: 10) {
            Toggle("Select \(task.identifier)", isOn: Binding(
                get: { model.picked.contains(task.identifier) },
                set: { _ in model.togglePick(task.identifier) }
            )).toggleStyle(.checkbox).labelsHidden()
                .opacity(model.hovered == task.identifier || !model.picked.isEmpty ? 1 : 0)
            Menu { actions(model.targets(task.identifier)) } label: { Image(systemName: "ellipsis") }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .foregroundStyle(palette.textTertiary).help("Actions")
            Button {
                let keys = NSEvent.modifierFlags
                NSApp.keyWindow?.makeFirstResponder(nil)
                if keys.contains(.command) { model.togglePick(task.identifier) }
                else if keys.contains(.shift) { model.extend(to: task.identifier) }
                else { model.select(task.identifier) }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Image(systemName: "chevron.right").font(.system(size: 10, weight: .semibold))
                        .rotationEffect(.degrees(isOpen ? 90 : 0)).foregroundStyle(palette.textTertiary).frame(width: 12)
                    Text(task.identifier).font(Typography.code(11)).foregroundStyle(palette.textSecondary).frame(width: 62, alignment: .leading)
                    Text(task.title).font(isOpen ? Typography.strong(14) : Typography.body(14))
                        .fixedSize(horizontal: false, vertical: true).frame(maxWidth: .infinity, alignment: .leading)
                    if model.drafts[task.identifier] != nil { Text("draft").font(Typography.body(11)).foregroundStyle(palette.accent) }
                    if model.wave(task.identifier) != model.scope {
                        Text(model.wave(task.identifier)).font(Typography.body(11)).foregroundStyle(palette.textSecondary)
                    }
                    owner(task.identifier)
                }
                .padding(.vertical, 10).padding(.trailing, 22)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityLabel("\(isOpen ? "Close" : "Open") \(task.identifier): \(task.title)")
        }
        .padding(.leading, 22)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isOpen ? palette.surfaceMuted : model.picked.contains(task.identifier) ? palette.selectionFill : .clear)
        .onHover { inside in
            if inside { model.hovered = task.identifier } else if model.hovered == task.identifier { model.hovered = nil }
        }
        .draggable(task.identifier) {
            let count = model.targets(task.identifier).count
            Text(count > 1 ? "\(count) Tasks" : task.title).font(Typography.strong(13)).lineLimit(1)
                .padding(.horizontal, 12).padding(.vertical, 8)
                .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
        }
        .dropDestination(for: String.self) { items, _ in
            guard let source = items.first, model.tasks.contains(where: { $0.identifier == source }) else { return false }
            model.move(model.targets(source), before: task.identifier)
            model.dragTarget = nil
            return true
        } isTargeted: { targeted in
            if targeted { model.dragTarget = task.id }
            else if model.dragTarget == task.id { model.dragTarget = nil }
        }
        .overlay(alignment: .top) {
            if model.dragTarget == task.id { Rectangle().fill(palette.accent).frame(height: 2).allowsHitTesting(false) }
        }
        .contextMenu { actions(model.targets(task.identifier)) }
        .accessibilityAction(named: Text("Move up")) { model.nudge(task.identifier, by: -1) }
        .accessibilityAction(named: Text("Move down")) { model.nudge(task.identifier, by: 1) }
    }

    /// Who holds the Task: a filled mark for you, a muted one for someone else, a dashed ring for nobody.
    private func owner(_ id: String) -> some View {
        let owner = model.owners[id]
        return Group {
            if owner == "you" {
                Image(systemName: "person.crop.circle.fill").foregroundStyle(palette.accent)
            } else if owner != nil {
                Image(systemName: "person.crop.circle.fill").foregroundStyle(palette.textTertiary)
            } else {
                Circle().strokeBorder(palette.textTertiary, style: StrokeStyle(lineWidth: 1, dash: [2, 2])).frame(width: 14, height: 14)
            }
        }
        .font(.system(size: 15)).frame(width: 18)
        .help(owner == "you" ? "Claimed by you" : owner.map { "Claimed by \($0)" } ?? "Unclaimed")
    }

    private func opened(_ task: PreviewTask) -> some View {
        let editing = model.editing == task.identifier
        let long = task.brief.count > 900
        return VStack(alignment: .leading, spacing: 14) {
            if editing {
                TextEditor(text: Binding(get: { model.drafts[task.identifier] ?? task.brief }, set: { model.drafts[task.identifier] = $0 }))
                    .font(Typography.body(14)).scrollContentBackground(.hidden).frame(minHeight: 300)
                    .padding(8).background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
            } else {
                Text(markdown(task.brief)).font(Typography.body(14)).lineSpacing(5).textSelection(.enabled)
                    .lineLimit(long && !model.showsAll ? 9 : nil)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if editing {
                HStack(spacing: 10) {
                    Button("Save") { model.saveDraft(task.identifier) }.buttonStyle(.borderedProminent).tint(palette.accent)
                    Button("Close, keep draft") { model.editing = nil }
                }
            } else if long {
                Button(model.showsAll ? "Show less" : "Show all \(wordCount(task.brief)) words") { model.showsAll.toggle() }
                    .buttonStyle(.link).help("Return")
            }
        }
        .frame(maxWidth: 720, alignment: .leading)
        .padding(.leading, 156).padding(.trailing, 22).padding(.top, 4).padding(.bottom, 18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted)
    }

    private func wordCount(_ text: String) -> Int {
        text.split(whereSeparator: \.isWhitespace).count / 10 * 10
    }

    private var conversation: some View {
        VStack(alignment: .leading, spacing: 24) {
            HStack {
                Button { model.destination = "Library" } label: { Label("Library", systemImage: "list.bullet") }
                Spacer()
                Text(model.destination).font(Typography.strong(16))
            }
            Divider()
            Label("Existing conversation", systemImage: "terminal").font(Typography.strong(16))
            Text("This destination has the full workspace width. Returning to Library retains the scope, selection and drafts.")
                .font(Typography.body(16)).foregroundStyle(palette.textSecondary).frame(maxWidth: 620, alignment: .leading)
            Spacer()
            Text("Native layout preview · live provider not connected").font(Typography.code(12)).foregroundStyle(palette.textSecondary)
            TextField("Unsent reply", text: $model.reply, axis: .vertical).lineLimit(3...6).textFieldStyle(.roundedBorder)
        }.padding(28).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading).background(palette.surface)
    }

    private var captureSheet: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("An idea for later").font(Typography.strong(24))
            TextField("What might be worth doing?", text: $model.newIdea, axis: .vertical).lineLimit(4...8)
            Text("Product · unstarted").font(Typography.body(12)).foregroundStyle(palette.textSecondary)
            HStack {
                Button("Cancel") { model.capturing = false }
                Spacer()
                Button("Save preview idea") {
                    let words = model.newIdea.trimmingCharacters(in: .whitespacesAndNewlines)
                    let task = PreviewTask(id: UUID().uuidString, identifier: "IDEA-\(model.tasks.count + 1)", title: words, brief: words, rank: model.tasks.count)
                    model.tasks.append(task); model.select(task.identifier); model.query = ""; model.scope = "Product"; model.destination = "Library"
                    model.newIdea = ""; model.capturing = false; model.notice = "Idea saved in preview only"
                }.disabled(model.newIdea.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }.padding(26).frame(width: 500).foregroundStyle(palette.text).background(palette.surface)
    }

    private func markdown(_ source: String) -> AttributedString {
        (try? AttributedString(markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(source)
    }
}

/// The Wave view gives the conversation 1 − 0.618. A backlog gives it 0.382 of that, so the list gets three quarters.
let backlogSessionShare = (1 - 0.618) * 0.618

/// A copy of `RatioSplit`, `SplitSizing` and `SplitDivider` (LoopflowMac/Views), side by side only:
/// those sources need the whole Mac target to compile.
struct PreviewSplit<First: View, Second: View>: View {
    let ratio: Double
    let update: (Double) -> Void
    @ViewBuilder let first: First
    @ViewBuilder let second: Second
    @Environment(\.palette) private var palette
    @State private var dragStart: Double?
    @State private var hovering = false

    var body: some View {
        GeometryReader { geometry in
            let extent = geometry.size.width
            let divider: CGFloat = 6
            let shown = constrain(ratio, extent: extent)
            let firstExtent = max(0, extent * shown - divider / 2)
            HStack(spacing: 0) {
                first.frame(width: firstExtent).clipped()
                Rectangle()
                    .fill(hovering || dragStart != nil ? palette.borderActive : .clear)
                    .frame(width: divider)
                    .background(palette.border.frame(width: 1))
                    .contentShape(Rectangle())
                    .onHover { inside in
                        guard hovering != inside else { return }
                        hovering = inside
                        if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                    }
                    .gesture(DragGesture(minimumDistance: 0, coordinateSpace: .global)
                        .onChanged { value in
                            let start = dragStart ?? ratio
                            if dragStart == nil { dragStart = ratio }
                            guard extent > 0 else { return }
                            update(snapped(start + Double(value.translation.width / extent), extent: extent))
                        }
                        .onEnded { _ in dragStart = nil })
                second.frame(width: max(0, extent - firstExtent - divider)).clipped()
            }
        }
    }

    private func constrain(_ ratio: Double, extent: CGFloat) -> Double {
        guard extent > 0 else { return ratio }
        let minimum: CGFloat = 240
        let lower = (minimum + 3) / extent, upper = 1 - (minimum + 3) / extent
        return min(max(ratio, lower), max(lower, upper))
    }

    /// Snaps within 10 points of the golden splits, as the Task pane divider does, plus the backlog's default.
    private func snapped(_ ratio: Double, extent: CGFloat) -> Double {
        let bounded = constrain(ratio, extent: extent)
        for target in [backlogSessionShare, 1 - 0.618, 0.618] where abs(bounded - target) * extent <= 10 { return target }
        return bounded
    }
}

final class PreviewAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

@main
struct BacklogPreviewApp: App {
    @NSApplicationDelegateAdaptor(PreviewAppDelegate.self) private var delegate
    init() { bootstrapLoopflowApp() }
    var body: some Scene {
        WindowGroup("Loopflow · Backlog review") {
            BacklogPreview().environment(\.palette, .light)
        }
        .defaultSize(width: 1440, height: 900)
        .windowToolbarStyle(.unifiedCompact(showsTitle: false))
    }
}
