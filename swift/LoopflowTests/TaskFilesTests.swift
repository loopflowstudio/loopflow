#if os(macOS) && canImport(GhosttyKit)
import AppKit
import os
import GhosttyKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

private struct FilesFixture: Decodable {
    let directory: TaskDirectory
    let changes: TaskChangesSnapshot
    let diff: TaskDiffSnapshot
    let file: TaskFileSnapshot
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

    private func snapshot(_ text: String, revision: String? = nil) -> TaskFileSnapshot {
        TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files", path: "note.txt",
                         content: text, state: .text, revision: revision ?? text, readOnlyReason: nil, sizeBytes: UInt64(text.utf8.count))
    }

    @Test func directoryPagesRefreshIndependentlyOfComparisonAndRetainDocuments() async throws {
        let query = RegistryQuery { args, _ in
            if args.contains("--files") { throw RegistryQueryError("Task has no active PR") }
            let path = args.contains("--cursor") ? "second.txt" : "first.txt"
            var entries = [["path": path, "kind": "file"]]
            if args.contains("--show-ignored") { entries.append(["path": "ignored.txt", "kind": "file"]) }
            var value: [String: Any] = ["path": "", "entries": entries]
            if !args.contains("--cursor") { value["next_cursor"] = "next" }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        }
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: query)
        store.showsChanges = true
        await store.refresh()
        #expect(store.directories[""]?.entries.map(\.path) == ["first.txt"])
        #expect(store.changesError == "Task has no active PR")
        let document = store.document("first.txt")
        await document.reconcile(snapshot("original"))
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 8))
        store.autosave = false
        await store.loadDirectory("", more: true)
        #expect(store.directories[""]?.entries.map(\.path) == ["first.txt", "second.txt"])
        store.showIgnored = true
        await store.refreshDirectories()
        #expect(store.directories[""]?.entries.map(\.path) == ["first.txt", "ignored.txt"])
        #expect(store.selectedDocument === document)
        #expect(document.text == "draft")
        #expect(document.canSave)
        #expect(store.directoryErrors.isEmpty)
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

    @Test func sameRevisionAccessChangePreservesDraftSelectionAndUndo() async {
        let document = TaskFileDocument(path: "note.txt")
        let original = snapshot("original")
        await document.reconcile(original)
        document.editor.insertText("draft", replacementRange: NSRange(location: 0, length: 8))
        document.editor.setSelectedRange(NSRange(location: 2, length: 1))
        let readOnly = TaskFileSnapshot(recoveries: [], issueIdentifier: original.issueIdentifier,
            taskId: original.taskId, path: original.path, content: original.content, state: .text,
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
        await document.save(issue: store.issue, cwd: store.cwd, query: query)
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
            if args.contains("--files") {
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
        store.showsChanges = true
        await store.refresh()
        #expect(store.directories[""]?.entries.first?.path == "note.txt")
        #expect(store.changesError == "Task has no active PR")
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
        await document.save(issue: store.issue, cwd: store.cwd, query: query)
        #expect(try String(contentsOf: file, encoding: .utf8) == "local\nexternal\n")
        await store.refresh()
        #expect(store.directories[""]?.entries.first?.path == "note.txt")
        #expect(store.directoryErrors.isEmpty)
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
        await observation?.watch("note.txt")
        weak var retained = observation
        try Data("pending".utf8).write(to: file, options: .atomic)
        observation = nil

        // Let queued callbacks and source cancellation finish after teardown.
        try Data("after".utf8).write(to: file, options: .atomic)
        try await Task.sleep(for: .milliseconds(200))
        #expect(retained == nil)
    }

    @Test func taskHeaderUsesRecordedCheckoutAndLeavesGenericWorkspaceIntact() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "ls", "session": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected file/launch request")
            }
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let generic = registry.layout(for: fixtureWorkspace("/src/loopflow"))
        generic.select(fixtureWorkspace("/another-checkout"))
        generic.split(generic.focusedSlotId, axis: .vertical)
        let original = generic.layout
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        for (task, location) in [("issue-now", "loopflow.make-lf-work-the-machine"),
                                 ("issue-review", "loopflow.all-wave-roadmap"),
                                 ("issue-now", "loopflow.make-lf-work-the-machine")] {
            model.select(.task(id: task))
            model.navigation.content = .terminals
            let text = try view.inspect().find(viewWithAccessibilityIdentifier: "task-worktree-location").text().string()
            #expect(text == location)
            #expect(throws: (any Error).self) {
                try view.inspect().find(viewWithAccessibilityIdentifier: "worktree-chip")
            }
            #expect(generic.layout == original)
        }
    }

    @Test func finishingSessionKeepsOtherRepositoryPanes() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let first = registry.workspace(for: fixtureWorkspace("/repo-one/task")).multiplexer
        let other = registry.workspace(for: fixtureWorkspace("/repo-two/task")).multiplexer
        first.load(sessionId: "finished")
        other.load(sessionId: "still-running")
        let retained = other.layout
        registry.removeSessions(["finished"])
        #expect(first.pane(forSessionId: "finished") == nil)
        #expect(other.layout == retained)
    }

    @Test func wirePreservesExactBaseAndLosslessText() throws {
        let fixture = try fixture()
        #expect(fixture.directory.entries.map(\.kind) == [.directory, .file, .symlink])
        #expect(fixture.changes.baseCommit == fixture.diff.baseCommit)
        #expect(fixture.changes.files[0].oldPath == "old.txt")
        #expect(fixture.file.content == "\u{feff}notes\r\n")
        #expect(fixture.file.revision?.count == 64)
        #expect(fixture.save.published)
        #expect(fixture.file.recoveries.first?.changed == true)
    }

    @Test func patchTintsHunkChangesAndLeavesFileHeadersPlain() {
        let patch = "diff --git a/old.txt b/new.txt\n--- a/old.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-- old\n+draft\n same\n"
        let lines = TaskPatchLine.changes(in: patch)
        let source = patch as NSString
        #expect(lines.map { source.substring(with: $0.range) } == ["-- old", "+draft"])
        #expect(lines.map(\.added) == [false, true])
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
        try Data("+edit 1\n".utf8).write(to: file)
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let store = TaskFilesStore(issue: "TEST-1", cwd: directory.path, query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            var value = root[args[1] == "files" ? "directory" : args.contains("--files") ? "changes" : "diff"] as! [String: Any]
            if args[1] == "diff" { value["patch"] = try String(contentsOf: file, encoding: .utf8) }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.selection = "new.txt"
        store.mode = .diff
        store.observeFiles()
        await store.refresh()
        await store.loadDiff()
        #expect(store.diff?.patch == "+edit 1\n")
        try Data("+edit 2\n".utf8).write(to: file, options: .atomic)
        try await eventually { store.diff?.patch == "+edit 2\n" }
        #expect(store.diff?.patch == "+edit 2\n")
        #expect(store.documents.isEmpty)
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
            if args.contains("--files") { readings.withLock { $0 += 1 } }
            let value = root[args[1] == "files" ? "directory" : args.contains("--files") ? "changes" : "diff"]!
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.showsChanges = true
        store.observeFiles()
        await store.refreshChanges()
        let before = readings.withLock { $0 }
        try Data("ref: refs/heads/next\n".utf8).write(to: metadata.appendingPathComponent("HEAD"), options: .atomic)
        try await eventually { readings.withLock { $0 } > before }
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
        let pending = Task { await document.save(issue: "TEST-1", cwd: "/fixture", query: query) }
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
            content: baseline.content, state: baseline.state, revision: baseline.revision, readOnlyReason: baseline.readOnlyReason, sizeBytes: baseline.sizeBytes))
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
        let saving = Task { await document.save(issue: "TEST-1", cwd: "/fixture", query: query) }
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
        await document.save(issue: "TEST-1", cwd: "/fixture", query: query)
        #expect(document.text == "draftexternal")
        #expect(document.snapshot?.content == "external")
        #expect(document.external == nil)
        #expect(document.recoveries.first?.changed == true)
        #expect(document.saveMessage == "Updated from disk")
        // The Save's own filesystem event rereads content, which carries no history.
        await document.reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files",
            path: original.path, content: "external again", state: .text, revision: "again", readOnlyReason: nil, sizeBytes: 14))
        #expect(document.recoveries.first?.changed == true)
    }

    @Test(.requiresDisplay) func draftsKeepUndoSelectionExternalChangesAndGhostty() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = registry.workspace(for: fixtureWorkspace(NSTemporaryDirectory()))
        workspace.multiplexer.newShell()
        let identity = TerminalIdentity.shell(workspace.multiplexer.focusedPaneId)
        let terminal = registry.surfaces.view(for: identity)
        terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 350)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = "/bin/cat"
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(identity) }
        let surface = try #require(terminal.surface)
        let store = workspace.files(taskId: "task-files", issue: "TEST-1", cwd: NSTemporaryDirectory(),
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
        store.mode = .file
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
            let browser = NSHostingView(rootView: TaskFilesView(store: store, prURL: nil, checkoutBase: nil))
            browser.frame = CGRect(x: 400, y: 0, width: 700, height: 500)
            root.addSubview(browser)
            browser.layoutSubtreeIfNeeded()
            browser.removeFromSuperview()
        }
        #expect(workspace.files(taskId: "task-files", issue: "TEST-1", cwd: NSTemporaryDirectory()) === store)
        #expect(store.document(original.path) === document)
        #expect(document.text == edited)
        #expect(registry.surfaces.view(for: identity) === terminal)
        #expect(terminal.surface == surface)
        let changed = TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files", path: original.path,
                                       content: "external\n", state: .text, revision: "different", readOnlyReason: nil, sizeBytes: 9)
        await document.reconcile(changed)
        #expect(document.text == "external\ndraft")
        #expect(document.external == nil)
        editor.undoManager?.undo()
        #expect(document.text == "external\n")
        editor.undoManager?.redo()
        #expect(document.text == "external\ndraft")
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

#endif
