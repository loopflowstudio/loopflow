#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import Observation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct TaskFileBrowserJourneyTests {
    let call = "src/nested/Calls.swift"
    let page = "scratch/report.html"
    let source = "// readMore()\nfunc run() {\n    /* 😀 */ read()\n}\n"

    @Test(arguments: [false, true])
    func fromCheckoutSearchToConversation(peer: Bool) async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let server = LocalServerFixture(fixture.lf)
        do {
            try await runJourney(fixture, server: server, peer: peer)
        } catch {
            await server.stop()
            throw error
        }
        await server.stop()
    }

    private func write(_ text: String, to path: String, in root: URL) throws {
        let file = root.appendingPathComponent(path)
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: file)
    }

    private func runJourney(_ fixture: WorkCLIFixture, server: LocalServerFixture, peer: Bool) async throws {
        let lf = fixture.lf
        try write("scratch/\nbuild/\n", to: ".gitignore", in: fixture.repo)
        try write("func run() {}\n", to: call, in: fixture.repo)
        for name in ["A", "B"] {
            try write("struct \(name) {\n func read() {}\n}\n", to: "src/\(name).swift", in: fixture.repo)
        }
        _ = try await fixture.run(["add", "."], executable: "/usr/bin/git")
        _ = try await fixture.run(["commit", "-m", "Reader"], executable: "/usr/bin/git")
        let created = try await fixture.run(["task", "create", "--no-wave", "--title", "Review reader", "--json"])
        let issue = try JSONDecoder().decode(TaskCreateReceipt.self, from: Data(created.utf8)).task.id
        _ = try await fixture.run(["checkout", issue, "--json"])
        let location = try await fixture.run(["task", "location", issue, "--json"])
        let owner = try #require(JSONDecoder().decode([Owner].self, from: Data(location.utf8)).first)
        let checkout = try #require(owner.location.checkout, "Missing checkout")
        let root = URL(fileURLWithPath: checkout)
        try write(source, to: call, in: root)
        try write("<p>Report</p>", to: page, in: root)
        try write("read()", to: "build/ignored.swift", in: root)

        func cli<T: Decodable>(_ type: T.Type, _ args: [String]) async throws -> T {
            let json = try await lf.run(["task", args[0], issue, "--json"] + args.dropFirst())
            return try JSONDecoder().decode(type, from: Data(json.utf8))
        }
        let inventory = try await cli(TaskFileInventory.self, ["find"])
        #expect(inventory.paths.contains(call) && inventory.latestScratchPage == page)
        #expect(!inventory.paths.contains("build/ignored.swift"))
        let refs = try await cli(TaskGrepResults.self, ["grep", "--whole-word", "read"])
        #expect(refs.hits.map(\.path) == ["src/A.swift", "src/B.swift", call])
        let definitions = try await cli(TaskSymbols.self, ["symbols", "read"])
        #expect(definitions.hits.compactMap(\.container) == ["A", "B"])

        // A peer's checkout path names nothing here; every read goes by Task through the server.
        let displayedPath = peer ? "/peer/\(issue)" : checkout
        let query = lf.query(server)
        if peer { #expect(!FileManager.default.fileExists(atPath: displayedPath)) }
        let place = WorkspaceIdentity(machineId: owner.machineID, worktree: displayedPath)
        let registry = SessionsWorkspaceRegistry(localMachineId: peer ? fixtureMachineId : owner.machineID)
        defer { registry.prepareForQuit() }
        let key = WorkWorkspaceKey(repository: owner.repositoryID, kind: .task, id: issue)
        let workspace = registry.open(key, title: "Reader", location: place)
        let pane = try #require(workspace.multiplexer.showBrowser(.files, location: place))
        let files = try workspace.files(for: pane.content, task: nil, query: query)
        files.autosave = false
        let model = WorkModel(query: query)
        let context = DesktopActionContext(model: model, workspaces: registry, workspace: workspace, pane: pane)
        func binding(_ key: String) throws -> DesktopAction {
            try #require(DesktopAction.binding(characters: key, modifiers: .command, context: context)?.0)
        }

        await files.enterFiles()
        #expect(files.selection == page && files.scratchPaths == inventory.scratch)
        let tabs = files.tabs
        try binding("p").perform(context)
        let picker = try #require(files.filePicker)
        await picker.load()
        #expect(Set(picker.results.map(\.path)) == Set(inventory.paths))
        picker.text = "nstcalls"
        let selected = try #require(picker.selection?.path)
        #expect(selected == call)
        await picker.loadPreview(selected, draft: nil)
        guard case .text(let preview, _) = picker.preview else { Issue.record("Missing source preview"); return }
        #expect(preview == source && files.tabs == tabs && files.jumps.back.isEmpty)
        let field = try TaskFilePickerView(store: files, picker: picker).inspect().find(PaletteSearchField.self).actualView()
        field.submit()
        await files.loadFile()
        let doc = try #require(files.selectedDocument)
        #expect(doc.path == call && doc.text == source && doc.retainedEditor == nil)

        try binding("f").perform(context)
        let find = try #require(files.find)
        find.query = "read()"
        await find.search(lines: CodeLine.read(doc.text, grammar: nil))
        #expect(find.current?.cell == .line(3))
        files.find = nil
        let monitor = DesktopActionMonitor(context: { context }, failed: { Issue.record($0) }).makeCoordinator()
        let readerText = NSTextView()
        readerText.isEditable = false
        readerText.isSelectable = true
        func lookup(_ key: String) async throws {
            let line = "    /* 😀 */ read()"
            files.jumps.target = .init(path: call, line: 3, word: "read", range: (line as NSString).range(of: "read"))
            try #require(monitor.handleReadKey("g", modifiers: [], firstResponder: readerText))
            try #require(monitor.handleReadKey(key, modifiers: [], firstResponder: readerText))
            for await loading in Observations({ files.jumps.peek?.loading }) where loading != true { break }
        }
        try await lookup("r")
        let references = try #require(files.jumps.peek)
        #expect(references.candidates.compactMap(\.hit) == refs.hits)
        #expect(files.selection == call && files.jumps.back.isEmpty)
        files.jumps.accept(try #require(references.candidates.first), in: files)
        #expect(files.selection == "src/A.swift" && files.visit?.line == 2)
        try binding("[").perform(context)
        #expect(files.selection == call && files.visit?.line == 3)
        #expect(try binding("[").name == "back")

        try await lookup("d")
        let choices = try #require(files.jumps.peek?.candidates)
        #expect(choices.map(\.label) == ["A.read", "B.read"])
        #expect(choices.map(\.position) == definitions.hits.map { FilePosition(path: $0.path, line: $0.line) })
        files.jumps.accept(try #require(choices.last), in: files)
        await files.loadFile()
        let declaration = try #require(files.selectedDocument)
        files.comments.select(path: declaration.path, source: .file(draft: false), line: 1, extend: false)
        files.comments.select(path: declaration.path, source: .file(draft: false), line: 2, extend: true)
        files.comments.draft = "Which reader?"
        files.comments.add(anchor: Array(declaration.text.components(separatedBy: "\n").prefix(2)))
        try binding("[").perform(context)
        #expect(files.selection == call && files.visit?.line == 3)
        await files.diffReview.loadReview()
        let changed = try #require(files.diffReview.shownReview?.file(call))
        let diff = ReviewStream(store: files, reader: .changes, files: [changed], split: false, addToConversation: nil)
        let gutter = try #require(try diff.inspect().findAll(ReviewGutter.self).first).actualView()
        gutter.select()
        files.comments.draft = "Check this edit"
        try diff.inspect().find(ReviewCommentEditor.self).find(button: "Comment").tap()
        var input = [String]()
        let strip = ReviewCommentsStrip(store: files, isFocused: true) { input.append($0); return true }
        try strip.inspect().find(viewWithAccessibilityIdentifier: "changes-send").button().tap()
        #expect(input.count == 1 && files.comments.items.isEmpty)
        let message = try #require(input.first)
        #expect(message.contains("src/B.swift:L1-2") && message.contains("(working file) Which reader?"))
        #expect(message.contains(call) && message.contains(files.diffReview.revision) && message.contains("Check this edit"))

        files.diffReview.showStep(ReviewStep(name: "Unavailable step", base: "missing-base", commits: [], head: "HEAD"))
        await files.diffReview.reviewReading.refresh()
        #expect(files.diffReview.reviewReading.error != nil)
        files.chooseFile(.changed)
        let changedPicker = try #require(files.filePicker)
        await changedPicker.load()
        #expect(changedPicker.results.map(\.path) == [call] && files.openAndScratch.contains(page))
        files.filePicker = nil

        try await checkPresentations(context: context, files: files, doc: doc, checkout: root, lf: lf, peer: peer)
    }

    private func checkPresentations(context: DesktopActionContext, files: TaskFilesStore, doc: TaskFileDocument,
                                    checkout: URL, lf: BuiltLf, peer: Bool) async throws {
        let workspace = try #require(context.workspace)
        let terminal = try #require(workspace.multiplexer.sessionPane)
        workspace.multiplexer.setFocusedPane(terminal.id)
        let fromTerminal = DesktopActionContext(model: context.model, workspaces: context.workspaces, workspace: workspace, pane: terminal)
        #expect(DesktopAction.binding(characters: "f", modifiers: .command, context: fromTerminal) == nil)
        if peer {
            // Observe peer scratch writes through the real task.files.watch stream, not a direct invalidate shortcut.
            files.observeFiles()
            for await cwd in Observations({ files.cwd }) where cwd == checkout.path { break }
        }
        files.mode = .edit
        let editor = doc.editor
        let range = NSRange(location: 3, length: 4)
        editor.insertText("mine", replacementRange: range)
        editor.setSelectedRange(range)
        for (panePage, mode) in [(TaskFilesStore.Page.files, TaskFilesStore.Mode.read), (.brief, .read),
                                  (.changes, .read), (.files, .edit)] {
            files.page = panePage
            files.mode = mode
            let path = "scratch/\(panePage.rawValue)-\(mode.rawValue).html"
            for text in ["first", "settled"] {
                try write(text, to: path, in: checkout)
                if !peer { files.invalidate([path]) }
            }
            // Wait for the watcher batch to reach the rail before checking what stayed in place.
            for await updates in Observations({ files.browser.updated }) where updates.contains(path) { break }
            try #require(files.openAndScratch.contains(path) && files.browser.updated.filter { $0 == path }.count == 1)
            #expect(files.browser.notice == .updated(path))
            #expect(files.page == panePage && files.mode == mode && files.selection == call)
            #expect(doc.dirty && editor.selectedRange() == range && workspace.multiplexer.focusedPane.id == terminal.id)
        }

        let router = WorkLinkRouter()
        router.register(UUID(), repository: workspace.key.repository, focus: {}, inspect: {
            context.model.inspectDesktop(repository: $0, window: $1, workspaces: context.workspaces)
        }, perform: {
            try DesktopAction.perform($0, model: context.model, workspaces: context.workspaces)
        }, readText: { try context.workspaces.readText($0) }, receive: { _ in })
        let socket = try DesktopServer(path: DesktopServer.socketPath(home: lf.environment["LF_HOME"], bundle: "com.loopflow.mac"),
            inspect: router.inspect, perform: router.perform, readText: router.readText)
        defer { withExtendedLifetime(socket) {} }
        let present = ["desktop", "files", page, "-p", "session"]
        _ = try await lf.run(present, cwd: checkout)
        let offer = FilesBrowserState.Presentation(path: page)
        try #require(files.browser.notice == .presented(offer) && files.selection == call)
        files.openPresentation(offer)
        #expect(files.selection == page && workspace.multiplexer.focusedPane.id == terminal.id)
        files.selectFile(call)
        #expect(files.mode == .edit && doc.editor === editor && editor.selectedRange() == range)
        editor.undoManager?.undo()
        #expect(doc.text == source)
        files.mode = .read
        _ = try await lf.run(present, cwd: checkout)
        #expect(files.selection == page && workspace.multiplexer.focusedPane.id == terminal.id)
    }

    /// `lf task location`'s reading: who runs the Task and where its checkout is.
    private struct Owner: Decodable {
        struct Location: Decodable { let checkout: String? }
        let machineID: String
        let repositoryID: String
        let location: Location
        enum CodingKeys: String, CodingKey {
            case machineID = "machine_id", repositoryID = "repository_id", location
        }
    }
}
#endif
