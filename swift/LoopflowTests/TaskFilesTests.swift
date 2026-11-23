#if os(macOS) && canImport(GhosttyKit)
import AppKit
import os
import GhosttyKit
import SwiftUI
import Testing
import ViewInspector
import WebKit
@testable import Loopflow
@testable import LoopflowMac

private struct FilesFixture: Decodable {
    let symbols: TaskSymbols
    let grep: TaskGrepResults
    let find: TaskFileInventory
    let directory: TaskDirectory
    let changes: TaskChangesSnapshot
    let diff: TaskDiffSnapshot
    let file: TaskFileSnapshot
    let image: TaskFileSnapshot
    let save: TaskFileSave
}

@Suite("TaskFiles", .serialized)
@MainActor
struct TaskFilesTests {
    private func fixture() throws -> FilesFixture {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json")
        return try JSONDecoder().decode(FilesFixture.self, from: Data(contentsOf: url))
    }

    private func snapshot(_ text: String, revision: String? = nil, path: String = "note.txt") -> TaskFileSnapshot {
        TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files", path: path,
                         content: text, data: nil, state: .text, revision: revision ?? text, readOnlyReason: nil, sizeBytes: UInt64(text.utf8.count))
    }

    @Test func localFilesKeepTheirContentLocationAndDraftWithoutPlanning() async throws {
        let location = WorkspaceIdentity(machineId: "local", worktree: "/task/checkout")
        let workspace = fixtureWork(.init(machineId: "local", worktree: "/other/work"))
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Offline") }
        let content = PaneContent.browser(taskId: "task-files", tabs: [.files], selected: "files", location: location)
        let pane = try #require(workspace.multiplexer.showBrowser(.files, location: location))
        let files = try workspace.files(for: content, task: nil, query: query)
        #expect(files.cwd == location.worktree)
        files.autosave = false
        let document = files.document("note.txt")
        await document.reconcile(snapshot("original"))
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 0))
        let draft = document.text
        #expect(document.dirty)
        // A new shell uses its Task location; the file draft retains its own location.
        let shell = try #require(try workspace.newShell())
        #expect(shell.content.location == workspace.location)
        workspace.multiplexer.close(pane.id)
        workspace.multiplexer.undoClose()
        let reopened = try #require(workspace.multiplexer.layout.pane(for: pane.id))
        #expect(reopened.content.location == content.location)
        #expect(reopened.content.selectedBrowserTab == .files)
        #expect(try workspace.files(for: reopened.content, task: nil, query: query) === files)
        #expect(document.text == draft)
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: .browser(taskId: "task-files", tabs: [.files], selected: "files"), task: nil, query: query)
        }
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: .browser(taskId: "task-files", tabs: [.files], selected: "files", location: .init(machineId: "local", worktree: "/replacement")),
                task: nil, query: query)
        }
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: content, task: nil, query: query.onMachine("peer", repository: "plan"))
        }
        #expect(files.document("note.txt") === document)
        #expect(document.text == draft)
    }

    @Test func filePaneRoutesWithoutLinkedSessionPrimingAndRetainsDrafts() async throws {
        let peer = RemoteFilesFixture()
        let query = RegistryQuery { args, cwd in try await peer.read(args, cwd: cwd) }
        let location = TaskLocationObservation(request: "opened", repositoryID: "plan", taskID: "portable",
            machineID: "peer", observedAt: 1, location: .recorded(taskID: "remote-task", checkout: "/peer/checkout"))
        // The enclosing checkout is deliberately unrelated to the pane's content.
        let workspace = fixtureWork(.init(machineId: "local", worktree: "/local/other"))
        let content = PaneContent.browser(taskId: "portable", tabs: [.files], selected: "files", execution: location)
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: .browser(taskId: "another-task", tabs: [.files], selected: "files", execution: location), task: nil, query: query)
        }
        let store = try workspace.files(for: content, task: nil, query: query)
        store.autosave = false
        store.selection = "note.txt"
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        #expect(document.text == "one\ntwo\n")
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 3))
        #expect(try workspace.files(for: content, task: nil, query: query) === store)
        let refreshed = TaskLocationObservation(request: "refreshed", repositoryID: location.repositoryID,
            taskID: location.taskID, machineID: location.machineID, observedAt: 2, location: location.location)
        #expect(try workspace.files(for: .browser(taskId: "portable", tabs: [.files], selected: "files", execution: refreshed), task: nil, query: query) === store)
        #expect(document.text == "draft\ntwo\n")
        #expect(document.dirty)

        let changed = TaskLocationObservation(request: "changed", repositoryID: "other-plan", taskID: "portable",
            machineID: "peer", observedAt: 2, location: location.location)
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: .browser(taskId: "portable", tabs: [.files], selected: "files", execution: changed), task: nil, query: query)
        }
        #expect(throws: RegistryQueryError.self) {
            try workspace.files(for: .browser(taskId: "portable", tabs: [.files], selected: "files"), task: nil, query: query)
        }
        #expect(store.document("note.txt") === document)
        #expect(document.text == "draft\ntwo\n")
        // A stale owner reply must not replace this draft with another checkout's bytes.
        await peer.change("WRONG OWNER", changeOwnerDuringRead: true)
        await store.loadFile()
        #expect(store.error != nil)
        #expect(document.text == "draft\ntwo\n")
    }

    @Test func releasingFilesStoreEndsRemoteObservation() async throws {
        let state = OSAllocatedUnfairLock(initialState: (started: false, ended: false))
        let stream = AsyncThrowingStream<TaskFilesFrame, any Error>.makeStream()
        stream.continuation.onTermination = { _ in state.withLock { $0.ended = true } }
        let query = RegistryQuery(watchFiles: { _, _ in
            state.withLock { $0.started = true }
            return TaskFilesObservation(frames: stream.stream, cancel: { stream.continuation.finish() })
        }, run: { _, _ in throw RegistryQueryError("No file read expected") })
            .onMachine("peer", repository: "plan")
        var store: TaskFilesStore? = TaskFilesStore(issue: "remote-task", cwd: "/peer/checkout", query: query)
        weak var retained = store
        store?.observeFiles()
        try await eventually { state.withLock { $0.started } }
        #expect(!state.withLock { $0.ended })
        store = nil
        #expect(retained == nil)
        try await eventually { state.withLock { $0.ended } }
    }

    @Test(arguments: ["disconnect", "stale", "changed-owner"])
    func remoteEventsReconcileRetainedDraftsAndRejectLostOrStaleOwners(ending: String) async throws {
        let peer = RemoteFilesFixture()
        let location = TaskLocationObservation(request: "opened", repositoryID: "plan", taskID: "portable",
            machineID: "peer", observedAt: 1, location: .recorded(taskID: "remote-task", checkout: "/peer/checkout"))
        let query = RegistryQuery(watchFiles: { args, cwd in
            try await peer.watch(args, cwd: cwd)
        }, run: { args, cwd in
            try await peer.read(args, cwd: cwd)
        }).onMachine("peer", repository: "plan").pinTaskLocation(to: location)
        let store = TaskFilesStore(issue: "remote-task", cwd: "/peer/checkout", query: query)
        store.autosave = false
        store.selection = "note.txt"
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        document.editor.insertText("local", replacementRange: NSRange(location: 0, length: 3))
        document.editor.setSelectedRange(NSRange(location: 2, length: 0))
        store.selection = "other.txt" // Retained hidden documents still receive changes.
        store.observeFiles()
        await peer.change("one\nPEER\n")
        try await eventually { document.text == "local\nPEER\n" }
        #expect(store.document("note.txt") === document)
        #expect(document.dirty)
        #expect(document.editor.selectedRange().location == 2)
        #expect(store.selection == "other.txt")
        if ending == "changed-owner" {
            await peer.change("WRONG OWNER", changeOwnerDuringRead: true)
        } else {
            await peer.end(stale: ending == "stale")
        }
        try await eventually { store.error != nil }
        #expect(document.text == "local\nPEER\n")
        #expect(document.snapshot?.content == "one\nPEER\n")
        #expect(document.dirty)
        #expect(store.selection == "other.txt")
        await peer.end(stale: false)
    }

    @Test func choosingAPeerDefinitionReturnsToTheCallLine() async throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let query = RegistryQuery { args, cwd in
            guard cwd == nil, args == ["--machine", "peer", "--repo-id", "plan", "task", "symbols", "TEST-1", "--json", "--", "read"]
            else { throw RegistryQueryError("Expected declaration lookup on the peer") }
            let fixture = try JSONSerialization.jsonObject(with: data) as! [String: Any]
            return String(decoding: try JSONSerialization.data(withJSONObject: fixture["symbols"]!), as: UTF8.self)
        }.onMachine("peer", repository: "plan")
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer/checkout", query: query)
        store.selectFile("Calls.swift")
        let doc = store.document("Calls.swift")
        await doc.reconcile(snapshot("func run() {\n  read()\n}\n", path: doc.path))
        await store.jumps.search(.definition("read"), line: 2, in: store)?.value
        let peek = try #require(store.jumps.peek)
        #expect(peek.candidates.map(\.position.path) == ["src/A.swift", "src/B.swift"])
        #expect(store.selection == doc.path && store.jumps.back.isEmpty)
        let list = FileSymbolPeek(store: store, peek: peek)
        #expect(try list.inspect().find(text: "A.read").string() == "A.read")
        #expect(try list.inspect().find(text: "B.read").string() == "B.read")
        #expect(try list.inspect().find(text: "src/B.swift:7").string() == "src/B.swift:7")
        store.jumps.dismiss()
        #expect(store.jumps.back.isEmpty)
        await store.jumps.search(.definition("read"), line: 2, in: store)?.value
        let reopened = FileSymbolPeek(store: store, peek: try #require(store.jumps.peek))
        try reopened.inspect().find(ViewType.Button.self, where: { button in
            (try? button.labelView().find(text: "B.read")) != nil
        }).tap()
        #expect(store.selection == "src/B.swift" && store.visit?.line == 7)
        store.jumps.step(backwards: true, in: store)
        #expect(store.selection == doc.path && store.visit?.line == 2 && store.jumps.back.isEmpty)
        store.jumps.step(backwards: false, in: store)
        #expect(store.selection == "src/B.swift" && store.visit?.line == 7)
    }

    @Test func aSinglePeerReferenceWaitsForAPickAndOpensTheMatchedBytes() async throws {
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer/checkout", query: RegistryQuery { args, cwd in
            guard cwd == nil, args == ["--machine", "peer", "--repo-id", "plan", "task", "grep", "TEST-1",
                                      "--json", "--whole-word", "--", "read"] else {
                throw RegistryQueryError("Expected whole-word grep on the peer")
            }
            return #"{"hits":[{"path":"Call.swift","line":2,"start":5,"end":9,"revision":"base","excerpt":"😀 read()"}],"truncated":false}"#
        }.onMachine("peer", repository: "plan"))
        store.selectFile("Call.swift")
        let doc = store.document("Call.swift")
        await doc.reconcile(snapshot("readMore()\n😀 read()\n", path: doc.path))
        var chord = FileJumps.Chord()
        #expect(chord.press("g", identifier: "read") == nil && chord.pending)
        let lookup = chord.press("r", identifier: "read")
        await store.jumps.search(try #require(lookup), line: 2, in: store)?.value
        let peek = try #require(store.jumps.peek)
        #expect(peek.lookup.title == "References to read" && peek.candidates.count == 1)
        #expect(store.visit == nil && store.jumps.back.isEmpty)
        let list = FileSymbolPeek(store: store, peek: peek)
        try list.inspect().find(ViewType.Button.self, where: { button in
            (try? button.labelView().find(text: "😀 read()")) != nil
        }).tap()
        #expect(store.jumps.peek == nil && store.visit?.line == 2)
        #expect(store.visit?.hit?.range(in: "😀 read()") == NSRange(location: 3, length: 4))
    }

    @Test func anIncompleteLookupCannotTakeTheReaderToAUniqueDefinition() async throws {
        let query = RegistryQuery { args, _ in
            switch args.last {
            case "partial":
                return #"{"hits":[{"path":"b.py","name":"partial","kind":"def","container":null,"line":1}],"truncated":true}"#
            case "missing": return #"{"hits":[],"truncated":false}"#
            default: throw RegistryQueryError("Peer disconnected")
            }
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: query)
        store.selectFile("a.py")
        await store.document("a.py").reconcile(snapshot("partial()\n", path: "a.py"))
        for (name, message) in [("partial", "Results truncated"), ("missing", "No declarations found"), ("offline", "Peer disconnected")] {
            await store.jumps.search(.definition(name), line: 1, in: store)?.value
            #expect(store.jumps.peek?.message?.contains(message) == true)
            #expect(store.selection == "a.py" && store.jumps.back.isEmpty)
        }
    }

    @Test func readerIdentifiesTheClickedWordAfterUnicode() {
        let font = NSFont(name: Typography.monoFace, size: Typography.monoSize)
            ?? .monospacedSystemFont(ofSize: Typography.monoSize, weight: .regular)
        let prefix = NSAttributedString(string: "😀. ", attributes: [.font: font]).size().width
        #expect(TaskCodeReader.word(in: "😀. read()", at: CGPoint(x: prefix + 1, y: 0)) == "read")
        #expect(TaskCodeReader.word(in: "read()", at: CGPoint(x: 1000, y: 0)) == nil)
    }

    @Test func outlineAndLocalDefinitionReadTheLiveSwiftBuffer() async throws {
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer", query: RegistryQuery { _, _ in
            throw RegistryQueryError("A same-file declaration should resolve in the draft")
        })
        store.autosave = false
        store.selectFile("Draft.swift")
        let doc = store.document("Draft.swift")
        await doc.reconcile(snapshot("struct Reader {}", path: doc.path))
        let draft = "struct Reader {\n func lookup() {}\n}\nReader().lookup()"
        doc.editor.insertText(draft, replacementRange: NSRange(location: 0, length: doc.text.utf16.count))
        await store.jumps.search(.outline, line: 4, in: store)?.value
        #expect(store.jumps.peek?.candidates.map(\.label) == ["Reader", "Reader.lookup"])
        #expect(doc.outline?.enclosing(2)?.container == "Reader")
        await store.jumps.search(.definition("lookup"), line: 4, in: store)?.value
        #expect(store.visit?.line == 2 && store.jumps.peek == nil && doc.text == draft)
        store.jumps.step(backwards: true, in: store)
        #expect(store.visit?.line == 4)
    }

    @Test func aDirtyDraftDropsItsStaleDiskDeclarationsFromTheCheckoutLookup() async throws {
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer", query: RegistryQuery { args, _ in
            guard args.last == "lookup" else { throw RegistryQueryError("Expected a declaration lookup") }
            return #"{"hits":[{"path":"Draft.swift","name":"lookup","kind":"func","container":null,"line":1},{"path":"Other.swift","name":"lookup","kind":"func","container":"Reader","line":3}],"truncated":false}"#
        })
        store.autosave = false
        store.selectFile("Draft.swift")
        let doc = store.document("Draft.swift")
        await doc.reconcile(snapshot("func lookup() {}", path: doc.path))
        doc.editor.insertText("lookup()", replacementRange: NSRange(location: 0, length: doc.text.utf16.count))
        await store.jumps.search(.definition("lookup"), line: 1, in: store)?.value
        #expect(store.selection == "Other.swift" && store.visit?.line == 3 && store.jumps.peek == nil)
    }

    @Test(arguments: [TaskFilePicker.Mode.files, .grep])
    func pickerPreviewsAnUnopenedPeerFileWithoutChangingTheDraftOrTabs(mode: TaskFilePicker.Mode) async throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let nested = "src/nested/Reader.swift"
        let query = RegistryQuery { args, cwd in
            guard cwd == nil, args.starts(with: ["--machine", "peer", "--repo-id", "plan", "task"]) else {
                throw RegistryQueryError("Expected the owning Machine")
            }
            let fixture = try JSONSerialization.jsonObject(with: data) as! [String: Any]
            let value: Any
            switch Array(args.dropFirst(5)) {
            case ["find", "TEST-1", "--json"]: value = fixture["find"]!
            case ["grep", "TEST-1", "--json", "--", "Reader"]: value = fixture["grep"]!
            case ["file", "TEST-1", nested, "--json"]:
                var file = fixture["file"]! as! [String: Any]
                file["path"] = nested
                file["content"] = "// Reader\r\n😀 Reader Reader\r\n"
                file["revision"] = "d49b198fdd7a62d73eca883aa2fb88b47c02de0dd141509e8741bf43e271d1aa"
                value = file
            default: throw RegistryQueryError("Unexpected read")
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }.onMachine("peer", repository: "plan")
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer/checkout", query: query)
        store.autosave = false
        store.selectFile("notes.md", mode: .edit)
        let draft = store.document("notes.md")
        await draft.reconcile(snapshot("saved", path: "notes.md"))
        draft.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 5))
        let tabs = store.tabs
        store.chooseFile(mode)
        let picker = try #require(store.filePicker)
        await picker.load()
        let field = try TaskFilePickerView(store: store, picker: picker).inspect().find(PaletteSearchField.self).actualView()
        switch mode {
        case .changed: Issue.record("Expected a checkout search"); return
        case .files:
            #expect(picker.selection?.path == "notes.md")
            field.move(1)
            field.move(1)
            #expect(picker.selection?.path == nested)
            picker.text = "nstrdr"
            #expect(picker.results.map(\.path) == [nested])
        case .grep:
            #expect(picker.remoteSearch == nil && picker.selection == nil && !picker.loading)
            picker.text = "Reader"
            await picker.search("Reader")
            field.move(1)
            field.move(1)
            let hit = try #require(picker.selection?.hit)
            #expect(hit.line == 2)
            #expect(hit.range(in: "😀 Reader Reader") == NSRange(location: 10, length: 6))
        }
        await picker.loadPreview(nested, draft: nil)
        guard case .text("// Reader\r\n😀 Reader Reader\r\n", let revision) = picker.preview else {
            Issue.record("Expected the nested file's source")
            return
        }
        #expect(store.tabs == tabs && store.tree.rows().isEmpty && store.documents[nested] == nil)
        #expect(store.selection == "notes.md" && store.mode == .edit)
        #expect(draft.dirty && draft.text == "draft")
        field.submit()
        if let hit = picker.selection?.hit {
            #expect(hit.revision == revision)
            await store.loadFile()
            let reader = try TaskFilesView(store: store, checkoutBase: nil).inspect().find(TaskCodeReader.self).actualView()
            #expect(reader.location?.hit == hit && reader.revision == hit.revision)
        }
        #expect(store.filePicker == nil)
        #expect(store.selection == nested && store.mode == .read)
    }

    @Test func recordedLocalCheckoutUsesTheResidentServerForFileSearch() async throws {
        let place = TaskLocation.recorded(taskID: "owner-task", checkout: "/checkout")
        let owner = TaskLocationObservation(request: "opened", repositoryID: "plan", taskID: "portable",
            machineID: "local", observedAt: 1, location: place)
        let workspace = fixtureWork(.init(machineId: "local", worktree: "/checkout"))
        let content = PaneContent.browser(taskId: "portable", tabs: [.files], selected: "files", execution: owner)
        let query = RegistryQuery(serve: { read in
            guard case .taskFilesFind("owner-task", nil, nil) = read else { throw RegistryQueryError("Expected checkout inventory") }
            return ReadOutput(#"{"paths":["src/local.rs"],"scratch":[],"latest_scratch_page":null,"truncated":false}"#)
        }) { args, _ in
            guard args.starts(with: ["--repo-id", "plan", "task", "location"]), let flag = args.firstIndex(of: "--request") else {
                throw RegistryQueryError("Only location validation uses the CLI")
            }
            let reading = TaskLocationObservation(request: args[flag + 1], repositoryID: "plan", taskID: "portable",
                machineID: "local", observedAt: 2, location: place)
            return String(decoding: try JSONEncoder().encode([reading]), as: UTF8.self)
        }
        let store = try workspace.files(for: content, task: nil, query: query)
        store.chooseFile()
        let picker = try #require(store.filePicker)
        await picker.load()
        #expect(picker.results.map(\.path) == ["src/local.rs"])
        #expect(picker.error == nil)
    }

    @Test func largeCheckoutPickerKeepsTheLatestSearchWhenAnEarlierReplyArrives() async throws {
        let (started, start) = AsyncStream<Void>.makeStream()
        let (released, release) = AsyncStream<Void>.makeStream()
        let query = RegistryQuery { args, _ in
            guard args.starts(with: ["task", "find", "TEST-1", "--json"]) else { throw RegistryQueryError("Expected find") }
            let paths: [String]
            switch args.last {
            case "before":
                start.yield()
                for await _ in released { break }
                paths = ["before.txt"]
            case "after": paths = ["nested/after.txt"]
            default: paths = ["first.txt"]
            }
            let value: [String: Any] = ["paths": paths, "scratch": [], "latest_scratch_page": NSNull(),
                                        "truncated": args.last == "--json"]
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/checkout", query: query)
        let picker = TaskFilePicker(store: store)
        await picker.load()
        #expect(picker.truncated)
        picker.text = "before"
        let before = try #require(picker.remoteSearch)
        let pending = Task { await picker.search(before) }
        for await _ in started { break }
        picker.text = "after"
        #expect(picker.loading && picker.selection == nil)
        await picker.search("after")
        release.yield()
        await pending.value
        #expect(!picker.truncated && !picker.loading)
        #expect(picker.results.map(\.path) == ["nested/after.txt"])
        #expect(picker.selection?.path == "nested/after.txt" && store.selection == nil)
    }

    @Test func presentationsRetainDraftsWhileCleanJumpDestinationsExpire() async throws {
        let store = TaskFilesStore(issue: "task", cwd: "/checkout", query: RegistryQuery { _, _ in "{}" })
        store.autosave = false
        store.openPresentation(.init(path: "draft.md"))
        let draft = store.document("draft.md")
        await draft.reconcile(snapshot("original", path: draft.path))
        draft.editor.insertText("edited", replacementRange: NSRange(location: 0, length: 8))
        store.jumps.readingLine = 7
        for n in 1...15 {
            store.openPresentation(.init(path: "page-\(n).md"))
        }
        #expect(store.jumps.back.first == FilePosition(path: "draft.md", line: 7))
        #expect(store.tabs.count == 13 && store.tabs.first?.path == "draft.md")
        store.closeTab(try #require(store.tabs.first).id)
        #expect(store.documents[draft.path] === draft && store.openAndScratch.contains(draft.path))
        #expect(draft.text == "edited" && store.selection == "page-15.md")
    }

    @Test func ignoredScratchWritesOfferThePageWithoutOpeningItOrReadingAComparison() async throws {
        let reads = OSAllocatedUnfairLock(initialState: [String]())
        let query = RegistryQuery { args, _ in
            let command = args[args.firstIndex(of: "task")! + 1]
            reads.withLock { $0.append(command) }
            guard command == "find" else { throw RegistryQueryError("Unexpected read") }
            return #"{"paths":[],"scratch":["scratch/page.html"],"latest_scratch_page":"scratch/page.html","truncated":false}"#
        }.onMachine("peer", repository: "plan")
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/peer/checkout", query: query)
        store.selectFile("src/reader.swift", mode: .edit)
        let tabs = store.tabs
        store.invalidate([], ignoredScratch: ["scratch/page.html", "scratch/page.html"])
        try await eventually { store.browser.updated == ["scratch/page.html"] }
        #expect(store.mode == .edit && store.tabs == tabs && store.selection == "src/reader.swift")
        #expect(store.scratchPaths == ["scratch/page.html"] && store.documents.isEmpty)
        #expect(reads.withLock { $0 } == ["find"])
    }

    @Test func initialScratchChoiceSurvivesAMissingComparisonAndIsNeverRepeated() async throws {
        let query = RegistryQuery { args, _ in
            guard args.contains("find") else { throw RegistryQueryError("Missing base") }
            return #"{"paths":[],"scratch":["scratch/a.md","scratch/z.html"],"latest_scratch_page":"scratch/z.html","truncated":false}"#
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/checkout", query: query)
        await store.enterFiles()
        #expect(store.selection == "scratch/z.html" && store.workingTree.error == "Missing base")
        store.closeTab(try #require(store.tabs.first).id)
        await store.enterFiles()
        #expect(store.selection == nil && store.documents.isEmpty && store.filePicker == nil)
    }

    @Test func readModeRendersHTMLAsAPageAndMarkdownAsBlocks() async throws {
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { _, _ in "{}" })
        #expect(store.mode == .read)
        for (path, text) in [("scratch/explore.html", "<h1>Three visions</h1>"), ("scratch/plan.md", "# Plan")] {
            let document = store.document(path)
            await document.reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files",
                path: path, content: text, data: nil, state: .text, revision: text, readOnlyReason: nil, sizeBytes: UInt64(text.utf8.count)))
            store.selection = path
            #expect(document.retainedEditor == nil)
        }
        let page = TaskFileReadingView(document: store.document("scratch/explore.html"), baseURL: nil)
        #expect(throws: Never.self) { try page.inspect().find(viewWithAccessibilityIdentifier: "files-rendered-html") }
        let note = TaskFileReadingView(document: store.document("scratch/plan.md"), baseURL: nil)
        #expect(try note.inspect().find(MarkdownBlocks.self).actualView().source == "# Plan")

        let path = "scratch/plan.md"
        let hit = TaskGrepHit(path: path, line: 1, start: 2, end: 6, revision: "# Plan", excerpt: "# Plan")
        store.openMatch(hit)
        let files = TaskFilesView(store: store, checkoutBase: nil)
        #expect(try files.inspect().find(TaskCodeReader.self).actualView().location?.hit == hit)
        store.selectFileFromInput(path)
        #expect(try files.inspect().find(MarkdownBlocks.self).actualView().source == "# Plan")
    }

    @Test func codeOpensInReadAndKeepsItsDraftAndEditorAcrossFiles() async throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let location = fixtureWorkspace("/fixture")
        let workspace = fixtureWork(registry, location)
        let pane = try #require(workspace.multiplexer.showBrowser(.files, location: location))
        let query = RegistryQuery { _, _ in
            #"{"recoveries":[],"issue_identifier":"TEST-1","task_id":"task-files","path":"Store.swift","content":"// 😀\r\nstruct Store {}","data":null,"state":"text","revision":"1","read_only_reason":null,"size_bytes":24}"#
        }
        let store = try workspace.files(for: pane.content, task: nil, query: query)
        let model = WorkModel(query: query)
        let context = DesktopActionContext(model: model, workspaces: registry, workspace: workspace, pane: pane)
        let (edit, _) = try #require(DesktopAction.binding(characters: "e", modifiers: [.command, .control]))
        store.autosave = false
        store.selectFileFromInput("Store.swift")
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        await document.reconcile(snapshot("// 😀\r\nstruct NewStore {}", path: document.path))
        #expect(store.mode == .read)
        #expect(document.retainedEditor == nil)
        #expect(document.saveMessage == "Updated from disk")
        let view = TaskFilesView(store: store, checkoutBase: nil)
        let reader = try view.inspect().find(TaskCodeReader.self).actualView()
        let lines = CodeLine.read(reader.text, grammar: Grammar.named(reader.path))
        #expect(lines.map(\.text) == ["// 😀", "struct NewStore {}"])
        #expect(lines[1].highlights.contains { $0.ink == .keyword && $0.range == NSRange(location: 0, length: 6) })

        func bracket(_ key: String) throws -> DesktopAction {
            try #require(DesktopAction.binding(characters: key, modifiers: .command, context: context)?.0)
        }
        #expect(try bracket("[").name == "back")
        store.jumps.record(.init(path: "Store.swift", line: 2), from: .init(path: "Store.swift", line: 1))
        #expect(try bracket("[").name == "files-back")
        try bracket("[").perform(context)
        #expect(try bracket("[").name == "back")
        #expect(try bracket("]").name == "files-forward")
        let (grep, _) = try #require(DesktopAction.binding(characters: "f", modifiers: [.shift, .command]))
        store.page = .changes
        #expect(!edit.available(context) && !grep.available(context))
        store.page = .files
        try grep.perform(context)
        #expect(store.filePicker?.mode == .grep)
        store.filePicker = nil
        #expect(edit.available(context))
        try edit.perform(context)
        let editor = document.editor
        editor.insertText("let draft = 1", replacementRange: NSRange(location: 0, length: editor.string.utf16.count))
        editor.setSelectedRange(NSRange(location: 4, length: 5))
        store.selectFileFromInput("notes.txt")
        #expect(store.mode == .read)
        store.selectFileFromInput("Store.swift")
        #expect(store.mode == .edit)
        try edit.perform(context)
        #expect(store.mode == .read)
        #expect(try view.inspect().find(TaskCodeReader.self).actualView().text == "let draft = 1")
        try edit.perform(context)
        #expect(document.editor === editor)
        #expect(editor.selectedRange() == NSRange(location: 4, length: 5))
        editor.undoManager?.undo()
        #expect(document.text == "// 😀\r\nstruct NewStore {}")
    }

    @Test func findInReadStepsOccurrencesAndPromotesTheQueryOnlyFromFiles() async throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let location = fixtureWorkspace("/fixture")
        let workspace = fixtureWork(registry, location)
        let pane = try #require(workspace.multiplexer.showBrowser(.files, location: location))
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Search stays in the displayed file") }
        let files = try workspace.files(for: pane.content, task: nil, query: query)
        let model = WorkModel(query: query)
        func context(_ pane: PaneState?) -> DesktopActionContext {
            DesktopActionContext(model: model, workspaces: registry, workspace: workspace, pane: pane)
        }
        files.selectFile("Read.swift")
        let document = files.document("Read.swift")
        await document.reconcile(snapshot("// 😀 Read read\r\nfunc read() {}", path: document.path))
        let binding = try #require(DesktopAction.binding(characters: "f", modifiers: .command, context: context(pane)))
        try binding.0.perform(context(pane))
        let find = try #require(files.find)
        let view = TaskFilesView(store: files, checkoutBase: nil)
        let field = try view.inspect().find(PaletteSearchField.self).actualView()
        field.text = "read"
        let reader = try view.inspect().find(TaskCodeReader.self).actualView()
        await find.search(lines: CodeLine.read(reader.text, grammar: nil))
        #expect(find.label == "1 of 3")
        #expect(find.current?.range == NSRange(location: 6, length: 4))
        field.submit()
        #expect(find.label == "2 of 3" && find.current?.range.location == 11)
        field.submitBackwards?()
        field.submitBackwards?()
        #expect(find.label == "3 of 3" && find.current?.cell == .line(2))
        #expect(document.retainedEditor == nil)
        let grep = try #require(DesktopAction.binding(characters: "f", modifiers: [.shift, .command], context: context(pane)))
        try grep.0.perform(context(pane))
        #expect(files.filePicker?.remoteSearch == "read")
        let pickerField = try view.inspect().find(TaskFilePickerView.self).find(PaletteSearchField.self).actualView()
        pickerField.cancel()
        #expect(files.find === find && files.filePicker == nil)
        let terminal = try #require(workspace.multiplexer.sessionPane)
        #expect(DesktopAction.binding(characters: "f", modifiers: .command, context: context(terminal)) == nil)
    }

    @Test func readAndDiffCommentsEnterUnsentInputTogether() async throws {
        let files = try twoFileStore()
        files.selectFile("a.swift")
        let document = files.document("a.swift")
        await document.reconcile(snapshot("uno\nsecond\nthird", path: document.path))
        let view = TaskFilesView(store: files, checkoutBase: nil)
        let reader = try view.inspect().find(TaskCodeReader.self).actualView()
        let lines = CodeLine.read(reader.text, grammar: nil)
        let first = try reader.row(0, in: lines).inspect().find(ReviewGutter.self).actualView()
        first.select()
        files.comments.select(path: document.path, source: .file(draft: false), line: 2, extend: true)
        let editor = try reader.row(1, in: lines).inspect().find(ReviewCommentEditor.self)
        files.comments.draft = "Read these together"
        try editor.find(button: "Comment").tap()
        #expect(files.comments.items.first?.anchor == ["uno", "second"])
        await document.reconcile(snapshot("above\nuno\nsecond\nthird", path: document.path))
        #expect(files.comments.items.first?.selection.range == "L2-3")
        await files.diffReview.loadReview()
        let file = try #require(files.diffReview.shownReview?.file(document.path))
        let diff = ReviewStream(store: files, reader: .changes, files: [file], split: false, addToConversation: nil)
        #expect(try diff.inspect().findAll(ReviewCommentRow.self).isEmpty)
        let gutter = try #require(try diff.inspect().findAll(ReviewGutter.self).first).actualView()
        gutter.select()
        files.comments.draft = "Explain this change"
        try diff.inspect().find(ReviewCommentEditor.self).find(button: "Comment").tap()
        var input: [String] = []
        func send(available: Bool) throws {
            let strip = ReviewCommentsStrip(store: files, isFocused: true) {
                if available { input.append($0) }
                return available
            }
            try strip.inspect().find(viewWithAccessibilityIdentifier: "changes-send").button().tap()
        }
        try send(available: false)
        #expect(files.comments.items.count == 2)
        try send(available: true)
        #expect(files.comments.items.isEmpty && input.count == 1)
        let message = try #require(input.first)
        #expect(message.contains("`a.swift:L2-3` (working file) Read these together"))
        #expect(message.contains("`a.swift:L1` (after, " + files.diffReview.revision + ") Explain this change"))
        #expect(!message.contains("\n"))
    }

    @Test func findCountsOnlyDisplayedDiffSidesAndRenderedMarkdownCells() async throws {
        let files = try twoFileStore()
        await files.diffReview.loadReview()
        let file = try #require(files.diffReview.review?.files.first)
        let stream = ReviewStream(store: files, reader: .file(file.path), files: [file], split: false, addToConversation: nil)
        let find = FileFind()
        find.query = "nine"
        await find.search(stream.searchCells(in: file))
        #expect(find.label == "No matches")
        files.diffReview.reviewLayout = .unified
        await find.search(stream.searchCells(in: file))
        #expect(find.current?.cell == .diff(row: "a.swift#o9", side: .old, line: 9))
        let blocks = MarkdownBlocks.blocks("| key | value |\n|---|---|\n| nine | **nine** |")
        await find.search(blocks.enumerated().flatMap { $0.element.searchCells(index: $0.offset) })
        #expect(find.label == "1 of 2")
        find.step(1)
        #expect(find.current?.cell == .markdown(block: 1, cell: 1))
    }

    @Test(.requiresDisplay) func htmlFindReadsVisibleTextAcrossMarkupAndKeepsPageState() async throws {
        let page = WebPage()
        let html = "<p>😀 a<b>+b</b></p><p hidden>a+b</p><p>a+B</p><script>window.loads = (window.loads ?? 0) + 1</script>"
        for try await _ in page.load(html: html, baseURL: URL(fileURLWithPath: "/")) {}
        let count = try await page.callJavaScript(TaskHTMLView.searchScript, arguments: ["needle": "a+b"], contentWorld: .defaultClient)
        #expect(count as? Int == 2)
        _ = try await page.callJavaScript(TaskHTMLView.revealScript, arguments: ["needle": "a+b", "index": 1], contentWorld: .defaultClient)
        let marked = try await page.callJavaScript("return [...CSS.highlights.get('lf-find')][0].toString()", contentWorld: .defaultClient)
        #expect(marked as? String == "a+B")
        let isolated = try await page.callJavaScript("return window.loads === 1 && window.fileFind === undefined", contentWorld: .page)
        #expect(isolated as? Bool == true)
    }

    @Test func aRecoveredDraftStaysReadOnlyWhenTheFileHasDisappeared() async {
        let document = TaskFileDocument(path: "note.txt")
        let draft = WorkFileDraft(location: .init(machineID: "local", cwd: "/fixture"), path: document.path,
                                  baseRevision: "1", baseContent: "saved", text: "mine")
        document.restore(draft, task: "TEST-1")
        await document.reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files",
            path: document.path, content: nil, data: nil, state: .missing, revision: nil,
            readOnlyReason: nil, sizeBytes: 0))
        #expect(document.text == "mine")
        #expect(document.external?.state == .missing)
        #expect(document.retainedEditor == nil)
        #expect(!document.canEdit)
        #expect(!document.editor.isEditable)
        #expect(!document.canSave)
    }

    @Test func diskWinsOverlapKeepsOtherLocalEditsAndUndoNeverRevertsDisk() async {
        let document = TaskFileDocument(path: "note.txt")
        await document.reconcile(snapshot("one\ntwo\nthree\n"))
        document.editor.insertText("local", replacementRange: NSRange(location: 0, length: 3))
        document.editor.insertText("THREE", replacementRange: NSRange(location: 10, length: 5))
        document.editor.setSelectedRange(NSRange(location: 12, length: 1))
        await document.reconcile(snapshot("disk\ntwo\nthree\n"))
        #expect(document.text == "disk\ntwo\nTHREE\n")
        #expect(document.external == nil)
        #expect(document.canSave)
        #expect(document.editor.selectedRange() == NSRange(location: 11, length: 1))
        document.editor.undoManager?.undo()
        #expect(document.text == "disk\ntwo\nthree\n")
        #expect(!document.editor.undoManager!.canUndo)
        document.editor.undoManager?.redo()
        #expect(document.text == "disk\ntwo\nTHREE\n")
    }

    /// The editor colors through the file's grammar, off the main thread, and
    /// colors again after typing: wrapping code in a block comment turns it
    /// comment-colored.
    @Test func theEditorColorsByGrammarAsYouType() async throws {
        let document = TaskFileDocument(path: "Store.swift")
        await document.reconcile(snapshot("struct Store {}\nlet count = 42\n"))
        let theme = ThemeDefinition.dark
        document.applyTheme(theme)
        let inks = SyntaxInks.of(theme)
        func ink(at offset: Int) -> UInt32? {
            guard let color = document.editor.textStorage?.attribute(.foregroundColor, at: offset, effectiveRange: nil)
                    as? NSColor, let rgb = color.usingColorSpace(.sRGB) else { return nil }
            return UInt32(rgb.redComponent * 255 + 0.5) << 16 | UInt32(rgb.greenComponent * 255 + 0.5) << 8
                | UInt32(rgb.blueComponent * 255 + 0.5)
        }
        func settles(_ offset: Int, to expected: SyntaxInk) async -> Bool {
            for _ in 0..<500 {
                if ink(at: offset) == inks.colors[expected]?.hex { return true }
                try? await Task.sleep(for: .milliseconds(10))
            }
            return false
        }
        #expect(await settles(0, to: .keyword))
        #expect(await settles(28, to: .constant))
        // An unclosed `/*` is a parse error, not a comment; closing it is.
        document.editor.insertText("/*", replacementRange: NSRange(location: 0, length: 0))
        document.editor.insertText("*/", replacementRange: NSRange(location: 34, length: 0))
        #expect(await settles(2, to: .comment))
        #expect(await settles(30, to: .comment))
    }

    @Test func sameRevisionAccessChangePreservesDraftSelectionAndUndo() async {
        let document = TaskFileDocument(path: "note.txt")
        let original = snapshot("original")
        await document.reconcile(original)
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 8))
        document.editor.setSelectedRange(NSRange(location: 2, length: 1))
        let readOnly = TaskFileSnapshot(recoveries: [], issueIdentifier: original.issueIdentifier,
            taskId: original.taskId, path: original.path, content: original.content, data: nil, state: .text,
            revision: original.revision, readOnlyReason: "Symlink is read-only", sizeBytes: original.sizeBytes)
        await document.reconcile(readOnly)
        #expect(document.text == "draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 1))
        #expect(document.editor.undoManager?.canUndo == true)
        #expect(!document.editor.isEditable)
        #expect(!document.canSave)
        #expect(document.readOnlyReason == "Symlink is read-only")
        await document.reconcile(original)
        #expect(document.editor.isEditable)
        #expect(document.canSave)
        document.editor.undoManager?.undo()
        #expect(document.text == "original")
    }

    @Test(arguments: [false, true])
    func symlinkAccessCancelsQueuedAutosaveAndManualSave(parentLink: Bool) async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let directory = root.appendingPathComponent("directory")
        let targetDirectory = root.appendingPathComponent("target")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: targetDirectory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("note.txt")
        let target = targetDirectory.appendingPathComponent("note.txt")
        try Data("original".utf8).write(to: file)
        try Data("original".utf8).write(to: target)
        let query = RegistryQuery(runWithInput: { _, _, draft in
            try Data(draft.utf8).write(to: target)
            return "{}"
        }, run: { _, _ in
            var value = try fileJSON(file)
            if (try? FileManager.default.destinationOfSymbolicLink(atPath: (parentLink ? directory : file).path)) != nil {
                value["read_only_reason"] = "Files reached through a symlink are read-only."
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        let store = TaskFilesStore(issue: "TEST-1", cwd: root.path, query: query)
        store.autosave = true
        store.selection = "directory/note.txt"
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 8))
        await Task.yield()
        try FileManager.default.removeItem(at: parentLink ? directory : file)
        try FileManager.default.createSymbolicLink(at: parentLink ? directory : file,
                                                  withDestinationURL: parentLink ? targetDirectory : target)
        await store.loadFile()
        await document.save(issue: store.issue, query: query)
        try await Task.sleep(for: .milliseconds(2200))
        #expect(try String(contentsOf: target, encoding: .utf8) == "original")
        #expect(document.text == "draft")
        #expect(!document.canSave)
        #expect(!document.editor.isEditable)
    }

    @Test func mergePreservesUnicodeLineEndingsAndSeveralSeparatedEdits() {
        #expect(TaskTextEdit.merge(base: "a\nb\nc", local: "A\nB\nc", disk: "X\nb\nc") == "X\nB\nc")
        #expect(TaskTextEdit.merge(base: "a b c", local: "A b C", disk: "X b c") == "X b C")
        #expect(TaskTextEdit.merge(base: "abc", local: "localabc", disk: "XYZ") == "localXYZ")
        #expect(TaskTextEdit.merge(base: "abc", local: "XYZ", disk: "diskabc") == "diskXYZ")
        #expect(TaskTextEdit.merge(base: "abc def ghi", local: "ABC def ghi", disk: "abc def GHI") == "ABC def GHI")
        #expect(TaskTextEdit.merge(base: "😀 a\r\nb\r\nc\r\nd\r\n", local: "😀 A\r\nb\r\nc\r\nD\r\n",
                                   disk: "😀 a\r\nB\r\nc\r\nd\r\n") == "😀 A\r\nB\r\nc\r\nD\r\n")
        for (old, new) in [("a", "a\n"), ("a\n", "a"), ("😀", "😁"), ("", "x"), ("a\nb", "b") ] {
            #expect(TaskTextEdit.applying(TaskTextEdit.between(old, new), to: old) == new)
        }
    }

    @Test func compositionDefersIncomingChangesAndSave() async throws {
        let document = TaskFileDocument(path: "note.txt")
        await document.reconcile(snapshot("one\ntwo\n"))
        document.editor.setMarkedText("あ", selectedRange: NSRange(location: 1, length: 0),
                                      replacementRange: NSRange(location: 0, length: 3))
        await document.reconcile(snapshot("one\nTWO\n"))
        #expect(document.editor.hasMarkedText())
        #expect(!document.canSave)
        #expect(!document.text.contains("TWO"))
        document.editor.insertText("あ", replacementRange: document.editor.markedRange())
        for _ in 0..<100 {
            if document.text.contains("TWO") { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(document.text == "あ\nTWO\n")
        #expect(!document.editor.hasMarkedText())
    }

    @Test(.requiresDisplay) func filesystemEventsFollowReplacementRapidWritesDeletionAndBothSaveModes() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("note.txt")
        try Data("one\ntwo\n".utf8).write(to: file)
        let query = RegistryQuery(runWithInput: { _, _, text in
            try Data(text.utf8).write(to: file, options: .atomic)
            let value: [String: Any] = ["published": true, "message": "Saved",
                                        "file": try fileJSON(file)]
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }, run: { args, _ in
            let value: [String: Any]
            if args[1] == "changes" {
                throw RegistryQueryError("Task has no active PR")
            } else if args[1] == "files" {
                value = ["path": "", "entries": [["path": "note.txt", "kind": "file"]]]
            } else { value = try fileJSON(file) }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        let store = TaskFilesStore(issue: "TEST-1", cwd: directory.path, query: query)
        store.autosave = false
        store.selection = "note.txt"
        store.observeFiles()
        await store.refresh()
        #expect(store.diffReview.reviewReading.error == "Task has no active PR")
        #expect(store.error == nil)
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 600, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = document.scroll
        defer { window.contentView = nil }
        let began = ContinuousClock.now
        try Data("ONE\ntwo\n".utf8).write(to: file, options: .atomic)
        try await eventually { document.text == "ONE\ntwo\n" }
        document.scroll.layoutSubtreeIfNeeded()
        if let bitmap = document.scroll.bitmapImageRepForCachingDisplay(in: document.scroll.bounds) {
            document.scroll.cacheDisplay(in: document.scroll.bounds, to: bitmap)
        }
        print("Task files external event to native bitmap: \(began.duration(to: .now)) · 8 bytes · fixture transport")
        for index in 0..<5 { try Data("ONE\nwrite \(index)\n".utf8).write(to: file) }
        try await eventually { document.text == "ONE\nwrite 4\n" }
        document.editor.insertText("local", replacementRange: NSRange(location: 0, length: 3))
        try Data("ONE\nexternal\n".utf8).write(to: file, options: .atomic)
        try await eventually { document.text == "local\nexternal\n" }
        try await Task.sleep(for: .milliseconds(2200))
        #expect(try String(contentsOf: file, encoding: .utf8) == "ONE\nexternal\n")
        await document.save(issue: store.issue, query: query)
        #expect(try String(contentsOf: file, encoding: .utf8) == "local\nexternal\n")
        await store.refresh()
        store.autosave = true
        document.editor.insertText("auto", replacementRange: NSRange(location: 0, length: 5))
        try await eventually { (try? String(contentsOf: file, encoding: .utf8)) == "auto\nexternal\n" }
        try FileManager.default.moveItem(at: file, to: directory.appendingPathComponent("renamed.txt"))
        try await eventually { document.snapshot?.state == .missing }
        #expect(document.text == "")
        try Data("recreated\n".utf8).write(to: file)
        try await eventually { document.text == "recreated\n" }
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 0))
        try FileManager.default.removeItem(at: file)
        try await eventually { document.external?.state == .missing }
        #expect(document.text == "draftrecreated\n")
        #expect(!document.canSave)
    }

    // MARK: Checkout tree

    /// `lf task files` over a fixed checkout; `failing` directories throw until cleared.
    private final class CheckoutFixture: @unchecked Sendable {
        let lock = OSAllocatedUnfairLock(initialState: (reads: [[String]], cwds: [String?], failing: Set<String>)(([], [], [])))
        let pages: [String: [(entries: [(String, String)], next: String?)]]

        init(_ pages: [String: [(entries: [(String, String)], next: String?)]]) { self.pages = pages }

        var reads: [[String]] { lock.withLock { $0.reads } }
        func fail(_ directory: String, _ failing: Bool) {
            lock.withLock { if failing { $0.failing.insert(directory) } else { $0.failing.remove(directory) } }
        }

        func read(_ args: [String], cwd: String?) throws -> String {
            lock.withLock { $0.reads.append(args); $0.cwds.append(cwd) }
            let tail = Array(args.drop { $0 != "task" })
            guard tail.count >= 4, tail[1] == "files" else { throw RegistryQueryError("Only task files expected") }
            let directory = tail[3] == "." ? "" : tail[3]
            if lock.withLock({ $0.failing.contains(directory) }) { throw RegistryQueryError("Permission denied: \(directory)") }
            let cursor = tail.firstIndex(of: "--cursor").map { tail[$0 + 1] }
            let index = cursor.map { Int($0.dropFirst("page".count))! } ?? 0
            let page = pages[directory]![index]
            let value: [String: Any] = ["path": directory,
                "entries": page.entries.map { ["path": $0.0, "kind": $0.1] },
                "next_cursor": page.next as Any? ?? NSNull()]
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }
    }

    private static let checkout: [String: [(entries: [(String, String)], next: String?)]] = [
        "": [([("src", "directory"), ("a.txt", "file")], "page1"), ([("b.txt", "file")], nil)],
        "src": [([("src/lib", "directory"), ("src/main.rs", "file")], nil)],
        "src/lib": [([("src/lib/mod.rs", "file")], nil)],
    ]

    private func paths(_ rows: [CheckoutTree.Row]) -> [String] {
        rows.map { row in
            switch row {
            case .entry(let entry, _, let depth): String(repeating: "  ", count: depth) + entry.path
            case .loading(let directory, _): "loading \(directory)"
            case .more(let directory, _): "more \(directory)"
            case .failed(let directory, _, _): "failed \(directory)"
            }
        }
    }

    @Test func directoryRevealFinishesPagesBeforeCompactingAndResetsIgnoredCursors() async throws {
        let fixture = CheckoutFixture(["": [([("src", "directory")], nil)],
            "src": [([("src/lib", "directory")], "page1"), ([], nil)],
            "src/lib": [([("src/lib/Parser.swift", "file")], nil)]])
        let tree = CheckoutTree(issue: "TEST-1", query: RegistryQuery { args, cwd in try fixture.read(args, cwd: cwd) })
        await tree.openRoot()
        await tree.setDirectory("src", expanded: true)
        #expect(paths(tree.rows()) == ["src", "  src/lib", "more src"])
        await tree.readDirectory("src", more: true)
        await tree.reveal("src/lib/Parser.swift")
        #expect(tree.selection == "src/lib/Parser.swift")
        #expect(paths(tree.rows()) == ["src/lib", "  src/lib/Parser.swift"])
        guard case .entry(_, let label, _) = tree.rows().first else { Issue.record("Missing compacted row"); return }
        #expect(label == "src/lib")
        tree.showIgnored = true
        await tree.openRoot()
        #expect(paths(tree.rows()) == ["src"])
        let read = try #require(fixture.reads.last)
        #expect(read.contains("--show-ignored") && !read.contains("--cursor"))
    }

    @Test func treeReadsOpenedDirectoriesOnlyAndPagesWithLoadMore() async throws {
        let checkout = CheckoutFixture(Self.checkout)
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/task", query: RegistryQuery { args, cwd in try checkout.read(args, cwd: cwd) })
        await store.tree.openRoot()
        #expect(paths(store.tree.rows()) == ["src", "a.txt", "more "])
        #expect(checkout.reads == [["task", "files", "TEST-1", ".", "--json"]])
        await store.tree.readDirectory("", more: true)
        #expect(paths(store.tree.rows()) == ["src", "a.txt", "b.txt"])
        #expect(checkout.reads.last == ["task", "files", "TEST-1", ".", "--json", "--cursor", "page1"])
        // Nothing left to load: Load more reads nothing.
        await store.tree.readDirectory("", more: true)
        #expect(checkout.reads.count == 2)
        // Opening a directory reads it once; closing and reopening keeps its entries.
        await store.tree.setDirectory("src", expanded: true)
        #expect(paths(store.tree.rows()) == ["src", "  src/lib", "  src/main.rs", "a.txt", "b.txt"])
        await store.tree.setDirectory("src", expanded: false)
        await store.tree.setDirectory("src", expanded: true)
        #expect(checkout.reads.count == 3)
        await store.tree.openRoot()
        #expect(checkout.reads.count == 3)
    }

    @Test func treeKeysMoveOpenAndCloseDirectories() async throws {
        let checkout = CheckoutFixture(Self.checkout)
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/task", query: RegistryQuery { args, cwd in try checkout.read(args, cwd: cwd) })
        await store.tree.openRoot()
        await store.moveTree(.down)
        #expect(store.tree.selection == "src")
        await store.moveTree(.expand)
        #expect(store.tree.isExpanded("src"))
        await store.moveTree(.expand)
        #expect(store.tree.selection == "src/lib")
        await store.moveTree(.down)
        #expect(store.tree.selection == "src/main.rs")
        await store.moveTree(.open)
        #expect(store.selection == "src/main.rs")
        #expect(store.page == .files)
        await store.moveTree(.collapse)
        #expect(store.tree.selection == "src")
        await store.moveTree(.collapse)
        #expect(!store.tree.isExpanded("src"))
        await store.moveTree(.down)
        #expect(store.tree.selection == "a.txt")
        await store.moveTree(.up)
        await store.moveTree(.up)
        #expect(store.tree.selection == "src")
        await store.moveTree(.open)
        #expect(store.tree.isExpanded("src"))
    }

    @Test func aFailedDirectoryReadStaysOnThatDirectory() async throws {
        let checkout = CheckoutFixture(Self.checkout)
        checkout.fail("src", true)
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/task", query: RegistryQuery { args, cwd in try checkout.read(args, cwd: cwd) })
        await store.tree.openRoot()
        await store.tree.setDirectory("src", expanded: true)
        #expect(paths(store.tree.rows()) == ["src", "failed src", "a.txt", "more "])
        guard case .failed(_, let reason, let depth) = store.tree.rows()[1] else { Issue.record("expected a failure row"); return }
        #expect(reason.contains("Permission denied"))
        #expect(depth == 1)
        #expect(store.error == nil)
        checkout.fail("src", false)
        await store.tree.retryDirectory("src")
        #expect(paths(store.tree.rows()) == ["src", "  src/lib", "  src/main.rs", "a.txt", "more "])
        // A failed next page keeps the pages already read.
        checkout.fail("", true)
        await store.tree.readDirectory("", more: true)
        #expect(paths(store.tree.rows()).suffix(2) == ["a.txt", "failed "])
        checkout.fail("", false)
        await store.tree.retryDirectory("")
        #expect(paths(store.tree.rows()).suffix(2) == ["a.txt", "b.txt"])
    }

    @Test func aRemoteTreeReadsThroughItsMachineNeverALocalPath() async throws {
        let checkout = CheckoutFixture(Self.checkout)
        let query = RegistryQuery { args, cwd in try checkout.read(args, cwd: cwd) }.onMachine("peer", repository: "plan")
        let store = TaskFilesStore(issue: "remote-task", cwd: "/peer/checkout", query: query)
        await store.tree.openRoot()
        await store.tree.setDirectory("src", expanded: true)
        #expect(paths(store.tree.rows()).prefix(3) == ["src", "  src/lib", "  src/main.rs"])
        let reads = checkout.lock.withLock { $0 }
        #expect(reads.reads.count == 2)
        #expect(reads.reads.allSatisfy { Array($0.prefix(4)) == ["--machine", "peer", "--repo-id", "plan"] })
        #expect(reads.cwds.allSatisfy { $0 == nil })
        #expect(!reads.reads.joined().contains("/peer/checkout"))
    }

    @Test func treeSurvivesTaskSwitchesWithItsWorkspace() async throws {
        let checkout = CheckoutFixture(Self.checkout)
        let query = RegistryQuery { args, cwd in try checkout.read(args, cwd: cwd) }
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let location = WorkspaceIdentity(machineId: "local", worktree: "/task/checkout")
        let content = PaneContent.browser(taskId: "task-files", tabs: [.files], selected: "files", location: location)
        let store = try fixtureWork(registry, location).files(for: content, task: nil, query: query)
        await store.tree.openRoot()
        await store.tree.setDirectory("src", expanded: true)
        store.tree.offset = CGPoint(x: 0, y: 240)
        store.page = .changes
        // Another Task's workspace opens its own Files, then you come back.
        let elsewhere = WorkspaceIdentity(machineId: "local", worktree: "/other/checkout")
        _ = try fixtureWork(registry, elsewhere).files(for: .browser(taskId: "other", tabs: [.files], selected: "files",
            location: elsewhere), task: nil, query: query)
        let returned = try fixtureWork(registry, location).files(for: content, task: nil, query: query)
        #expect(returned === store)
        #expect(returned.tree.isExpanded("src"))
        #expect(returned.tree.selection == "src")
        #expect(returned.tree.offset.y == 240)
        #expect(returned.page == .changes)
        #expect(checkout.reads.count == 2)
    }

    @Test func changeBadgesMarkFilesAndTheFoldersHoldingThem() {
        func file(_ path: String, old: String? = nil, untracked: Bool = false) throws -> TaskChangedFile {
            let value: [String: Any] = ["path": path, "old_path": old as Any? ?? NSNull(), "committed": !untracked,
                                        "staged": false, "unstaged": false, "untracked": untracked, "changes": 1, "lines": NSNull()]
            return try JSONDecoder().decode(TaskChangedFile.self, from: JSONSerialization.data(withJSONObject: value))
        }
        let badges = CheckoutChange.badges([try! file("src/lib/mod.rs"), try! file("src/new.rs", untracked: true),
                                            try! file("README.md", old: "README")])
        #expect(badges["src/lib/mod.rs"] == .modified)
        #expect(badges["src/new.rs"] == .untracked)
        #expect(badges["README.md"] == .renamed)
        #expect(badges["src/lib"] == .contains)
        #expect(badges["src"] == .contains)
        #expect(badges["docs"] == nil)
    }

    private func eventually(_ predicate: () -> Bool) async throws {
        for _ in 0..<200 {
            if predicate() { return }
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(predicate())
    }

    @Test nonisolated func observationReleasesOutsideMainActorWithPendingEvents() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("note.txt")
        try Data("before".utf8).write(to: file)

        var observation: TaskFileObservation? = try await TaskFileObservation(path: directory.path) { _ in }
        await observation?.watch(["note.txt"])
        weak var retained = observation
        try Data("pending".utf8).write(to: file, options: .atomic)
        observation = nil

        // Let queued callbacks and source cancellation finish after teardown.
        try Data("after".utf8).write(to: file, options: .atomic)
        try await Task.sleep(for: .milliseconds(200))
        #expect(retained == nil)
    }

    @Test func taskBrowserSwitchesViewsWithoutLosingItsFileSelection() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from:
            Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
        let task = try #require(roadmap.waves.first?.tasks.items.first)
        let store = TaskFilesStore(issue: task.task.identifier, cwd: "/fixture", query: RegistryQuery { _, _ in "{}" })
        store.selection = "draft.txt"
        store.page = .brief
        let view = TaskFilesView(store: store, checkoutBase: nil, task: task)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "browser-task-overview")
        store.page = .changes
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "browser-changes")
        store.page = .files
        try view.inspect().find(viewWithAccessibilityIdentifier: "files-toggle-navigator").button().tap()
        #expect(store.browser.showsRail == true)
        #expect(store.selection == "draft.txt")
        store.page = .brief
        // Brief and Diff keep the Files page's navigator and selection.
        store.page = .files
        #expect(store.browser.showsRail == true)
        try view.inspect().find(viewWithAccessibilityIdentifier: "files-toggle-navigator").button().tap()
        #expect(store.browser.showsRail == false)
        #expect(store.selection == "draft.txt")
    }

    @Test func taskWorkspaceUsesRecordedCheckoutAndLeavesGenericWorkspaceIntact() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "plan" where args.dropFirst().first == "list": return roadmap
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return "[]"
            case "session" where args.dropFirst().first == "history": return #"{"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected file/launch request")
            }
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: "fixture-repository")
        let rootKey = try #require(registry.selectedWork)
        let rootWorkspace = try #require(registry.retained(rootKey))
        let rootLayout = rootWorkspace.multiplexer.layout
        for (task, location) in [("issue-now", "loopflow.make-lf-work-the-machine"),
                                 ("issue-review", "loopflow.all-wave-roadmap"),
                                 ("issue-now", "loopflow.make-lf-work-the-machine")] {
            model.select(.task(id: task))
            model.navigation.content = .terminals
            let key = try #require(registry.selectedWork)
            let workspace = try #require(registry.retained(key))
            #expect(workspace.location?.worktree.hasSuffix(location) == true)
            #expect(registry.retained(rootKey) === rootWorkspace)
            #expect(rootWorkspace.multiplexer.layout == rootLayout)
        }
    }

    @Test func wirePreservesExactBaseAndLosslessText() throws {
        let fixture = try fixture()
        #expect(fixture.symbols.hits.map(\.container) == ["A", "B"])
        #expect(fixture.grep.hits.last?.start == 12)
        #expect(fixture.find.latestScratchPage == fixture.find.scratch.first)
        #expect(fixture.directory.entries.map(\.kind) == [.directory, .file, .symlink])
        #expect(fixture.changes.baseCommit == fixture.diff.baseCommit)
        #expect(fixture.changes.files[0].oldPath == "old.txt")
        #expect(fixture.file.content == "\u{feff}notes\r\n")
        #expect(fixture.file.revision?.count == 64)
        #expect(fixture.save.published)
        #expect(fixture.file.recoveries.first?.changed == true)
    }

    @Test func taskFileSnapshotCarriesABinaryFilesBytes() throws {
        let fixture = try fixture()
        #expect(fixture.file.data == nil)
        #expect(fixture.image.state == .binary)
        #expect(fixture.image.readOnlyReason == "A committed version is read-only.")
        #expect(fixture.image.data == Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]))
    }

    @Test func lateFileReadCannotReplaceAnotherSelection() async throws {
        let source = HeldTaskFileRead()
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            try await source.read(args[3])
        })
        store.selection = "first.txt"
        let first = Task { await store.loadFile() }
        for _ in 0..<100 {
            if await source.waiting { break }
            await Task.yield()
        }
        store.selection = "second.txt"
        await store.loadFile()
        await source.release()
        await first.value
        #expect(store.selectedDocument?.text == "second.txt")
        #expect(store.documents["first.txt"] == nil)
    }

    @Test func filesystemEventsRefreshDiffForFileNeverOpened() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("new.txt")
        let header = "diff --git a/new.txt b/new.txt\n--- a/new.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-old\n"
        try Data((header + "+edit 1\n").utf8).write(to: file)
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let store = TaskFilesStore(issue: "TEST-1", cwd: directory.path, query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1] == "files" ? "directory" : args[1] == "changes" ? "changes" : "diff"] as! [String: Any]
            if args[1] == "diff" { value["patch"] = try String(contentsOf: file, encoding: .utf8) }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.selection = "new.txt"
        store.mode = .diff
        store.observeFiles()
        await store.refresh()
        await store.loadDiff()
        let added = { store.diffReview.fileReview?.diff.file("new.txt")?.lines.filter { $0.kind == .added }.map(\.text) }
        #expect(added() == ["edit 1"])
        try Data((header + "+edit 2\n").utf8).write(to: file, options: .atomic)
        try await eventually { added() == ["edit 2"] }
        #expect(added() == ["edit 2"])
        #expect(store.documents.isEmpty)
    }

    /// Files' Diff mode reads the unsaved draft through the same stream as Changes.
    @Test func diffModeComparesTheUnsavedDraftInTheReviewStream() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let reply: @Sendable ([String], String?) throws -> String = { args, draft in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1] == "changes" ? "changes" : "diff"] as! [String: Any]
            if args[1] == "diff" {
                let added = draft.map { "+" + $0.trimmingCharacters(in: .newlines) } ?? "+saved"
                value["patch"] = "diff --git a/new.txt b/new.txt\n--- a/new.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-saved\n\(added)\n"
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery(
            runWithInput: { args, _, draft in try reply(args, draft) },
            run: { args, _ in try reply(args, nil) }))
        store.autosave = false
        store.selection = "new.txt"
        store.mode = .diff
        let document = store.document("new.txt")
        await document.reconcile(snapshot("saved\n"))
        await store.loadDiff()
        #expect(store.diffReview.fileReview?.draft == false)
        document.editor.insertText("typed ", replacementRange: NSRange(location: 0, length: 0))
        await store.loadDiff()
        let review = try #require(store.diffReview.fileReview)
        #expect(review.draft)
        let file = try #require(review.diff.file("new.txt"))
        let change = try #require(file.blocks.compactMap { block -> ReviewChange? in
            if case .change(let change) = block { return change }
            return nil
        }.first)
        // An edited line pairs its before and after and emphasizes the typed words.
        #expect(change.pairs.first?.added?.text == "typed saved")
        #expect(change.pairs.first?.addedSegments?.filter(\.changed).map(\.text).joined().contains("typed") == true)
        #expect(store.diffReview.comparisonDescription?.hasSuffix(". Includes uncommitted changes.") == true)
        #expect(store.diffReview.comparisonDescription?.hasPrefix("Compared with the Task's base at aaaaaaaaaa") == true)
    }

    /// A store over two changed files, each with two separate changes.
    /// `word` is the first hunk's new line, read on every file read.
    private func twoFileStore(failingOnce: String? = nil, calledOff: Bool = false,
                              word: OSAllocatedUnfairLock<String> = .init(initialState: "uno"),
                              before: @escaping @Sendable (String) async -> Void = { _ in }) throws -> TaskFilesStore {
        let failed = OSAllocatedUnfairLock(initialState: false)
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let reply: @Sendable ([String]) throws -> String = { args in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = ["a.swift", "b.swift"].map { path in
                    file.merging(["path": path, "old_path": NSNull(), "changes": 2]) { $1 }
                }
            case "file":
                // No tour. Any other file is the fixture's one line, shorter than a second hunk expects.
                if args[3] == DiffTour.path { throw RegistryQueryError("no such file") }
            default:
                if args[3] == failingOnce, !failed.withLock({ let was = $0; $0 = true; return was }) {
                    if calledOff { throw CancellationError() }
                    throw RegistryQueryError("the server restarted")
                }
                // No path argument is the whole comparison.
                let paths = args[3].hasPrefix("--") ? ["a.swift", "b.swift"] : [args[3]]
                value["patch"] = paths.map { path in
                    "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n"
                        + "@@ -1 +1 @@\n-one\n+\(word.withLock { $0 })\n@@ -9 +9 @@\n-nine\n+nueve\n"
                }.joined()
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            if args[1] == "diff" { await before(args[3]) }
            return try reply(args)
        })
        store.autosave = false
        return store
    }

    @Test func theChangeCountIsKnownBeforeEveryPatchArrives() async throws {
        let (released, release) = AsyncStream<Void>.makeStream()
        let store = try twoFileStore(before: { path in
            if path == "b.swift" { for await _ in released {} }
        })
        let load = Task { await store.diffReview.loadReview() }
        while store.diffReview.shownReview?.file("a.swift")?.blocks.isEmpty != false {
            try await Task.sleep(for: .milliseconds(5))
        }
        #expect(store.diffReview.position(.changes) == "4 changes")
        #expect(store.diffReview.step(1, in: .changes)?.path == "a.swift")
        #expect(store.diffReview.position(.changes) == "1 of 4")
        release.finish()
        await load.value
        #expect(store.diffReview.position(.changes) == "1 of 4")
    }

    /// Each shown file's lines are read on their own, so one file's size never
    /// cuts another short, and a hidden file is not read until it is shown. A
    /// new file, untracked or not, is read as its patch; a line too long to
    /// read is cut where it stands and its file keeps every stop.
    @Test func theDiffReadsEachShownFileOnItsOwn() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let paths = ["a.swift", "data.json", "scratch/notes.md", "z.swift"]
        let reads = OSAllocatedUnfairLock<[String]>(initialState: [])
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = paths.map {
                    file.merging(["path": $0, "old_path": NSNull(), "untracked": $0 == "data.json",
                                  "changes": $0 == "data.json" ? 1 : 2]) { $1 }
                }
            case "file":
                throw RegistryQueryError("a new file is read as its patch")
            default:
                let path = args[3]
                reads.withLock { $0.append("diff \(path)") }
                let uno = path == "a.swift" ? String(repeating: "x", count: 2_000) : "uno"
                value["patch"] = path == "data.json"
                    ? "diff --git a/\(path) b/\(path)\nnew file mode 100644\n--- /dev/null\n+++ b/\(path)\n@@ -0,0 +1,2 @@\n+{}\n+end\n"
                    : "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n@@ -1 +1 @@\n-one\n+\(uno)\n@@ -9 +9 @@\n-nine\n+nueve\n"
                value["truncated"] = path == "z.swift"
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        await store.diffReview.loadReview()
        #expect(store.diffReview.review?.files.map(\.path) == paths)
        #expect(Set(reads.withLock { $0 }) == ["diff a.swift", "diff data.json", "diff z.swift"])
        #expect(store.diffReview.shownReview?.files.map(\.path) == ["a.swift", "data.json", "z.swift"])
        #expect(store.diffReview.unreadShown == 0 && store.diffReview.unread == ["scratch/notes.md"])
        #expect(store.diffReview.fileNotes == ["z.swift": .cut])

        // The untracked file is its two lines.
        let data = try #require(store.diffReview.shownReview?.file("data.json"))
        #expect(data.status == .added && data.lines.map(\.new) == [1, 2])
        // The long line shows its first 1,000 units; the stepper still reaches every change it counts.
        let long = try #require(store.diffReview.shownReview?.file("a.swift")?.lines.first { $0.kind == .added })
        #expect(long.cut?.shown.utf16.count == 1_000 && long.cut?.more == 1_000)
        #expect(data.lines.allSatisfy { $0.cut == nil })
        #expect(store.diffReview.position(.changes) == "5 changes")
        #expect((1...6).map { _ in store.diffReview.step(1, in: .changes)?.path } == ["a.swift", "a.swift", "data.json", "z.swift", "z.swift", nil])
        #expect(store.diffReview.position(.changes) == "5 of 5")

        store.diffReview.showsHidden = true
        await store.diffReview.loadReview()
        #expect(reads.withLock { $0 }.contains("diff scratch/notes.md"))
        #expect(store.diffReview.unread.isEmpty && store.diffReview.review?.stops.count == 7)
    }

    /// A file the Diff renders is one change, landing on its top: a picture
    /// counts though it has no lines, and a redline's hunks are not drawn.
    /// With Source on, the file's hunks are its changes again.
    @Test func aRenderedFileIsOneChange() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let paths = ["a.swift", "doc.md", "logo.png"]
        let (waiting, patchStarted) = AsyncStream<Void>.makeStream()
        let (held, releasePatch) = AsyncStream<Void>.makeStream()
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = zip(paths, [2, 3, 0]).map { path, count in
                    file.merging(["path": path, "old_path": NSNull(), "untracked": false, "changes": count]) { $1 }
                }
            case "file":
                throw RegistryQueryError("sides are not read here")
            default:
                let path = args[3]
                if path == "logo.png" {
                    patchStarted.finish()
                    for await _ in held {}
                }
                let hunks = path == "doc.md" ? [1, 9, 19] : [1, 9]
                value["patch"] = path == "logo.png"
                    ? "diff --git a/\(path) b/\(path)\nBinary files a/\(path) and b/\(path) differ\n"
                    : "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n"
                        + hunks.map { "@@ -\($0) +\($0) @@\n-old\n+new\n" }.joined()
                value["truncated"] = false
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        let load = Task { await store.diffReview.loadReview() }
        for await _ in waiting {}
        #expect(store.diffReview.position(.changes) == "4 changes")
        releasePatch.finish()
        await load.value
        #expect(store.diffReview.position(.changes) == "4 changes")
        var visited: [String] = []
        while let stop = store.diffReview.step(1, in: .changes) { visited.append(stop.id) }
        #expect(Array(visited.dropFirst(2)) == ["doc.md#rendered", "logo.png#rendered"])
        #expect(visited.prefix(2).allSatisfy { $0.hasPrefix("a.swift#") })
        #expect(store.diffReview.position(.changes) == "4 of 4")

        store.diffReview.sourceFiles.insert("doc.md")
        #expect(store.diffReview.position(.changes) == "6 of 6")
        #expect(store.diffReview.stops(.changes).map(\.path) == ["a.swift", "a.swift", "doc.md", "doc.md", "doc.md", "logo.png"])
    }

    /// A file whose read failed says so and keeps its place; Retry reads it again.
    @Test func aFailedFileReadCanBeRetried() async throws {
        let store = try twoFileStore(failingOnce: "b.swift")
        await store.diffReview.loadReview()
        guard case .failed = store.diffReview.fileNotes["b.swift"] else { Issue.record("b.swift should have failed"); return }
        #expect(store.diffReview.review?.file("a.swift")?.stops.count == 2 && store.diffReview.review?.file("b.swift")?.blocks.isEmpty == true)
        #expect(store.diffReview.unreadShown == 0)
        // Its changes still count: the totals come with the file list, not its lines.
        #expect(store.diffReview.position(.changes) == "4 changes")
        store.diffReview.step(1, in: .changes)
        store.diffReview.step(1, in: .changes)
        #expect(store.diffReview.position(.changes) == "2 of 4")

        await store.diffReview.retryFile("b.swift")
        #expect(store.diffReview.fileNotes.isEmpty && store.diffReview.unread.isEmpty)
        #expect(store.diffReview.review?.file("b.swift")?.stops.count == 2)
    }

    /// A read that was called off is not a failure: the file stays unread and is read again.
    @Test func aFileReadCalledOffIsNotAFailure() async throws {
        let store = try twoFileStore(failingOnce: "b.swift", calledOff: true)
        await store.diffReview.loadReview()
        #expect(store.diffReview.fileNotes.isEmpty && store.diffReview.unread.isEmpty)
        #expect(store.diffReview.review?.file("b.swift")?.stops.count == 2)
    }

    /// The Diff draws a window of files, so a jump lands at once however far
    /// down it goes. A jump far down starts the window at its file, scrolling
    /// down draws the next files, and showing earlier files keeps the top line.
    @Test func aFarJumpStartsTheDiffsWindowAtItsFile() async throws {
        let paths = (0..<6).map { "f\($0).swift" }
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1] == "changes" ? "changes" : "diff"] as! [String: Any]
            if args[1] == "changes" {
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = paths.map { file.merging(["path": $0, "old_path": NSNull(), "changes": 1]) { $1 } }
            } else {
                // 600 added lines: about 600 rows each.
                let path = args[3]
                value["patch"] = "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n@@ -0,0 +1,600 @@\n"
                    + (1...600).map { "+line \($0)\n" }.joined()
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        await store.diffReview.loadReview()
        let files = try #require(store.diffReview.shownReview?.files)
        #expect(store.diffReview.window == 0..<3)

        // Stepping into the next file stays in the window.
        store.diffReview.step(1, in: .changes)
        let next = try #require(store.diffReview.step(1, in: .changes))
        store.diffReview[place: .changes].anchor = next.id
        #expect(next.path == "f1.swift" && store.diffReview.window == 0..<3)

        store.diffReview[place: .changes] = .init(anchor: files[5].topID, current: files[5].stops.first)
        #expect(store.diffReview.window == 5..<6 && store.diffReview.position(.changes) == "6 of 6")

        // The scroll view's reports never move it.
        store.diffReview.scrolled(to: "\(files[5].path)#c0-300", in: .changes)
        #expect(store.diffReview.window == 5..<6)

        store.diffReview.showEarlierFiles()
        #expect(store.diffReview.window == 2..<6 && store.diffReview[place: .changes].anchor == "\(files[5].path)#c0-300")

        store.diffReview[place: .changes] = .init(anchor: files[0].topID, current: files[0].stops.first)
        #expect(store.diffReview.window == 0..<3)
        store.diffReview.extendWindow()
        #expect(store.diffReview.window == 0..<5)
    }

    /// A file marked viewed folds to its header and stepping passes over it,
    /// until its lines change.
    @Test func aViewedFileFoldsUntilItsLinesChange() async throws {
        let word = OSAllocatedUnfairLock(initialState: "uno")
        let store = try twoFileStore(word: word)
        await store.diffReview.loadReview()
        let a = try #require(store.diffReview.shownReview?.file("a.swift"))
        store.diffReview.setViewed(a, true)
        #expect(store.diffReview.viewedCount == 1 && store.diffReview.position(.changes) == "2 changes")
        #expect((1...3).map { _ in store.diffReview.step(1, in: .changes)?.path } == ["b.swift", "b.swift", nil])
        #expect(store.diffReview.shownReview?.files.count == 2)

        word.withLock { $0 = "eins" }
        await store.diffReview.loadReview(changed: ["a.swift"])
        #expect(store.diffReview.viewedCount == 0 && store.diffReview.position(.changes).hasSuffix("of 4"))
    }

    /// The Task's commits read as Graph steps; picking one compares its first
    /// commit's parent with its last, and the whole Task comes back after.
    @Test func aGraphStepComparesItsOwnCommits() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let sha: @Sendable (Int) -> String = { String(repeating: "\($0)", count: 40) }
        let base = String(repeating: "a", count: 40)
        let subjects = OSAllocatedUnfairLock(initialState: ["lf implement: parse", "lf implement: render", "fix a typo", "lf compress: fold"])
        let reads = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1] == "changes" ? "changes" : "diff"] as! [String: Any]
            if args[1] == "changes" {
                reads.withLock { $0.append(args) }
                let whole = !args.contains("--head")
                if !whole { value["files"] = [] }
                value["base_commit"] = base
                value["commits"] = subjects.withLock { $0 }.enumerated().filter { whole || $0.offset < 2 }
                    .map { ["sha": sha($0.offset + 1), "parent": $0.offset == 0 ? base : sha($0.offset), "subject": $0.element] }
            } else {
                value["patch"] = ""
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        let firstPicker = TaskFilePicker(store: store, mode: .changed)
        await firstPicker.load()
        #expect(!firstPicker.results.isEmpty)
        await store.diffReview.loadReview()
        #expect(store.diffReview.steps.map(\.title) == ["implement · 2 commits", "fix a typo", "compress"])
        #expect(store.diffReview.steps.map(\.base) == [base, sha(2), sha(3)])

        let whole = try #require(store.workingChanges)
        store.diffReview.showStep(store.diffReview.steps[0])
        await store.diffReview.loadReview()
        #expect(store.diffReview.comparison == .branch(base: base, head: sha(2)))
        #expect(reads.withLock { $0.last }?.contains(sha(2)) == true)
        #expect(store.diffReview.changes?.files.isEmpty == true && store.workingChanges?.files == whole.files)
        let picker = TaskFilePicker(store: store, mode: .changed)
        await picker.load()
        #expect(!picker.results.isEmpty && picker.results.map(\.path) == whole.files.map(\.path))
        #expect(store.diffReview.steps.count == 3 && store.diffReview.comparisonDescription?.hasPrefix("Step implement · 2 commits") == true)

        subjects.withLock { $0.append("lf qa: exercise the reader") }
        await store.workingTree.refresh()
        #expect(store.diffReview.steps.last?.title == "qa")
        #expect(store.diffReview.comparison == .branch(base: base, head: sha(2)))

        store.diffReview.showStep(nil)
        await store.diffReview.loadReview()
        #expect(store.diffReview.comparison == .workingTree(base: "parent") && store.diffReview.step == nil)
    }

    /// A step's skipped lines open from its head commit, not the working tree.
    @Test func aStepOpensSkippedLinesFromItsHeadCommit() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let base = String(repeating: "a", count: 40), head = String(repeating: "1", count: 40)
        let committed = (1...9).map { $0 == 5 ? "let five = 5" : "let line\($0) = \($0)" }.joined(separator: "\n") + "\n"
        let reads = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = [file.merging(["path": "a.swift", "old_path": NSNull()]) { $1 }]
                value["base_commit"] = base
                value["head_commit"] = args.contains("--head") ? head : String(repeating: "b", count: 40)
                value["commits"] = [["sha": head, "parent": base, "subject": "lf implement: count"]]
            case "file":
                reads.withLock { $0.append(args) }
                value["path"] = args[3]
                // The working tree has moved on since the step.
                value["content"] = args.contains(head) ? committed : "worktree\n"
            default:
                value["patch"] = "diff --git a/a.swift b/a.swift\n--- a/a.swift\n+++ b/a.swift\n"
                    + "@@ -1 +1 @@\n-let line1 = 0\n+let line1 = 1\n@@ -9 +9 @@\n-let line9 = 0\n+let line9 = 9\n"
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        await store.diffReview.loadReview()
        let step = try #require(store.diffReview.steps.first)
        store.diffReview.showStep(step)
        await store.diffReview.loadReview()
        let file = try #require(store.diffReview.shownReview?.file("a.swift"))
        let (index, header) = try #require(file.blocks.enumerated().compactMap { index, block -> (Int, String)? in
            if case .hunk(let header) = block, index > 0 { return (index, header) }
            return nil
        }.first)
        await store.diffReview.openGap(header, gap: try #require(file.gaps[index]), file: file, reader: .changes)
        #expect(store.diffReview.reviewReading.openedGaps[file.path]?.failure == nil)
        let opened = store.diffReview.opened(file)
        #expect(opened.lines.first { $0.new == 5 }?.text == "let five = 5")
        #expect(reads.withLock { $0 }.contains { $0.suffix(2) == ["--at", head] })
        #expect(reads.withLock { $0 }.contains { $0.suffix(2) == ["--at", base] })
    }

    /// The Diff fits the pane it is given. No row asks for more than a narrow
    /// pane offers, so nothing widens the column or is clipped, and the page
    /// takes exactly the pane's width.
    @Test(arguments: [360, 480, 640] as [CGFloat]) func theDiffFitsItsPane(width: CGFloat) async throws {
        let store = try twoFileStore()
        await store.diffReview.loadReview()
        let first = try #require(store.diffReview.shownReview?.files.first)
        store.comments.selection = ReviewSelection(path: first.path, source: .diff(side: .new, revision: store.diffReview.revision), first: 1, last: 1)
        store.comments.draft = "why"
        store.comments.add(anchor: ["first"])
        store.comments.selection = ReviewSelection(path: first.path, source: .diff(side: .new, revision: store.diffReview.revision), first: 9, last: 9)
        let path = "swift/LoopflowMac/Views/ADirectoryWithALongName/AnotherOne/TaskChangesViewWithALongName.swift"
        let long = ReviewFile(path: path, oldPath: path + ".old", status: .renamed, blocks: [
            .change(ReviewChange(id: "\(path)#n1", removed: [], added: [
                ReviewLine(kind: .added, text: String(repeating: "x", count: 2_000), old: nil, new: 1),
            ])),
        ])

        func taken<V: View>(_ view: V) -> CGFloat {
            let host = NSHostingController(rootView: view)
            host.view.frame = CGRect(x: 0, y: 0, width: width, height: 600)
            host.view.layoutSubtreeIfNeeded()
            return host.sizeThatFits(in: CGSize(width: width, height: 600)).width
        }
        let page = TaskChangesView(store: store, task: nil, isFocused: true, addToConversation: nil)
        #expect(taken(page) == width)
        #expect(taken(page.header) <= width)
        #expect(taken(page.toolbar) <= width)
        #expect(taken(ReviewFileTitle(store: store, file: long, note: "from \(path).old")) <= width)
        #expect(taken(VStack(spacing: 0) { ReviewCommentsStrip(store: store, isFocused: true, addToConversation: nil) }) <= width)
        #expect(taken(VStack(spacing: 0) {
            ReviewStream(store: store, reader: .changes, files: (store.diffReview.shownReview?.files ?? []) + [long],
                         split: false, addToConversation: nil)
        }) <= width)
    }

    /// A narrow pane moves a file's counts and Viewed under its name rather
    /// than clipping either; a wide one keeps the header on one line.
    @Test func aFileHeaderWrapsInsteadOfClipping() async throws {
        let store = try twoFileStore()
        await store.diffReview.loadReview()
        let path = "swift/LoopflowMac/Design/TaskChangesViewWithALongName.swift"
        let file = ReviewFile(path: path, oldPath: nil, status: .added, blocks: [])
        func height(at width: CGFloat) -> CGFloat {
            let host = NSHostingController(rootView: ReviewFileTitle(store: store, file: file, note: nil, viewable: true))
            return host.sizeThatFits(in: CGSize(width: width, height: 600)).height
        }
        let wide = height(at: 1_200), narrow = height(at: 360)
        #expect(narrow > wide * 1.5)
    }

    /// The checkout's `scratch/tour.yaml` orders the Diff and hides what
    /// the tour named, until you ask for it; your own hidden paths are untouched.
    @Test func aTourFileOrdersTheDiffAndHidesItsPathsUntilShown() async throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let paths = ["a.swift", "docs/guide.md", "z.swift"]
        let content = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnueve\n"
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                value["files"] = paths.map { file.merging(["path": $0, "old_path": NSNull()]) { $1 } }
            case "file":
                value["path"] = args[3]
                value["content"] = args[3] == DiffTour.path
                    ? "first:\n  - path: z.swift\n    why: Start here\nhide:\n  - path: docs/\n    why: Rename only\n" : content
            default:
                value["patch"] = paths.map { path in
                    "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n@@ -1 +1 @@\n-uno\n+one\n@@ -9 +9 @@\n-nine\n+nueve\n"
                }.joined()
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        await store.diffReview.loadReview()
        #expect(store.diffReview.ordering == .tour)
        #expect(store.diffReview.shownReview?.files.map(\.path) == ["z.swift", "a.swift"])
        #expect(store.diffReview.shownGroups.first?.title == "Start here")
        #expect(store.diffReview.step(1, in: .changes)?.path == "z.swift")
        #expect(store.diffReview.tourHides("docs/guide.md")?.why == "Rename only")
        #expect(store.diffReview.hidden == [.init(pattern: "scratch/", source: "Default")])
        store.diffReview.shownTourHides = ["docs/"]
        #expect(store.diffReview.shownReview?.files.map(\.path) == ["z.swift", "a.swift", "docs/guide.md"])
        // By path reads folder by folder, top-level files first.
        store.diffReview.orderingChoice = .path
        #expect(store.diffReview.shownReview?.files.map(\.path) == ["a.swift", "z.swift", "docs/guide.md"])

        // Skipped lines open from the file as it stands, and a comment on one stays through a reload.
        let file = try #require(store.diffReview.shownReview?.file("a.swift"))
        let (index, header) = try #require(file.blocks.enumerated().compactMap { index, block -> (Int, String)? in
            if case .hunk(let header) = block, index > 0 { return (index, header) }
            return nil
        }.first)
        await store.diffReview.openGap(header, gap: try #require(file.gaps[index]), file: file, reader: .changes)
        #expect(store.diffReview.reviewReading.openedGaps[file.path]?.failure == nil)
        #expect(store.diffReview.opened(file).lines.compactMap(\.new).contains(5))
        store.comments.selection = ReviewSelection(path: "a.swift", source: .diff(side: .new, revision: store.diffReview.revision), first: 5, last: 5)
        store.comments.draft = "what is five?"
        let selection = try #require(store.comments.selection)
        store.comments.add(anchor: selection.excerpt(in: store.diffReview.opened(file)))
        #expect(store.comments.selection == nil && store.comments.draft.isEmpty)
        #expect(store.comments.items.map(\.anchor) == [["five"]])
        await store.diffReview.loadReview()
        #expect(store.comments.items.map(\.detached) == [false])
    }

    /// Skipped lines that do not load say so under their own file, and the
    /// words go when that file's opened rows go.
    @Test func aFailedSkippedLinesOpenBelongsToItsFile() async throws {
        let store = try twoFileStore()
        await store.diffReview.loadReview()
        let file = try #require(store.diffReview.review?.file("a.swift"))
        let (index, header) = try #require(file.blocks.enumerated().compactMap { index, block -> (Int, String)? in
            if case .hunk(let header) = block, index > 0 { return (index, header) }
            return nil
        }.first)
        await store.diffReview.openGap(header, gap: try #require(file.gaps[index]), file: file, reader: .changes)
        #expect(store.diffReview.reviewReading.openedGaps["a.swift"]?.failure?.contains("the file changed since this comparison was read") == true)
        #expect(store.diffReview.reviewReading.openedGaps["b.swift"] == nil)
        #expect(store.diffReview.opened(file) == file)

        store.diffReview.showStep(ReviewStep(name: "implement", base: "a", commits: [], head: "b"))
        #expect(store.diffReview.reviewReading.openedGaps["a.swift"] == nil)
    }

    /// A store over `a.swift` and `z.swift` whose checkout holds `tour`.
    /// `sinceTour` is what changed after the commit of forty `c`s; `launches`
    /// collects each `skill.run` with its checkout.
    private func tourStore(tour: OSAllocatedUnfairLock<String>, sinceTour: OSAllocatedUnfairLock<[String]>,
                           launches: OSAllocatedUnfairLock<[[String]]>) throws -> TaskFilesStore {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let old = Self.tourCommit
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, cwd in
            if args.first == "--batch" {
                launches.withLock { $0.append(args + [cwd ?? ""]) }
                return #"{"operation":"skill.run","lf_process_id":"p1","run":"skill tour","log":"/logs/tour.log"}"#
            }
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1]] as! [String: Any]
            switch args[1] {
            case "changes":
                let file = (value["files"] as! [[String: Any]])[0]
                let paths = args[4] == old ? sinceTour.withLock({ $0 }) : ["a.swift", "z.swift"]
                value["files"] = paths.map { file.merging(["path": $0, "old_path": NSNull()]) { $1 } }
            case "file":
                value["path"] = args[3]
                value["content"] = args[3] == DiffTour.path ? tour.withLock({ $0 }) : "one\n"
            default:
                value["patch"] = ["a.swift", "z.swift"].map { path in
                    "diff --git a/\(path) b/\(path)\n--- a/\(path)\n+++ b/\(path)\n@@ -1 +1 @@\n-uno\n+one\n"
                }.joined()
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        return store
    }
    private static let tourCommit = String(repeating: "c", count: 40)
    private static let tourAtOlderCommit = "at: \(tourCommit)\nfirst:\n  - path: a.swift\n    why: Start here\n"

    /// A tour names the commit it was written at. Once a later commit changes a
    /// file outside `scratch/`, the Diff says how far behind it is and offers
    /// Update tour, which runs the `tour` skill headless in the Task's checkout;
    /// the order switches to Tour when the new file lands.
    @Test func aTourWrittenAtAnOlderCommitReadsAsStaleAndUpdateTourRewritesIt() async throws {
        let tour = OSAllocatedUnfairLock(initialState: Self.tourAtOlderCommit)
        let sinceTour = OSAllocatedUnfairLock(initialState: ["a.swift", "scratch/tour.yaml"])
        let launches = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let store = try tourStore(tour: tour, sinceTour: sinceTour, launches: launches)
        await store.diffReview.loadReview()
        #expect(store.diffReview.tourStatus == "The tour was written at ccccccc · 1 file changed since")
        #expect(store.diffReview.tourAction == "Update tour")

        // Committing only the tour itself leaves it current.
        sinceTour.withLock { $0 = ["scratch/tour.yaml"] }
        await store.diffReview.loadReview()
        #expect(store.diffReview.tourStatus == nil && store.diffReview.tourAction == nil)

        sinceTour.withLock { $0 = ["a.swift"] }
        await store.diffReview.loadReview()
        store.diffReview.orderingChoice = .path
        await store.diffReview.writeTour()
        #expect(launches.withLock { $0 } == [["--batch", "skill", "tour", "/fixture"]])
        #expect(store.diffReview.tourStatus == "Writing the tour… · its output: /logs/tour.log")

        tour.withLock { $0 = "at: \(String(repeating: "b", count: 40))\nfirst:\n  - path: z.swift\n    why: Now here\n" }
        await store.diffReview.loadReview()
        #expect(store.diffReview.ordering == .tour)
        #expect(store.diffReview.shownGroups.first?.title == "Now here")
        #expect(store.diffReview.tourStatus == nil && store.diffReview.tourAction == nil)
    }

    /// Nothing reports a tour run ending. Once its launch has answered, the
    /// button is back beside the log path, so a run that died can be started
    /// again; the wait ends when the file changes, whatever commit it names.
    @Test func aTourRunThatWritesNothingLeavesTheButtonBesideItsLog() async throws {
        let tour = OSAllocatedUnfairLock(initialState: Self.tourAtOlderCommit)
        let launches = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let store = try tourStore(tour: tour, sinceTour: OSAllocatedUnfairLock(initialState: ["a.swift"]), launches: launches)
        await store.diffReview.loadReview()
        await store.diffReview.writeTour()

        // The run died: the file never changes, and the next read keeps both.
        await store.diffReview.loadReview()
        #expect(store.diffReview.tourStatus == "Writing the tour… · its output: /logs/tour.log")
        #expect(store.diffReview.tourAction == "Update tour")
        await store.diffReview.writeTour()
        #expect(launches.withLock { $0.count } == 2)

        // A rewrite at the same commit is still the run's file landing.
        let rewritten = Self.tourAtOlderCommit.replacingOccurrences(of: "Start here", with: "Read this first")
        tour.withLock { $0 = rewritten }
        await store.diffReview.loadReview()
        #expect(store.diffReview.shownGroups.first?.title == "Read this first")
        #expect(store.diffReview.tourStatus == "The tour was written at ccccccc · 1 file changed since")
        #expect(store.diffReview.tourAction == "Update tour")
    }

    @Test func aDiffWithoutATourOffersToWriteOne() async throws {
        let store = try twoFileStore()
        await store.diffReview.loadReview()
        #expect(store.diffReview.tourAction == "Write tour")
    }

    /// `diff_hidden` as `lf` resolved it: every layer's patterns, each named by
    /// the layer that added it; only yours can be removed, and removing yours
    /// leaves the others.
    @Test func hiddenPatternsKeepWhereTheyCameFromAndOnlyYoursChange() throws {
        let explained = try JSONDecoder().decode(ConfigurationValue.self, from: Data("""
        {"value": ["scratch/", "vendor/", "tests/"], "candidates": [
          {"key": "diff_hidden", "source": "builtin", "reason": "", "locator": "builtin", "scope": "global",
           "target": "general", "value": ["scratch/"]},
          {"key": "diff_hidden", "source": "repository", "reason": "", "locator": ".lf/config.yaml",
           "scope": "repository", "target": "general", "value": ["vendor/"]},
          {"key": "diff_hidden", "source": "personal", "reason": "", "locator": ".lf/config.local.yaml",
           "scope": "repository", "target": "general", "value": ["tests/"]}]}
        """.utf8))
        let store = try twoFileStore()
        store.diffReview.hidden = try #require(DiffReviewState.hidden(from: explained))
        #expect(store.diffReview.hidden.map(\.source) == ["Default", "Repository", nil])
        #expect(store.diffReview.yourHidden == ["tests/"])
        store.diffReview.hide(" *.lock ")
        store.diffReview.unhide("vendor/")
        store.diffReview.unhide("tests/")
        #expect(store.diffReview.hidden.map(\.pattern) == ["scratch/", "vendor/", "*.lock"])
        #expect(store.diffReview.hides("Cargo.lock") && store.diffReview.hides("rust/vendor/x.rs") && !store.diffReview.hides("rust/lockfile.rs"))
    }

    @Test func fileDiffStopsAtItsLastChange() async throws {
        let store = try twoFileStore()
        store.selectFile("a.swift", mode: .diff)
        await store.loadDiff()
        let stops = try #require(store.diffReview.diff(.file("a.swift"))).stops
        #expect(store.diffReview.step(1, in: .file("a.swift"))?.id == stops[0].id)
        #expect(store.diffReview.step(1, in: .file("a.swift"))?.id == stops[1].id)
        #expect(store.diffReview.position(.file("a.swift")) == "2 of 2")
        #expect(store.diffReview.step(1, in: .file("a.swift")) == nil)
        #expect(store.selection == "a.swift")
        #expect(store.diffReview.position(.file("a.swift")) == "2 of 2")
    }

    @Test func eachReaderKeepsItsPlaceAcrossPathSwitches() async throws {
        let store = try twoFileStore()
        store.selectFile("a.swift", mode: .diff)
        await store.loadDiff()
        store.diffReview.step(1, in: .file("a.swift"))
        store.diffReview[place: .file("a.swift")].anchor = "a.swift#@@ -9 +9 @@"
        let a = store.diffReview[place: .file("a.swift")]
        store.page = .changes
        await store.diffReview.loadReview()
        store.diffReview.step(1, in: .changes)
        store.diffReview.step(1, in: .changes)
        store.diffReview.step(1, in: .changes)
        let changes = store.diffReview[place: .changes]
        store.selectFile("b.swift", mode: .diff)
        await store.loadDiff()
        store.diffReview.step(1, in: .file("b.swift"))
        store.selectFile("a.swift", mode: .diff)
        await store.loadDiff()
        #expect(store.diffReview[place: .file("a.swift")] == a)
        #expect(store.diffReview[place: .changes] == changes)
        #expect(store.diffReview[place: .changes].current?.hasPrefix("b.swift") == true)
        #expect(store.diffReview[place: .file("b.swift")].current != nil)
    }

    /// A commit in a linked worktree changes no file in the checkout.
    @Test func commitInLinkedWorktreeRereadsTheComparison() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let checkout = directory.appendingPathComponent("checkout")
        let metadata = directory.appendingPathComponent("main/.git/worktrees/checkout")
        try FileManager.default.createDirectory(at: checkout, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: metadata, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data("gitdir: \(metadata.path)\n".utf8).write(to: checkout.appendingPathComponent(".git"))
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let readings = OSAllocatedUnfairLock(initialState: 0)
        let store = TaskFilesStore(issue: "TEST-1", cwd: checkout.path, query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            if args[1] == "changes" { readings.withLock { $0 += 1 } }
            let value = root[args[1] == "files" ? "directory" : args[1] == "changes" ? "changes" : "diff"]!
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.observeFiles()
        await store.workingTree.refresh()
        let before = readings.withLock { $0 }
        try Data("ref: refs/heads/next\n".utf8).write(to: metadata.appendingPathComponent("HEAD"), options: .atomic)
        try await eventually { readings.withLock { $0 } > before }
    }

    /// Every `lf` read writes a journal trace and agents build into ignored
    /// directories; neither may schedule the next read.
    @Test func ignoredWritesFromReadsNeverScheduleAnotherRead() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let checkout = URL(fileURLWithPath: directory.resolvingSymlinksInPath().path)
        try git(["init", "-q"], at: checkout)
        try Data(".lf/journal/\n".utf8).write(to: checkout.appendingPathComponent(".git/info/exclude"))
        try Data("target/\nscratch/\n".utf8).write(to: checkout.appendingPathComponent(".gitignore"))
        let tracked = checkout.appendingPathComponent("tracked.txt")
        try Data("one\n".utf8).write(to: tracked)
        try git(["add", "."], at: checkout)
        try git(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"], at: checkout)
        let scratch = checkout.appendingPathComponent("scratch")
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
        let traces = checkout.appendingPathComponent(".lf/journal/traces")
        let target = checkout.appendingPathComponent("target")
        try FileManager.default.createDirectory(at: traces, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        try await Task.sleep(for: .milliseconds(300))
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let readings = OSAllocatedUnfairLock(initialState: 0)
        let store = TaskFilesStore(issue: "TEST-1", cwd: checkout.path, query: RegistryQuery { args, _ in
            if args[1] == "find" {
                return #"{"paths":[],"scratch":["scratch/page.html"],"latest_scratch_page":"scratch/page.html","truncated":false}"#
            }
            if args[1] == "changes" {
                readings.withLock { $0 += 1 }
                let trace = traces.appendingPathComponent(UUID().uuidString)
                try FileManager.default.createDirectory(at: trace, withIntermediateDirectories: true)
                try Data("{}\n".utf8).write(to: trace.appendingPathComponent("events.jsonl"))
                try Data("o".utf8).write(to: target.appendingPathComponent(UUID().uuidString))
            }
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            let value = root[args[1] == "files" ? "directory" : args[1] == "changes" ? "changes" : "diff"]!
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.observeFiles()
        await store.workingTree.refresh()
        try await Task.sleep(for: .seconds(2))
        #expect(readings.withLock { $0 } == 1)
        store.selection = "tracked.txt"
        try Data("<h1>Report</h1>".utf8).write(to: scratch.appendingPathComponent("page.html"))
        try await eventually { store.browser.updated == ["scratch/page.html"] }
        #expect(store.selection == "tracked.txt" && store.documents.isEmpty && readings.withLock { $0 } == 1)
        // A real change re-reads; FSEvents may split it into two batches, but the
        // reads it causes never cause more.
        try Data("two\n".utf8).write(to: tracked)
        try await eventually { readings.withLock { $0 } > 1 }
        try await Task.sleep(for: .seconds(1))
        let settled = readings.withLock { $0 }
        try await Task.sleep(for: .seconds(1))
        #expect(readings.withLock { $0 } == settled)
    }

    private func git(_ args: [String], at directory: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", directory.path] + args
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0)
    }

    @Test func saveKeepsTypingAndUndoWhileAdvancingOnlyTheSubmittedBaseline() async throws {
        let original = try fixture().file
        let document = TaskFileDocument(path: original.path)
        await document.reconcile(original)
        document.editor.insertText("first", replacementRange: NSRange(location: 0, length: 0))
        let submitted = document.text
        let transport = HeldTaskSave()
        let query = RegistryQuery(runWithInput: { _, _, input in try await transport.save(input) },
                                  run: { _, _ in throw RegistryQueryError("unused read") })
        let pending = Task { await document.save(issue: "TEST-1", query: query) }
        for _ in 0..<100 {
            if await transport.waiting { break }
            await Task.yield()
        }
        document.editor.breakUndoCoalescing()
        document.editor.insertText("later", replacementRange: NSRange(location: 0, length: 0))
        let latest = document.text
        await transport.release()
        await pending.value
        #expect(document.snapshot?.content == submitted)
        #expect(document.text == latest)
        #expect(document.dirty)
        #expect(document.saveMessage?.contains("Newer edits remain unsaved") == true)
        #expect(!document.saving)
        #expect(document.editor.undoManager?.canUndo == true)
        document.editor.undoManager?.undo()
        #expect(document.text == submitted)
        #expect(!document.dirty)
        let baseline = try #require(document.snapshot)
        await document.reconcile(TaskFileSnapshot(recoveries: [TaskFileRecovery(directory: "/recovery", changed: true,
                                                                         message: "Late writer")],
            issueIdentifier: baseline.issueIdentifier, taskId: baseline.taskId, path: baseline.path,
            content: baseline.content, data: nil, state: baseline.state, revision: baseline.revision, readOnlyReason: baseline.readOnlyReason, sizeBytes: baseline.sizeBytes))
        #expect(document.editor.undoManager?.canRedo == true)
        #expect(document.recoveries.first?.changed == true)
    }

    @Test func incomingReadDuringSaveRereadsTheSettledDiskName() async throws {
        let document = TaskFileDocument(path: "note.txt")
        await document.reconcile(snapshot("one\ntwo\n"))
        document.editor.insertText("local", replacementRange: NSRange(location: 0, length: 3))
        let transport = HeldTaskSave()
        let final = #"{"issue_identifier":"TEST-1","task_id":"task-files","path":"note.txt","content":"local\nDISK\n","state":"text","revision":"latest","size_bytes":11,"recoveries":[]}"#
        let query = RegistryQuery(runWithInput: { _, _, input in try await transport.save(input) },
                                  run: { _, _ in final })
        let saving = Task { await document.save(issue: "TEST-1", query: query) }
        while !(await transport.waiting) { await Task.yield() }
        await document.reconcile(snapshot("one\ntwo\n"))
        await transport.release()
        await saving.value
        #expect(document.text == "local\nDISK\n")
        #expect(document.snapshot?.revision == "latest")
        #expect(!document.dirty)
    }

    @Test func rejectedSaveReconcilesDiskAndDisclosesRecovery() async throws {
        let original = try fixture().file
        let document = TaskFileDocument(path: original.path)
        await document.reconcile(original)
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 0))
        let response = #"{"published":false,"message":"File changed on disk","file":{"issue_identifier":"TEST-1","task_id":"task-files","path":"scratch/notes.md","content":"external","state":"text","revision":"external","size_bytes":8,"recoveries":[{"directory":"/recovery","changed":true,"message":"Late write retained"}]}}"#
        let query = RegistryQuery(runWithInput: { _, _, _ in response },
                                  run: { _, _ in throw RegistryQueryError("unused read") })
        await document.save(issue: "TEST-1", query: query)
        #expect(document.text == "draftexternal")
        #expect(document.snapshot?.content == "external")
        #expect(document.external == nil)
        #expect(document.recoveries.first?.changed == true)
        #expect(document.saveMessage == "Updated from disk")
        // The Save's own filesystem event rereads content, which carries no history.
        await document.reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files",
            path: original.path, content: "external again", data: nil, state: .text, revision: "again", readOnlyReason: nil, sizeBytes: 14))
        #expect(document.recoveries.first?.changed == true)
    }

    @Test(.requiresDisplay) func draftsKeepUndoSelectionExternalChangesAndGhostty() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace(NSTemporaryDirectory()))
        workspace.multiplexer.newShell(location: workspace.location)
        let identity = try #require(workspace.multiplexer.focusedPane.terminal)
        let terminal = registry.surfaces.view(for: identity)
        terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 350)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = "/bin/cat"
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(identity) }
        let surface = try #require(terminal.surface)
        let store = workspace.files(issue: "TEST-1", cwd: NSTemporaryDirectory(),
                                    query: RegistryQuery { _, _ in throw RegistryQueryError("Fixture is offline") })
        let original = try fixture().file
        let document = store.document(original.path)
        await document.reconcile(original)
        let editor = document.editor
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 500),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let root = NSView(frame: window.contentLayoutRect)
        window.contentView = root
        defer { window.contentView = nil }
        terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 500)
        root.addSubview(terminal)
        document.scroll.frame = CGRect(x: 400, y: 0, width: 700, height: 500)
        root.addSubview(document.scroll)
        #expect(window.makeFirstResponder(editor))
        editor.breakUndoCoalescing()
        editor.setSelectedRange(NSRange(location: (editor.string as NSString).length, length: 0))
        editor.insertText("draft", replacementRange: editor.selectedRange())
        #expect(document.dirty)
        let edited = document.text
        let selection = editor.selectedRange()
        store.selection = original.path
        store.mode = .diff
        store.selection = "new.txt"
        _ = store.document("new.txt")
        store.selection = original.path
        store.mode = .edit
        #expect(store.selectedDocument?.editor === editor)
        #expect(editor.selectedRange() == selection)
        #expect(document.text == edited)

        try #require(editor.undoManager?.canUndo == true)
        editor.undoManager?.undo()
        #expect(editor.string == original.content)
        #expect(document.text == original.content)
        editor.undoManager?.redo()
        #expect(document.text == edited)

        // Mount, hide and reopen the actual browser; its documents belong to the checkout.
        for _ in 0..<2 {
            let browser = NSHostingView(rootView: TaskFilesView(store: store, checkoutBase: nil))
            browser.frame = CGRect(x: 400, y: 0, width: 700, height: 500)
            root.addSubview(browser)
            browser.layoutSubtreeIfNeeded()
            browser.removeFromSuperview()
        }
        #expect(workspace.files(issue: "TEST-1", cwd: NSTemporaryDirectory()) === store)
        #expect(store.document(original.path) === document)
        #expect(document.text == edited)
        #expect(registry.surfaces.view(for: identity) === terminal)
        #expect(terminal.surface == surface)
        let changed = TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files", path: original.path,
                                       content: "external\n", data: nil, state: .text, revision: "different", readOnlyReason: nil, sizeBytes: 9)
        await document.reconcile(changed)
        #expect(document.text == "external\ndraft")
        #expect(document.external == nil)
        editor.undoManager?.undo()
        #expect(document.text == "external\n")
        editor.undoManager?.redo()
        #expect(document.text == "external\ndraft")
    }

    /// The Diff counts files in words, names its refs as code, and the
    /// sidebar counts and marks what you have viewed.
    @Test func theDiffSaysWhatChangedAndWhatYouViewed() async throws {
        let store = try twoFileStore()
        await store.diffReview.loadReview()
        #expect(store.diffReview.changesTitle == "2 changed files")
        let line = try #require(store.diffReview.comparisonLine)
        #expect(String(line.characters).hasSuffix(". Includes uncommitted changes."))
        let named = TaskChangesView.names(line)
        let mono = named.runs.filter { $0.font == Typography.mono }.map { String(named[$0.range].characters) }
        #expect(mono == [String(try #require(store.diffReview.changes).baseCommit.prefix(10))])
        #expect(!named.runs.contains { $0.inlinePresentationIntent != nil })

        store.diffReview.hide("b.swift")
        #expect(store.diffReview.changesTitle == "1 of 2 changed files")
        store.diffReview.unhide("b.swift")

        let sidebar = ChangedFilesSidebar(store: store, current: nil, notice: nil, jump: { _ in })
        #expect(store.diffReview.viewedSummary == nil)
        #expect((try? sidebar.inspect().find(text: "Viewed")) == nil)
        store.diffReview.setViewed(try #require(store.diffReview.shownReview?.files.first), true)
        #expect(store.diffReview.viewedSummary == "1 of 2 viewed")
        _ = try sidebar.inspect().find(text: "1 of 2 viewed")
        _ = try sidebar.inspect().find(text: "Viewed")
        #expect(ChangedFilesSidebar.place("swift/LoopflowMac/Views/TaskBriefView.swift", under: nil)
            == ("TaskBriefView.swift", "swift/LoopflowMac/Views/"))
        #expect(ChangedFilesSidebar.place("swift/LoopflowMac/Views/TaskBriefView.swift", under: "swift/".count)
            == ("TaskBriefView.swift", "LoopflowMac/Views/"))
        #expect(ChangedFilesSidebar.place("README.md", under: nil) == ("README.md", nil))
    }

    /// Layouts are named in words, and the delivery card's link has no typed arrow.
    @Test func theDiffNamesItsChoicesInWords() throws {
        let store = try twoFileStore()
        let picker = ReviewLayoutPicker(store: store, width: 800)
        for layout in ["New code", "Unified", "Side by side"] { _ = try picker.inspect().find(text: layout) }
        let url = try #require(URL(string: "https://github.com/loopflow/loopflow/pull/1949"))
        let card = DeliveryCard(store: store.pullRequest, record: nil, taskOpen: true, url: url, localHead: nil)
        #expect(try card.inspect().find(viewWithAccessibilityIdentifier: "changes-open-github").button().labelView().text().string()
            == "Open on GitHub")
        _ = try card.inspect().find(button: "Read again")
    }

    /// The filename stays visible when the full breadcrumb cannot fit.
    @Test func filesNamesTheOpenFileInTheBreadcrumb() throws {
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/src/loopflow", query: RegistryQuery { _, _ in "{}" })
        store.page = .files
        store.selection = "scratch/plan.md"
        store.selection = "scratch/questions.md"
        store.selection = "swift/TaskRow.swift"
        store.selection = "scratch/plan.md"
        let view = TaskFilesView(store: store, checkoutBase: nil)
        _ = try view.inspect().find(text: "plan.md")
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "files-open")
    }
}
private func fileJSON(_ url: URL) throws -> [String: Any] {
    let data = try? Data(contentsOf: url)
    var value: [String: Any] = ["issue_identifier": "TEST-1", "task_id": "task-files", "path": "note.txt",
                               "recoveries": [], "state": data == nil ? "missing" : "text", "size_bytes": data?.count ?? 0]
    if let data {
        value["content"] = String(decoding: data, as: UTF8.self)
        value["revision"] = data.base64EncodedString()
    }
    return value
}

private actor HeldTaskFileRead {
    private var pending: CheckedContinuation<Void, Never>?
    private var released = false
    var waiting: Bool { pending != nil }

    func read(_ path: String) async throws -> String {
        if path == "first.txt", !released {
            await withCheckedContinuation { pending = $0 }
        }
        let data = try JSONSerialization.data(withJSONObject: [
            "issue_identifier": "TEST-1", "task_id": "task-files", "path": path,
            "recoveries": [], "content": path, "state": "text", "revision": path, "size_bytes": path.utf8.count,
        ])
        return String(decoding: data, as: UTF8.self)
    }

    func release() {
        released = true
        pending?.resume()
        pending = nil
    }
}

private actor HeldTaskSave {
    private var pending: CheckedContinuation<Void, Never>?
    var waiting: Bool { pending != nil }

    func save(_ content: String) async throws -> String {
        await withCheckedContinuation { pending = $0 }
        let data = try JSONSerialization.data(withJSONObject: [
            "published": true, "message": "Saved", "file": [
                "issue_identifier": "TEST-1", "task_id": "task-files", "path": "scratch/notes.md",
                "content": content, "state": "text", "revision": "saved", "size_bytes": content.utf8.count,
                "recoveries": [],
            ],
        ])
        return String(decoding: data, as: UTF8.self)
    }

    func release() { pending?.resume(); pending = nil }
}

private actor RemoteFilesFixture {
    private let stream = AsyncThrowingStream<TaskFilesFrame, any Error>.makeStream()
    private var request: String?
    private var content = "one\ntwo\n"
    private var checkout = "/peer/checkout"
    private var changeOwnerDuringRead = false

    func watch(_ args: [String], cwd: String?) throws -> TaskFilesObservation {
        guard cwd == nil, args.starts(with: ["--machine", "peer", "--repo-id", "plan", "task", "watch-files", "remote-task"]),
              let flag = args.firstIndex(of: "--request") else { throw RegistryQueryError("Not the pinned peer") }
        request = args[flag + 1]
        emit()
        return TaskFilesObservation(frames: stream.stream, cancel: { [stream] in stream.continuation.finish() })
    }
    func change(_ text: String, changeOwnerDuringRead: Bool = false) {
        content = text
        self.changeOwnerDuringRead = changeOwnerDuringRead
        emit()
    }
    func end(stale: Bool) {
        if stale { request = "stale"; emit() }
        else { stream.continuation.finish(throwing: RegistryQueryError("Peer disconnected")) }
    }
    private func emit() {
        guard let request else { return }
        stream.continuation.yield(TaskFilesFrame(request: request, taskID: "remote-task", machineID: "peer",
            checkout: "/peer/checkout", paths: ["note.txt"], ignoredScratch: []))
    }
    func read(_ args: [String], cwd: String?) throws -> String {
        guard cwd == nil, args.starts(with: ["--machine", "peer", "--repo-id", "plan"]) else {
            throw RegistryQueryError("Peer paths must not reach local reads")
        }
        let command = Array(args.dropFirst(4))
        if command.starts(with: ["task", "location"]) {
            let flag = try #require(command.firstIndex(of: "--request"))
            let location = TaskLocationObservation(request: command[flag + 1], repositoryID: "plan", taskID: "portable",
                machineID: "peer", observedAt: 1, location: .recorded(taskID: "remote-task", checkout: checkout))
            return String(decoding: try JSONEncoder().encode([location]), as: UTF8.self)
        }
        guard command.starts(with: ["task", "file", "remote-task"]) else { throw RegistryQueryError("Unexpected operation") }
        if changeOwnerDuringRead { checkout = "/changed-owner" }
        return String(decoding: try JSONSerialization.data(withJSONObject: [
            "recoveries": [], "issue_identifier": "TEST-1", "task_id": "remote-task", "path": "note.txt",
            "content": content, "state": "text", "revision": content, "read_only_reason": NSNull(), "size_bytes": content.utf8.count
        ]), as: UTF8.self)
    }
}
#endif
