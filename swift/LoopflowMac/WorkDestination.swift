import AppKit
import Foundation
import Loopflow
import SwiftUI

/// Inspection destinations never confer launch or process-control authority.
enum WorkDestination: Hashable {
    case wave(String)
    case task(String)
    case session(String)
    case flow(String)
    case workflow(String)
    case rename(String)
    case bind(String)
    case flowLog(String)
}

struct TaskLink: Equatable, Sendable {
    let issue: String
    let repo: String?
    let session: String?
    let diff: Bool

    init(issue: String, repo: String?, session: String? = nil, diff: Bool = false) {
        self.issue = issue
        self.repo = repo
        self.session = session
        self.diff = diff
    }

    var url: URL? {
        var components = URLComponents()
        components.scheme = "loopflow"
        components.host = "task"
        components.path = "/" + issue
        components.queryItems = [("repo", repo), ("session", session), ("diff", diff ? "true" : nil)].compactMap { name, value in
            value.map { URLQueryItem(name: name, value: $0) }
        }
        return components.url
    }

    init(url: URL) throws {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              components.scheme == "loopflow", components.host == "task",
              components.user == nil, components.password == nil, components.port == nil,
              components.fragment == nil else {
            throw RegistryQueryError("Invalid Task link")
        }
        let path = components.percentEncodedPath
        guard path.hasPrefix("/"),
              let issue = String(path.dropFirst()).removingPercentEncoding,
              !issue.isEmpty, !issue.contains("/"), !issue.contains("\\"),
              !issue.unicodeScalars.contains(where: { CharacterSet.whitespacesAndNewlines.union(.controlCharacters).contains($0) }) else {
            throw RegistryQueryError("Task links require one issue identifier")
        }
        let items = components.queryItems ?? []
        guard items.allSatisfy({ ["repo", "session", "diff"].contains($0.name) && $0.value?.isEmpty == false }),
              Set(items.map(\.name)).count == items.count,
              items.first(where: { $0.name == "diff" }).map({ $0.value == "true" }) ?? true else {
            throw RegistryQueryError("Task links accept one optional repo, session and diff=true query")
        }
        self.issue = issue
        repo = items.first(where: { $0.name == "repo" })?.value
        session = items.first(where: { $0.name == "session" })?.value
        diff = items.contains { $0.name == "diff" }
    }
}

struct WorkLinkReceiver: NSViewRepresentable {
    let router: WorkLinkRouter
    let repository: String
    let inspect: (UUID) -> DesktopWindowInspection
    let controlPane: (DesktopPaneCommand) throws -> Void
    let readText: (DesktopTextRequest) throws -> DesktopTextReading
    let receive: (URL) async -> Void

    func makeNSView(context: Context) -> Receiver {
        Receiver(router: router, repository: repository, inspect: inspect, controlPane: controlPane, readText: readText, receive: receive)
    }
    func updateNSView(_ view: Receiver, context: Context) {
        view.receive = receive
        view.inspect = inspect
        view.controlPane = controlPane
        view.readText = readText
    }
    static func dismantleNSView(_ view: Receiver, coordinator: ()) {
        view.router.remove(view.id, repository: view.repository)
    }

    final class Receiver: NSView {
        let id = UUID()
        let router: WorkLinkRouter
        let repository: String
        var inspect: (UUID) -> DesktopWindowInspection
        var controlPane: (DesktopPaneCommand) throws -> Void
        var readText: (DesktopTextRequest) throws -> DesktopTextReading
        var receive: (URL) async -> Void

        init(router: WorkLinkRouter, repository: String, inspect: @escaping (UUID) -> DesktopWindowInspection,
             controlPane: @escaping (DesktopPaneCommand) throws -> Void,
             readText: @escaping (DesktopTextRequest) throws -> DesktopTextReading,
             receive: @escaping (URL) async -> Void) {
            self.router = router
            self.repository = repository
            self.receive = receive
            self.inspect = inspect
            self.controlPane = controlPane
            self.readText = readText
            super.init(frame: .zero)
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) is unsupported") }
        override func viewDidMoveToWindow() {
            router.remove(id, repository: repository)
            if let window {
                router.register(id, repository: repository, focus: { [weak window] in
                    window?.makeKeyAndOrderFront(nil)
                }, inspect: inspect, controlPane: { [weak self] request in
                    guard let self else { throw RegistryQueryError("The repository window is unavailable.") }
                    try self.controlPane(request)
                }, readText: { [weak self] request in
                    guard let self else { throw RegistryQueryError("The repository window is unavailable.") }
                    return try self.readText(request)
                }) { [weak self] link in
                    await self?.receive(link)
                }
            }
        }
    }
}
