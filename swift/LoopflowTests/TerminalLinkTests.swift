#if os(macOS)
import Foundation
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
        "loopflow://update", "loopflow://open?repo=/tmp", "loopflow://%75pdate",
        "LOOPFLOW://SESSIONS", "loopflow://portfolio",
    ])
    func invalidAndControlTargetsAreRefused(_ target: String) {
        guard case .refused = TerminalLink(target: target) else {
            Issue.record("Opened an invalid terminal target")
            return
        }
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
}
#endif
