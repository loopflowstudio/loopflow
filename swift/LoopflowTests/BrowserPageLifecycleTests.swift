#if os(macOS)
import AppKit
import Foundation
import Testing
import WebKit
@testable import Loopflow
@testable import LoopflowMac

@Suite("Browser page lifetime", .serialized)
@MainActor
struct BrowserPageLifecycleTests {
    @Test func reuseDetachAndFinalReleaseHaveRedactedDiagnostics() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let log = DesktopLog(url: directory.appendingPathComponent("pages.log"))
        log.begin()
        var pages: BrowserPages? = BrowserPages(workspace: "task:test", log: log)
        let url = try #require(URL(string: "https://user:password@private.example/secret?token=credential#fragment"))
        weak var released: WKWebView?
        autoreleasepool {
            let page = pages!.view(for: "tab", url: url)
            released = page
            #expect(pages!.view(for: "tab", url: url) === page)
            let first = NSView()
            let second = NSView()
            first.addSubview(page)
            page.removeFromSuperview()
            second.addSubview(page)
            pages!.release("tab")
            // Removing pool ownership does not prove native destruction.
            #expect(released != nil)
            page.removeFromSuperview()
            page.stopLoading()
        }
        #expect(released == nil)
        pages = nil
        log.flush()
        let lines = try String(contentsOf: log.url, encoding: .utf8)
        for forbidden in ["user", "password", "private.example", "/secret", "token", "credential", "fragment"] {
            #expect(!lines.contains(forbidden))
        }
        #expect(lines.components(separatedBy: "event=create").count == 2)
        #expect(lines.contains("scheme=https"))
        #expect(lines.contains("event=retire"))
        #expect(lines.contains("event=detach"))
        #expect(lines.contains("event=final-release"))
        #expect(lines.contains("reason=undo-replaced"))
        #expect(lines.contains("browser workspace released"))
    }

    @Test func ownerLossIsDistinguishedFromCloseReplacement() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let log = DesktopLog(url: directory.appendingPathComponent("pages.log"))
        log.begin()
        var pages: BrowserPages? = BrowserPages(workspace: "task:test", log: log)
        weak var released: WKWebView?
        autoreleasepool {
            let page = pages!.view(for: "tab", url: URL(string: "https://example.invalid")!)
            released = page
            page.stopLoading()
            pages = nil
            #expect(released != nil)
        }
        #expect(released == nil)
        log.flush()
        let lines = try String(contentsOf: log.url, encoding: .utf8)
        #expect(lines.contains("event=final-release"))
        #expect(lines.contains("reason=workspace-loss"))
    }

    @Test func addressedCloseReplacesUndoWithoutDiscardingTheLatestPage() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let work = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")
        let workspace = registry.open(work, title: "Task", location: nil)
        let firstURL = URL(string: "https://first.invalid")!
        let secondURL = URL(string: "https://second.invalid")!
        workspace.showBrowser(.url(id: "first", url: firstURL))
        let pane = try #require(workspace.showBrowser(.url(id: "second", url: secondURL)))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution") })
        func close(_ tab: String?) throws {
            let target = DesktopPaneTarget(repository: work.repository, window: "window", work: work,
                pane: pane.id, incarnation: pane.incarnation, tab: tab)
            _ = try DesktopAction.perform(DesktopActionRequest("close", at: target), model: model, workspaces: registry)
        }
        weak var discarded: WKWebView?
        autoreleasepool {
            let first = workspace.pages.view(for: "first", url: firstURL)
            first.stopLoading()
            discarded = first
        }
        let second = workspace.pages.view(for: "second", url: secondURL)
        defer { second.stopLoading() }
        try close("first") // Addressed background tab, not selected second tab.
        #expect(discarded != nil)
        try close("second")
        #expect(discarded == nil)
        workspace.multiplexer.undoClose()
        #expect(workspace.pages.view(for: "second", url: secondURL) === second)
        #expect(workspace.multiplexer.browserPane?.content.selectedTabID == "second")
        try close("second")
        workspace.showBrowser(.files)
        let session = try #require(workspace.multiplexer.sessionPane)
        workspace.closePane(session.id) // Hiding a pane also replaces the Undo entry.
        #expect(workspace.multiplexer.lastClosed == .pane(session.id))
    }

    @Test(arguments: [false, true])
    func anotherPaneDisplacesBrowserUndo(shell: Bool) throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let work = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")
        let location = WorkspaceIdentity(machineId: "local", worktree: "/task")
        let workspace = registry.open(work, title: "Task", location: location)
        let url = URL(string: "https://closed.invalid")!
        let browser = try #require(workspace.showBrowser(.url(id: "closed", url: url)))
        weak var discarded: WKWebView?
        autoreleasepool {
            let page = workspace.pages.view(for: "closed", url: url)
            page.stopLoading()
            discarded = page
        }
        workspace.closePane(browser.id)
        #expect(discarded != nil)
        workspace.showBrowser(.files)
        let replacement = try #require(shell
            ? workspace.multiplexer.newShell(location: location)
            : workspace.multiplexer.sessionPane)
        let target = DesktopPaneTarget(repository: work.repository, window: "window", work: work,
            pane: replacement.id, incarnation: replacement.incarnation, tab: nil)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution") })
        _ = try DesktopAction.perform(DesktopActionRequest("close", at: target), model: model, workspaces: registry)
        #expect(discarded == nil)
    }

    @Test func unavailableLogDoesNotChangePageOwnership() {
        let log = DesktopLog(url: URL(fileURLWithPath: "/dev/null/browser.log"))
        log.begin()
        let pages = BrowserPages(log: log)
        let url = URL(string: "https://example.invalid")!
        let page = pages.view(for: "tab", url: url)
        defer { page.stopLoading() }
        log.flush()
        #expect(pages.view(for: "tab", url: url) === page)
        pages.release("tab")
        log.flush()
    }
}
#endif
