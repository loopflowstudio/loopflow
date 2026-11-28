#if os(macOS)
import Foundation
import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Terminal links")
struct TerminalLinkTests {
    @Test(arguments: ["https://example.com/pr#discussion", "http://localhost:8080/"])
    func webLinksOpenInTheBrowser(_ target: String) throws {
        #expect(TerminalLink(target: target) == .web(try #require(URL(string: target))))
    }

    @Test func mailWorkAndExternalLinksHaveDistinctRoutes() throws {
        let mail = try #require(URL(string: "mailto:team@example.com?subject=Review"))
        let work = try #require(URL(string: "loopflow://loopflow/task/LOO-1126"))
        let external = try #require(URL(string: "slack://channel?id=C123"))
        #expect(TerminalLink(target: mail.absoluteString) == .mail(mail))
        #expect(TerminalLink(target: work.absoluteString) == .work(work))
        #expect(TerminalLink(target: external.absoluteString) == .externalScheme(external))
    }

    @Test(arguments: [
        "", "https:relative", "/tmp/relative-to-scheme", "scripts/test.py#L12",
        "https://example.com/\u{202e}test", "https://example.com/%E2%80%AEtest",
        "https://example.com/%00", "https://example.com/%zz", "https://user@example.com",
        "file:relative", "file:///tmp/%2e%2e/note", "file:///tmp/note#other", "file:///tmp/note#L0", "file:///tmp/note#L+1",
        "file:///tmp/note#L\(Int.max)", "loopflow://", "loopflow:///",
        "loopflow://open?repo=/tmp", "loopflow://open?view=tasks", "LOOPFLOW://SESSIONS",
        "loopflow://foo/bar/baz", "loopflow://repo/task/", "loopflow://repo/wave/a%2Fb",
        "loopflow://repo:/task/A", "loopflow://repo?session=a&session=b",
        "loopflow://repo?machine=home_11111111111111111111111111111111",
        "loopflow://repo/task/A?session=s&machine=home_11111111111111111111111111111111",
    ])
    func invalidAndControlTargetsAreRefused(_ target: String) {
        guard case .refused = TerminalLink(target: target) else {
            Issue.record("Opened an invalid terminal target")
            return
        }
    }

    @Test(arguments: [
        "loopflow://open/task/LOO-1", "loopflow://my%20repo/wave/my%20wave",
        "loopflow://repo?session=a%2Bb%26c&machine=home_11111111111111111111111111111111",
        "loopflow://update",
    ])
    func workAddressesGoOnlyToTheWorkResolver(_ target: String) throws {
        #expect(TerminalLink(target: target) == .work(try #require(URL(string: target))))
    }

    @Test func hoverShowsWhyAClickWouldBeRefused() {
        #expect(TerminalLink.preview("https://example.com/pr") == "https://example.com/pr")
        #expect(TerminalLink.preview("https://example.com/%E2%80%AEtest") == "Invalid terminal link.")
        #expect(TerminalLink.preview("https://github.com@evil.example/") == "Invalid web link.")
        #expect(TerminalLink.preview("") == nil)
    }

    @Test func checkoutFileLinksKeepTheirLineAndHostname() {
        let link = TerminalLink(target: "file://WORKSTATION.local/tmp/check%20out/scripts/test.py#L12")
        guard case .file(let file) = link else {
            Issue.record("Expected a file link")
            return
        }
        #expect(file.path == "/tmp/check out/scripts/test.py")
        #expect(file.line == 12)
        #expect(file.isLocal(hostName: "workstation.local"))
        #expect(!file.isLocal(hostName: "another.local"))
        #expect(file.checkoutPath(in: "/tmp/check out") == "scripts/test.py")
        #expect(file.checkoutPath(in: "/tmp/check") == nil)
        #expect(TerminalLink(target: "/tmp/file", allowsBarePath: true) ==
                .file(.init(path: "/tmp/file", host: nil, line: nil)))
    }

    @Test func localDocumentsOpenAfterResolvingSymlinks() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        let document = root.appendingPathComponent("notes.txt")
        try Data("Notes".utf8).write(to: document)
        let shortcut = root.appendingPathComponent("shortcut.txt")
        try FileManager.default.createSymbolicLink(at: shortcut, withDestinationURL: document)
        #expect(try TerminalLink.File(path: shortcut.path, host: nil, line: nil).localDocument() ==
                document.resolvingSymlinksInPath())
        let folder = TerminalLink.File(path: root.path, host: nil, line: nil)
        #expect(try folder.localDocument().path == root.resolvingSymlinksInPath().path)
    }

    @Test func localFileLinksRefuseLaunchers() throws {
        let manager = FileManager.default
        let root = manager.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try manager.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? manager.removeItem(at: root) }
        let app = root.appendingPathComponent("Example.app")
        try manager.createDirectory(at: app, withIntermediateDirectories: false)
        let alias = root.appendingPathComponent("app-alias")
        try manager.createSymbolicLink(at: alias, withDestinationURL: app)
        try Data("echo test".utf8).write(to: root.appendingPathComponent("test.command"))
        try Data().write(to: root.appendingPathComponent("launcher.jar"))
        try Data("#!/bin/sh\n".utf8).write(to: root.appendingPathComponent("script"))
        let executable = root.appendingPathComponent("executable.txt")
        try Data("test".utf8).write(to: executable)
        try manager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable.path)
        for name in ["app-alias", "test.command", "launcher.jar", "script", "executable.txt"] {
            let file = TerminalLink.File(path: root.appendingPathComponent(name).path, host: nil, line: nil)
            #expect(throws: TerminalLink.File.OpenError.unsafe) { try file.localDocument() }
        }
        let missing = TerminalLink.File(path: root.appendingPathComponent("missing.txt").path, host: nil, line: nil)
        #expect(throws: TerminalLink.File.OpenError.missing) { try missing.localDocument() }
    }

    @Test @MainActor func localPaneRoutesFromItsTasksCurrentCheckout() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let checkout = root.appendingPathComponent("checkout")
        let folder = checkout.appendingPathComponent("folder")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let script = checkout.appendingPathComponent("test.py")
        let notes = checkout.appendingPathComponent("notes.txt")
        let outside = root.appendingPathComponent("outside.txt")
        for url in [script, notes, outside] { try Data("text".utf8).write(to: url) }
        let shortcut = root.appendingPathComponent("link")
        try FileManager.default.createSymbolicLink(at: shortcut, withDestinationURL: checkout)

        let pane = WorkspaceIdentity(machineId: "local", worktree: shortcut.path)
        let active = SessionsWorkspace.FilesPlace(taskId: "task", machine: nil, label: nil, cwd: checkout.path)
        let moved = SessionsWorkspace.FilesPlace(taskId: "task", machine: "peer", label: nil, cwd: checkout.path)
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Local file links need no remote read.") }
        func open(_ url: URL, from pane: WorkspaceIdentity = pane, files: SessionsWorkspace.FilesPlace? = active,
                  host: String? = nil) async throws -> TerminalLink.File.Destination {
            try await TerminalLink.File(path: url.path, host: host, line: 42)
                .destination(from: pane, localMachine: "local", files: files, query: query)
        }

        #expect(try await open(shortcut.appendingPathComponent("test.py")) == .checkout(path: "test.py", line: 42))
        #expect(try await open(folder) == .document(folder.resolvingSymlinksInPath()))
        #expect(try await open(checkout) == .document(checkout.resolvingSymlinksInPath()))
        #expect(try await open(outside) == .document(outside.resolvingSymlinksInPath()))
        #expect(try await open(notes, files: nil) == .document(notes.resolvingSymlinksInPath()))
        #expect(try await open(notes, files: moved) == .document(notes.resolvingSymlinksInPath()))
        #expect(try await open(notes, from: .init(machineId: "local", worktree: root.path)) == .document(notes.resolvingSymlinksInPath()))
        #expect(try await open(notes, from: .init(machineId: "local", worktree: "")) == .checkout(path: "notes.txt", line: 42))
        await #expect(throws: TerminalLink.File.OpenError.unsafe) { try await open(script, files: moved) }
        await #expect(throws: TerminalLink.File.OpenError.missing) { try await open(checkout.appendingPathComponent("missing")) }
        await #expect(throws: TerminalLink.File.OpenError.anotherMachine) { try await open(notes, host: "another.invalid") }
    }

    @Test @MainActor func remotePaneChecksFoldersOnTheTaskMachine() async throws {
        let active = SessionsWorkspace.FilesPlace(taskId: "task", machine: "peer", label: nil, cwd: "/peer/checkout")
        let pane = WorkspaceIdentity(machineId: "peer", worktree: active.cwd)
        let query = RegistryQuery(serve: { operation in
            switch operation {
            case .taskFiles(issue: "task", path: "script.py", cursor: nil, showIgnored: true):
                throw ServerError(code: "invalid", message: "Task directory read needs a folder.",
                                  data: .object(["reason": .string("file")]))
            case .taskFiles(issue: "task", path: "folder", cursor: nil, showIgnored: true):
                return ReadOutput(#"{"path":"folder","entries":[],"next_cursor":null}"#)
            case .taskFiles(issue: "task", path: "offline", cursor: nil, showIgnored: true):
                throw RegistryQueryError("Peer unavailable.")
            default:
                Issue.record("Unexpected read: \(operation)")
                throw RegistryQueryError("Unexpected read.")
            }
        }) { _, _ in throw RegistryQueryError("Unexpected CLI read.") }

        func open(_ path: String, from pane: WorkspaceIdentity = pane, files: SessionsWorkspace.FilesPlace? = active)
            async throws -> TerminalLink.File.Destination {
            try await TerminalLink.File(path: path, host: "peer.example", line: 42)
                .destination(from: pane, localMachine: "local", files: files, query: query)
        }

        #expect(try await open("/peer/checkout/script.py") == .checkout(path: "script.py", line: 42))
        await #expect(throws: TerminalLink.File.OpenError.remoteDirectory) { try await open("/peer/checkout/folder") }
        await #expect(throws: TerminalLink.File.OpenError.remoteDirectory) { try await open("/peer/checkout") }
        await #expect(throws: TerminalLink.File.OpenError.anotherMachine) { try await open("/peer/checkout-next/file") }
        await #expect(throws: TerminalLink.File.OpenError.anotherMachine) { try await open("/peer/checkout/script.py", files: nil) }
        await #expect(throws: TerminalLink.File.OpenError.anotherMachine) {
            try await open("/peer/checkout/script.py", from: .init(machineId: "old-peer", worktree: active.cwd))
        }
        await #expect(throws: TerminalLink.File.OpenError.anotherMachine) {
            try await open("/peer/checkout/script.py", from: .init(machineId: "peer", worktree: "/peer/old"))
        }
        await #expect(throws: RegistryQueryError.self) { try await open("/peer/checkout/offline") }
    }

    @Test @MainActor func terminalFileRevealsFilesAtTheLinkedLineInATile() async throws {
        let checkout = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: checkout, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: checkout) }
        let file = checkout.appendingPathComponent("notes.md")
        try Data("text".utf8).write(to: file)

        let location = WorkspaceIdentity(machineId: "local", worktree: checkout.path)
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task"),
                                          title: "Task", location: location, localMachineId: "local")
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Opening the pane needs no server read.") }
        let sessions = workspace.sessionStore(repoPath: checkout.path, query: query)
        let shell = try #require(try workspace.newShell())
        let pane = SessionPaneView(pane: shell, isFocused: true, sessions: sessions,
                                   workspace: workspace, model: WorkModel(query: query), onFocus: { _, _ in },
                                   inTile: true)
        await pane.openTerminalLink(.file(.init(path: file.path, host: nil, line: 42)), at: location) {
            Issue.record("Could not open the file link: \($0)")
        }

        let browser = try #require(workspace.multiplexer.browserPane)
        #expect(workspace.multiplexer.zoomedPaneId == browser.id)
        #expect(browser.content.selectedBrowserTab == .files)
        let files = try workspace.files(for: browser.content, task: nil, query: query)
        #expect(files.selection == "notes.md")
        #expect(files.mode == .read)
        #expect(files.visit?.line == 42)
        #expect(files.visit?.showsSource == true)
    }

    @Test @MainActor func aTaskMoveDuringTheRemoteLookupLeavesFilesClosed() async throws {
        let original = WorkspaceIdentity(machineId: "peer", worktree: "/peer/checkout")
        let replacement = WorkspaceIdentity(machineId: "new-peer", worktree: "/another/checkout")
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task"),
                                          title: "Task", location: original, localMachineId: "local")
        let query = RegistryQuery(serve: { _ in
            await MainActor.run { workspace.location = replacement }
            throw ServerError(code: "invalid", message: "Task directory read needs a folder.",
                              data: .object(["reason": .string("file")]))
        }) { _, _ in throw RegistryQueryError("Unexpected CLI read.") }
        let shell = try #require(try workspace.newShell())
        let pane = SessionPaneView(pane: shell, isFocused: true, sessions: workspace.sessionStore(repoPath: "/repo", query: query),
                                   workspace: workspace, model: WorkModel(query: query), onFocus: { _, _ in })

        var notices: [String] = []
        let link = TerminalLink(target: "file://peer.example/peer/checkout/notes.md#L42")
        await pane.openTerminalLink(link, at: original) { notices.append($0) }
        #expect(workspace.multiplexer.browserPane?.content.selectedBrowserTab == nil)
        #expect(notices == ["The Task’s checkout moved before the file could open."])
    }

    @Test @MainActor func terminalWorkLinksOpenResolvedWorkAndReportUnknownTargetsInThePane() async throws {
        let links = try JSONDecoder().decode([WorkLink].self, from: loadFixtureData("work_links.json"))
        let work = try #require(links.first)
        let reply = String(decoding: try JSONEncoder().encode([work]), as: UTF8.self)
        let query = RegistryQuery(serve: { read in
            if read == .workResolve(directory: "/", url: work.url) { return ReadOutput(reply) }
            throw RegistryQueryError("No Task matches that Work link.")
        }) { _, _ in throw RegistryQueryError("Unexpected CLI read.") }
        let location = WorkspaceIdentity(machineId: "local", worktree: "/repo")
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .repository, id: "repo"),
                                          title: "Repository", location: location, localMachineId: "local")
        let model = WorkModel(query: query)
        var opened: [WorkLink] = []
        model.openResolvedWorkLink = { opened.append($0) }
        let pane = SessionPaneView(pane: workspace.multiplexer.focusedPane, isFocused: true,
                                   sessions: workspace.sessionStore(repoPath: "/repo", query: query),
                                   workspace: workspace, model: model, onFocus: { _, _ in })

        var notices: [String] = []
        await pane.openTerminalLink(TerminalLink(target: work.url), at: location) { notices.append($0) }
        await pane.openTerminalLink(TerminalLink(target: "loopflow://loopflow/task/missing"), at: location) { notices.append($0) }
        #expect(opened == [work])
        #expect(notices == ["No Task matches that Work link."])
    }
}
#endif
