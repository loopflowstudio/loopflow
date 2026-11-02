// The one mutation path for Desktop gestures. The shortcut monitor, menus,
// Keyboard Shortcuts window and the Desktop socket's `perform` all
// look actions up here and call `perform`; `lf desktop` verbs name them.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI
import WebKit

/// A default key and the arguments pressing it supplies (⌘2 is `goto index=2`).
struct DesktopKeyBinding: Equatable {
    enum Key: Equatable { case character(Character), `return`, tab, left, right, up, down }
    let key: Key
    let modifiers: NSEvent.ModifierFlags
    let args: [String: String]

    init(_ key: Key, _ modifiers: NSEvent.ModifierFlags, args: [String: String] = [:]) {
        self.key = key; self.modifiers = modifiers; self.args = args
    }

    var display: String {
        let symbols: [(NSEvent.ModifierFlags, String)] = [(.control, "⌃"), (.option, "⌥"), (.shift, "⇧"), (.command, "⌘")]
        let prefix = symbols.filter { modifiers.contains($0.0) }.map(\.1).joined()
        return switch key {
        case .character(let character): prefix + String(character).uppercased()
        case .return: prefix + "↩"
        case .tab: prefix + "⇥"
        case .left: prefix + "←"
        case .right: prefix + "→"
        case .up: prefix + "↑"
        case .down: prefix + "↓"
        }
    }

    var keyEquivalent: KeyEquivalent {
        switch key {
        case .character(let character): KeyEquivalent(character)
        case .return: .return
        case .tab: .tab
        case .left: .leftArrow
        case .right: .rightArrow
        case .up: .upArrow
        case .down: .downArrow
        }
    }

    var eventModifiers: EventModifiers {
        var result: EventModifiers = []
        if modifiers.contains(.command) { result.insert(.command) }
        if modifiers.contains(.shift) { result.insert(.shift) }
        if modifiers.contains(.option) { result.insert(.option) }
        if modifiers.contains(.control) { result.insert(.control) }
        return result
    }

    func matches(characters: String?, keyCode: UInt16, specialKey: NSEvent.SpecialKey?, modifiers flags: NSEvent.ModifierFlags) -> Bool {
        guard flags.intersection([.command, .shift, .option, .control]) == modifiers else { return false }
        return switch key {
        case .character(let character): characters?.lowercased() == String(character)
        case .return: keyCode == 36 || keyCode == 76
        // ⌃⇧⇥ arrives as backtab.
        case .tab: characters == "\t" || characters == "\u{19}"
        case .left: specialKey == .leftArrow
        case .right: specialKey == .rightArrow
        case .up: specialKey == .upArrow
        case .down: specialKey == .downArrow
        }
    }
}

/// The window an action runs in and, for pane actions, the addressed pane:
/// the focused one for keys and menus, an exact target for scripts.
@MainActor
struct DesktopActionContext {
    let model: WorkModel
    let workspaces: SessionsWorkspaceRegistry
    let workspace: SessionsWorkspace?
    let pane: PaneState?
    /// The addressed shell or browser tab; nil addresses the pane's selected tab.
    let tab: String?
    let args: [String: String]

    init(model: WorkModel, workspaces: SessionsWorkspaceRegistry, workspace: SessionsWorkspace?,
         pane: PaneState?, tab: String? = nil, args: [String: String] = [:]) {
        self.model = model; self.workspaces = workspaces; self.workspace = workspace
        self.pane = pane; self.tab = tab; self.args = args
    }

    func with(args: [String: String]) -> Self {
        Self(model: model, workspaces: workspaces, workspace: workspace, pane: pane, tab: tab, args: args)
    }

    fileprivate func string(_ name: String) throws -> String {
        guard let value = args[name] else { throw DesktopArgumentError("Missing argument \(name)=…; nothing changed.") }
        return value
    }

    fileprivate func integer(_ name: String) throws -> Int {
        let value = try string(name)
        guard let number = Int(value) else { throw DesktopArgumentError("\(name)=\(value) is not an integer; nothing changed.") }
        return number
    }

    fileprivate func boolean(_ name: String) throws -> Bool? {
        guard let value = args[name] else { return nil }
        switch value.lowercased() {
        case "true", "on", "yes", "1": return true
        case "false", "off", "no", "0": return false
        default: throw DesktopArgumentError("\(name)=\(value) is not true or false; nothing changed.")
        }
    }

    fileprivate var addressed: (SessionsWorkspace, PaneState) {
        get throws {
            guard let workspace, let pane else { throw DesktopTargetError("Address a pane for this action; nothing changed.") }
            return (workspace, pane)
        }
    }

    fileprivate var task: SessionsWorkspace {
        get throws {
            let (workspace, _) = try addressed
            guard workspace.key.kind == .task else {
                throw DesktopTargetError("Shell and browser belong to Task workspaces; nothing changed.")
            }
            return workspace
        }
    }

    /// The addressed pane's terminal: a shell's addressed or selected tab, or the session.
    fileprivate var terminal: TerminalIdentity {
        get throws {
            let (workspace, pane) = try addressed
            if let tab, let shell = workspace.multiplexer.shell(tab), let terminal = shell.terminal { return terminal }
            guard let terminal = pane.terminal else {
                throw RegistryQueryError("The addressed pane is not a terminal; no input was sent.")
            }
            return terminal
        }
    }

    /// Scripts fence input with the surface they inspected; menus use the current one.
    fileprivate func surface(for terminal: TerminalIdentity) throws -> String {
        if let surface = args["surface"] { return surface }
        guard let surface = workspaces.surfaces.surfaceIncarnation(for: terminal) else {
            throw RegistryQueryError("The retained pane has no native surface; no input was sent.")
        }
        return surface
    }

    /// Another pane of the addressed workspace, by pane ID or designated kind.
    fileprivate func peer(_ name: String) throws -> PaneState {
        let (workspace, pane) = try addressed
        let selector = try string(name)
        let store = workspace.multiplexer
        let other = store.layout.pane(for: selector)
            ?? PaneContent.Kind(rawValue: selector).flatMap { store.layout.pane(of: $0) }
        guard let other, other.id != pane.id else {
            throw DesktopTargetError("Choose another pane of the same workspace for \(name)=\(selector); nothing changed.")
        }
        return other
    }
}

/// The pane and tab an action affected, when it addressed or created one.
struct DesktopActionOutcome: Equatable {
    var pane: String?
    var tab: String?
}

@MainActor
struct DesktopAction {
    let name: String
    let title: String
    let scope: DesktopActionScope
    let arguments: [DesktopActionArgument]
    let keys: [DesktopKeyBinding]
    /// Keys and menus offer the action only where it can run.
    let available: (DesktopActionContext) -> Bool
    let run: (DesktopActionContext) throws -> DesktopActionOutcome?

    init(_ name: String, _ title: String, scope: DesktopActionScope = .window, arguments: [DesktopActionArgument] = [],
         keys: [DesktopKeyBinding] = [], available: ((DesktopActionContext) -> Bool)? = nil,
         run: @escaping (DesktopActionContext) throws -> DesktopActionOutcome?) {
        self.name = name; self.title = title; self.scope = scope; self.arguments = arguments; self.keys = keys
        self.available = available ?? { context in scope == .window || context.pane != nil }
        self.run = run
    }

    var descriptor: DesktopActionDescriptor {
        DesktopActionDescriptor(name: name, title: title, scope: scope, arguments: arguments, keys: keys.map(\.display))
    }

    /// Validate arguments, then run. A failure changes nothing.
    @discardableResult
    func perform(_ context: DesktopActionContext) throws -> DesktopActionOutcome {
        if let unknown = context.args.keys.sorted().first(where: { name in !arguments.contains { $0.name == name } }) {
            throw DesktopArgumentError("\(self.name) takes no argument \(unknown); see lf desktop actions. Nothing changed.")
        }
        if let missing = arguments.first(where: { $0.required && context.args[$0.name] == nil }) {
            throw DesktopArgumentError("\(name) requires \(missing.name)=…; nothing changed.")
        }
        // A view part named by `part` stands in for the pane.
        if scope == .pane, context.args["part"] == nil { _ = try context.addressed }
        let outcome = try run(context)
        return outcome ?? DesktopActionOutcome(pane: context.pane?.id, tab: context.tab ?? context.pane?.content.selectedTabID)
    }

    static func named(_ name: String) throws -> DesktopAction {
        guard let action = all.first(where: { $0.name == name }) else {
            throw DesktopArgumentError("Unknown action \(name); see lf desktop actions. Nothing changed.")
        }
        return action
    }

    /// The registry action and arguments a key press performs.
    static func binding(characters: String?, keyCode: UInt16 = 0, specialKey: NSEvent.SpecialKey? = nil,
                        modifiers: NSEvent.ModifierFlags) -> (DesktopAction, [String: String])? {
        for action in all {
            if let binding = action.keys.first(where: {
                $0.matches(characters: characters, keyCode: keyCode, specialKey: specialKey, modifiers: modifiers)
            }) { return (action, binding.args) }
        }
        return nil
    }

    /// The socket's entry: resolve the exact target, then perform.
    static func perform(_ request: DesktopActionRequest, model: WorkModel,
                        workspaces: SessionsWorkspaceRegistry) throws -> DesktopActionOutcome {
        let action = try named(request.action)
        var workspace: SessionsWorkspace?
        var pane: PaneState?
        if let target = request.pane {
            (workspace, pane, _) = try workspaces.retainedPane(for: target)
        }
        return try action.perform(DesktopActionContext(model: model, workspaces: workspaces, workspace: workspace,
            pane: pane, tab: request.pane?.tab, args: request.args))
    }

    // MARK: - Registry

    private static let taskTool: (DesktopActionContext) -> Bool = { $0.pane != nil && $0.workspace?.key.kind == .task }

    static let all: [DesktopAction] = navigation + panes + tools

    private static let navigation: [DesktopAction] = [
        DesktopAction("back", "Back", keys: [.init(.character("["), .command)]) { context in
            guard !context.model.navigation.backStack.isEmpty else { throw DesktopTargetError("Nothing to go back to. Nothing changed.") }
            context.model.goBack()
            return nil
        },
        DesktopAction("forward", "Forward", keys: [.init(.character("]"), .command)]) { context in
            guard !context.model.navigation.forwardStack.isEmpty else { throw DesktopTargetError("Nothing to go forward to. Nothing changed.") }
            context.model.goForward()
            return nil
        },
        DesktopAction("goto", "Working Task", arguments: [.init(name: "index", kind: .integer, required: true)],
            keys: (1...9).map { .init(.character(Character(String($0))), .command, args: ["index": String($0)]) }) { context in
            let index = try context.integer("index")
            let ids = context.model.sidebarTaskIDs
            guard ids.indices.contains(index - 1) else {
                throw DesktopTargetError("The sidebar has no Task at position \(index); it has \(ids.count). Nothing changed.")
            }
            context.model.selectSidebarTask(slot: index - 1)
            return nil
        },
        DesktopAction("next", "Next Task", keys: [.init(.tab, .control)]) { try cycle($0, by: 1) },
        DesktopAction("previous", "Previous Task", keys: [.init(.tab, [.control, .shift])]) { try cycle($0, by: -1) },
        DesktopAction("new-task", "New Task", keys: [.init(.character("n"), .command)],
                      available: { $0.model.repoPath != nil }) { context in
            guard context.model.repoPath != nil else { throw DesktopTargetError("Open a repository first. Nothing changed.") }
            context.model.navigation.palette = .newTask
            return nil
        },
        DesktopAction("working-tasks", WorkNavigation.workingTasksTitle) { context in
            context.model.selectWorkingTasksDashboard()
            return nil
        },
        // lf resolves the tile as `-p` does and sends its full Work ID.
        DesktopAction("enter", "Enter Tile", arguments: [.init(name: "tile", kind: .string, required: true)]) { context in
            let model = context.model
            let id = try context.string("tile")
            let tiles: [WorkMemberRef]? = switch model.navigation.destination {
            case .workingTasks: model.workingTaskMembers
            case .dashboard, .view: model.selectedViewPlan?.projection.visible
            case .work: model.navigation.waveView.map { model.boardTiles($0) }
            case .entered: nil
            }
            guard let tiles else { throw DesktopTargetError("This window shows no tiles. Nothing changed.") }
            guard let member = tiles.first(where: { $0.work.id == id }) else {
                throw DesktopTargetError("\(id) is not a tile here. Nothing changed.")
            }
            model.enterTile(member.work, origin: nil)
            return nil
        },
        DesktopAction("page", "Turn Wave Tasks to Page", arguments: [.init(name: "page", kind: .integer, required: true)],
                      available: { $0.model.navigation.waveView != nil }) { context in
            let model = context.model
            guard let wave = model.navigation.waveView, let plan = model.boardPlan(wave) else {
                throw DesktopTargetError("This window shows no Wave view. Nothing changed.")
            }
            let pages = plan.slots.pages(model.boardTiles(wave)).count
            guard let page = Int(try context.string("page")), (1...pages).contains(page) else {
                throw DesktopArgumentError("page must be 1 to \(pages); nothing changed.")
            }
            model.navigation.wavePages[wave] = page - 1
            return nil
        },
        DesktopAction("section", "Show Wave Board Section", arguments: [.init(name: "section", kind: .string, required: true)],
                      available: { $0.model.navigation.waveView != nil }) { context in
            guard let view = context.model.navigation.waveView else {
                throw DesktopTargetError("This window shows no Wave board. Nothing changed.")
            }
            let value = try context.string("section")
            guard let section = WaveBoardSection(rawValue: value) else {
                throw DesktopArgumentError("section=\(value) is not backlog, working, autonomous or done; nothing changed.")
            }
            context.model.navigation.boardSections[view] = section
            return nil
        },
        DesktopAction("filter", "Set Wave Board Done Days", arguments: [.init(name: "days", kind: .integer, required: true)],
                      available: { $0.model.navigation.waveView != nil }) { context in
            guard context.model.navigation.waveView != nil else {
                throw DesktopTargetError("This window shows no Wave board. Nothing changed.")
            }
            var filter = TaskHistoryFilter()
            filter.editDays(try context.string("days"))
            if let message = filter.validationMessage { throw DesktopArgumentError(message) }
            UserDefaults.standard.set(filter.days, forKey: WaveStandupDefaults.doneDaysKey)
            return nil
        },
    ]

    /// `lf desktop hide|show -p board|tiles` names a Wave view part in place of a pane.
    private static let part = DesktopActionArgument(name: "part", kind: .string, required: false)

    /// The Wave view part `part=` names; nil when the action addresses a pane.
    private static func viewPart(_ context: DesktopActionContext) throws -> (WorkNavigation.Board, WaveViewPart)? {
        guard let name = context.args["part"] else { return nil }
        guard let view = context.model.navigation.waveView else {
            throw DesktopTargetError("This window shows no Wave view. Nothing changed.")
        }
        guard let part = WaveViewPart(rawValue: name) else {
            throw DesktopArgumentError("part=\(name) is not board, session or tiles; nothing changed.")
        }
        return (view, part)
    }

    /// Move the divider between `part` and `toward=` so `part` takes `ratio` of their split.
    private static func resizeViewPart(_ context: DesktopActionContext, view: WorkNavigation.Board,
                                       part: WaveViewPart, ratio: Double) throws {
        let value = try context.string("toward")
        guard let toward = WaveViewPart(rawValue: value), toward != part else {
            throw DesktopArgumentError("toward=\(value) must name another of board, session or tiles; nothing changed.")
        }
        var ratios = context.model.navigation.waveRatios[view] ?? WaveViewRatios()
        switch (part, toward) {
        case (.session, .board): ratios.session = ratio
        case (.board, .session): ratios.session = 1 - ratio
        case (.tiles, _): ratios.band = 1 - ratio
        case (_, .tiles): ratios.band = ratio
        default: return
        }
        context.model.navigation.waveRatios[view] = ratios
    }

    /// Hide or show the named Wave view part; false when the action addresses a pane.
    private static func setViewPart(_ context: DesktopActionContext, hidden: Bool) throws -> Bool {
        guard let (view, part) = try viewPart(context) else { return false }
        let name = part.rawValue
        guard context.model.isHidden(part, in: view) != hidden else {
            throw DesktopTargetError("\(name) is already \(hidden ? "hidden" : "shown"); nothing changed.")
        }
        context.model.setHidden(part, hidden, in: view)
        return true
    }

    private static func cycle(_ context: DesktopActionContext, by offset: Int) throws -> DesktopActionOutcome? {
        guard !context.model.sidebarTaskIDs.isEmpty else { throw DesktopTargetError("The sidebar shows no Tasks. Nothing changed.") }
        context.model.cycleSidebarTask(offset)
        return nil
    }

    private static let panes: [DesktopAction] = [
        DesktopAction("focus", "Focus Pane", scope: .pane, arguments: [.init(name: "direction", kind: .direction, required: false)],
            keys: [(DesktopKeyBinding.Key.left, "left"), (.right, "right"), (.up, "up"), (.down, "down")].map {
                .init($0.0, [.command, .option], args: ["direction": $0.1])
            }) { context in
            let (workspace, pane) = try context.addressed
            let store = workspace.multiplexer
            if let direction = context.args["direction"] {
                let directions: [String: SpatialDirection] = ["left": .left, "right": .right, "up": .up, "down": .down]
                guard let direction = directions[direction] else {
                    throw DesktopArgumentError("direction=\(direction) is not left, right, up or down; nothing changed.")
                }
                store.focus(direction)
                return DesktopActionOutcome(pane: store.focusedPaneId, tab: store.focusedPane.content.selectedTabID)
            }
            let model = context.model, workspaces = context.workspaces
            if model.navigation.showsSelection, workspaces.openWork.contains(workspace.key) {
                workspaces.focus(workspace.key)
            } else {
                model.select(workspace.key.reference)
            }
            if let tab = context.tab, store.shell(tab) != nil { store.selectShell(tab) } else { store.setFocusedPane(pane.id) }
            if let terminal = try? context.terminal { workspaces.surfaces.focus(terminal) }
            return nil
        },
        DesktopAction("zoom", "Zoom Pane", scope: .pane, arguments: [.init(name: "on", kind: .boolean, required: false), part],
            keys: [.init(.return, [.command, .shift])]) { context in
            if let (view, part) = try viewPart(context) {
                let on = try context.boolean("on") ?? (context.model.zoomedPart(in: view) != part)
                context.model.setZoomed(on ? part : nil, in: view)
                return nil
            }
            let (workspace, pane) = try context.addressed
            if let on = try context.boolean("on") { workspace.multiplexer.setZoom(pane.id, enabled: on) }
            else { workspace.toggleFocus(pane.id) }
            return nil
        },
        DesktopAction("hide", "Hide Pane", scope: .pane, arguments: [part]) { context in
            if try setViewPart(context, hidden: true) { return nil }
            let (workspace, pane) = try context.addressed
            workspace.setCollapsed(paneId: pane.id, collapsed: true)
            return nil
        },
        DesktopAction("close", "Close Tab", scope: .pane, keys: [.init(.character("w"), .command)]) { context in
            let (workspace, pane) = try context.addressed
            workspace.closePane(pane.id, tab: context.tab)
            return nil
        },
        DesktopAction("show", "Show Pane", scope: .pane, arguments: [part]) { context in
            if try setViewPart(context, hidden: false) { return nil }
            let (workspace, pane) = try context.addressed
            guard workspace.multiplexer.collapsedPaneIds.contains(pane.id) else {
                throw DesktopTargetError("\(pane.id) is already shown; nothing changed.")
            }
            workspace.setCollapsed(paneId: pane.id, collapsed: false)
            return nil
        },
        DesktopAction("reopen", "Undo Close", scope: .pane, keys: [.init(.character("t"), [.command, .shift])],
            available: { $0.workspace?.multiplexer.canUndoClose == true }) { context in
            let (workspace, _) = try context.addressed
            let store = workspace.multiplexer
            guard store.canUndoClose else { throw DesktopTargetError("Nothing was closed; nothing changed.") }
            store.undoClose()
            return DesktopActionOutcome(pane: store.focusedPaneId, tab: store.focusedPane.content.selectedTabID)
        },
        DesktopAction("move", "Move Pane", scope: .pane, arguments: [
            .init(name: "destination", kind: .pane, required: true), .init(name: "axis", kind: .axis, required: true),
        ]) { context in
            let (workspace, pane) = try context.addressed
            let destination = try context.peer("destination")
            let value = try context.string("axis")
            guard let axis = SplitAxis(rawValue: value) else {
                throw DesktopArgumentError("axis=\(value) is not horizontal or vertical; nothing changed.")
            }
            workspace.multiplexer.move(pane.id, beside: destination.id, axis: axis)
            return nil
        },
        DesktopAction("resize", "Resize Pane", scope: .pane, arguments: [
            .init(name: "toward", kind: .pane, required: true), .init(name: "ratio", kind: .ratio, required: true), part,
        ]) { context in
            let value = try context.string("ratio")
            guard let ratio = Double(value), ratio.isFinite, (0.1...0.9).contains(ratio) else {
                throw DesktopArgumentError("Split ratio must be between 0.1 and 0.9; nothing changed.")
            }
            if let (view, part) = try viewPart(context) {
                try resizeViewPart(context, view: view, part: part, ratio: ratio)
                return nil
            }
            let (workspace, pane) = try context.addressed
            let toward = try context.peer("toward")
            workspace.multiplexer.updateRatio(between: pane.id, and: toward.id, ratio: ratio)
            return nil
        },
        DesktopAction("send", "Send Text", scope: .pane, arguments: [
            .init(name: "text", kind: .string, required: true), .init(name: "surface", kind: .string, required: false),
        ]) { context in
            let terminal = try context.terminal
            try context.workspaces.surfaces.insertText(try context.string("text"), terminal: terminal,
                surface: try context.surface(for: terminal))
            return nil
        },
        DesktopAction("send-key", "Send Key", scope: .pane, arguments: [
            .init(name: "key", kind: .key, required: true), .init(name: "surface", kind: .string, required: false),
        ]) { context in
            let value = try context.string("key")
            guard let key = DesktopKey(rawValue: value) else {
                throw DesktopArgumentError("key=\(value) is not one of \(DesktopKey.allCases.map(\.rawValue).joined(separator: ", ")); nothing changed.")
            }
            let terminal = try context.terminal
            try context.workspaces.surfaces.sendKey(key, terminal: terminal, surface: try context.surface(for: terminal))
            return nil
        },
    ]

    private static let tools: [DesktopAction] = [
        // Scripts add a tab without moving focus; ⌘T focuses it.
        DesktopAction("run", "New Terminal", scope: .pane, arguments: [
            .init(name: "command", kind: .string, required: false), .init(name: "focus", kind: .boolean, required: false),
        ], keys: [.init(.character("t"), .command, args: ["focus": "true"])], available: taskTool) { context in
            let workspace = try context.task
            if let line = context.args["command"] { try validateDesktopLiteralText(line) }
            let shell = try workspace.newShell(focus: try context.boolean("focus") ?? false)
            // The line is typed into the tab's own shell, as a person would, so the
            // tab outlives the command and `wait` reads its finish.
            if let line = context.args["command"], let terminal = shell?.content.selectedShell?.terminal {
                try context.workspaces.surfaces.submit(line, terminal: terminal)
            }
            return DesktopActionOutcome(pane: shell?.id, tab: shell?.content.selectedShell?.id)
        },
        DesktopAction("toggle-shell", "Toggle Shell", scope: .pane, keys: [.init(.character("`"), .control)],
            available: taskTool) { context in
            let workspace = try context.task
            try workspace.toggleShell()
            return DesktopActionOutcome(pane: workspace.multiplexer.shellPane?.id, tab: workspace.multiplexer.shellPane?.content.selectedTabID)
        },
        DesktopAction("toggle-browser", "Toggle Browser", scope: .pane, keys: [.init(.character("e"), [.command, .shift])],
            available: taskTool) { context in
            let workspace = try context.task
            workspace.toggleBrowser()
            return browserOutcome(workspace)
        },
        DesktopAction("files", "Show Files", scope: .pane, available: taskTool) { context in
            try show(.files, context)
        },
        DesktopAction("graph-log", "Show Graph Processes", scope: .pane, available: taskTool) { context in
            try show(.graphLog, context)
        },
        DesktopAction("browse", "Open Web Page", scope: .pane, arguments: [.init(name: "url", kind: .url, required: true)]) { context in
            let value = try context.string("url")
            guard let url = URL(string: value), let page = BrowserTab.page(url) else {
                throw DesktopArgumentError("url=\(value) is not an http or https address; nothing changed.")
            }
            return try show(page, context)
        },
        // ⌘[ and ⌘] stay Desktop navigation; the page's own Back and Forward are scriptable and in its toolbar.
        DesktopAction("page-back", "Web Page Back", scope: .pane, available: webPage) { context in
            let page = try page(context)
            guard page.canGoBack else { throw DesktopTargetError("The web page has nothing to go back to. Nothing changed.") }
            page.goBack()
            return nil
        },
        DesktopAction("page-forward", "Web Page Forward", scope: .pane, available: webPage) { context in
            let page = try page(context)
            guard page.canGoForward else { throw DesktopTargetError("The web page has nothing to go forward to. Nothing changed.") }
            page.goForward()
            return nil
        },
        DesktopAction("reload", "Reload Web Page", scope: .pane, keys: [.init(.character("r"), .command)],
            available: webPage) { context in
            try page(context).reload()
            return nil
        },
        DesktopAction("switch", "Switch Conversation", scope: .pane,
            arguments: [.init(name: "session", kind: .string, required: true)]) { context in
            let (workspace, _) = try context.addressed
            let id = try context.string("session")
            let model = context.model
            let candidates = workspace.key.kind == .task ? model.taskSessions(workspace.key.id) : (model.sessions.value ?? [])
            guard let record = candidates.first(where: { $0.id == id }) else {
                throw DesktopTargetError("Session \(id) is not a conversation of this Work; nothing changed.")
            }
            context.workspaces.revealSession(record, in: workspace)
            return DesktopActionOutcome(pane: workspace.multiplexer.sessionPane?.id, tab: nil)
        },
    ]

    private static let webPage: (DesktopActionContext) -> Bool = { (try? webTab($0)) != nil }

    /// The addressed browser tab when it is a web page; Files and Graph processes are not.
    private static func webTab(_ context: DesktopActionContext) throws -> (SessionsWorkspace, id: String, url: URL) {
        let (workspace, pane) = try context.addressed
        guard case .browser(_, let tabs, let selected, _, _) = pane.content,
              case .url(let id, let url)? = tabs.first(where: { $0.id == (context.tab ?? selected) }) else {
            throw DesktopTargetError("The addressed tab is not a web page; nothing changed.")
        }
        return (workspace, id, url)
    }

    private static func page(_ context: DesktopActionContext) throws -> WKWebView {
        let (workspace, id, url) = try webTab(context)
        return workspace.pages.view(for: id, url: url)
    }

    private static func show(_ tab: BrowserTab, _ context: DesktopActionContext) throws -> DesktopActionOutcome {
        let workspace = try context.task
        workspace.showBrowser(tab, from: context.pane?.content.kind == .browser ? nil : context.pane, focus: false)
        return browserOutcome(workspace)
    }

    private static func browserOutcome(_ workspace: SessionsWorkspace) -> DesktopActionOutcome {
        let browser = workspace.multiplexer.browserPane
        return DesktopActionOutcome(pane: browser?.id, tab: browser?.content.selectedTabID)
    }
}

extension PaneContent {
    /// The selected shell or browser tab; a session pane has none.
    var selectedTabID: String? {
        switch self {
        case .shell: selectedShell?.id
        case .browser: selectedBrowserTab?.id
        case .session: nil
        }
    }
}

/// Intercepts registry keys before a retained terminal can consume them.
struct DesktopActionMonitor: NSViewRepresentable {
    /// The context a key press acts in, read at the moment of the press.
    let context: () -> DesktopActionContext
    let failed: (Error) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(context: context, failed: failed) }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.install(for: view)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.view = nsView
        context.coordinator.context = self.context
        context.coordinator.failed = failed
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) { coordinator.uninstall() }

    @MainActor
    final class Coordinator {
        weak var view: NSView?
        var context: () -> DesktopActionContext
        var failed: (Error) -> Void
        private var monitor: Any?

        init(context: @escaping () -> DesktopActionContext, failed: @escaping (Error) -> Void) {
            self.context = context
            self.failed = failed
        }

        func install(for view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self, let window = self.view?.window,
                      event.window === window, window.isKeyWindow, window.attachedSheet == nil,
                      (window.firstResponder as? NSTextView)?.hasMarkedText() != true,
                      let (action, args) = DesktopAction.binding(characters: event.charactersIgnoringModifiers,
                          keyCode: event.keyCode, specialKey: event.specialKey, modifiers: event.modifierFlags)
                else { return event }
                let context = self.context().with(args: args)
                guard action.available(context) else { return event }
                do { try action.perform(context) } catch { self.failed(error) }
                return nil
            }
        }

        func uninstall() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
        }
    }
}
#endif
