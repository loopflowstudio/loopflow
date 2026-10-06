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
    case chooseFlow(String)
    case rename(String)
    case bind(String)
    case monitor(String)
}

struct TaskLink: Equatable, Sendable {
    let issue: String
    let repo: String?
    let session: String?

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
        guard items.allSatisfy({ ["repo", "session"].contains($0.name) && $0.value?.isEmpty == false }),
              Set(items.map(\.name)).count == items.count else {
            throw RegistryQueryError("Task links accept one optional repo and session query")
        }
        self.issue = issue
        repo = items.first(where: { $0.name == "repo" })?.value
        session = items.first(where: { $0.name == "session" })?.value
    }
}

/// Delivers to one repository window. A cold-start request waits for its first mounted window.
@MainActor
final class WorkLinkRouter {
    private struct Target {
        weak var window: NSWindow?
        let contains: (URL) -> Bool
        let receive: (URL) -> Void
    }
    private var targets: [UUID: Target] = [:]
    private var pending: URL?

    func register(_ id: UUID, window: NSWindow, contains: @escaping (URL) -> Bool = { _ in false },
                  receive: @escaping (URL) -> Void) {
        targets[id] = Target(window: window, contains: contains, receive: receive)
        if let pending {
            self.pending = nil
            receive(pending)
        }
    }

    func remove(_ id: UUID) { targets[id] = nil }

    /// Returns false when a repository window must be opened.
    @discardableResult
    func deliver(_ url: URL) -> Bool {
        targets = targets.filter { $0.value.window != nil }
        let ordered = NSApp.orderedWindows
        let orderedTargets = ordered.compactMap { window in
            targets.values.first { $0.window === window }
        }
        let target = orderedTargets.first { $0.contains(url) }
            ?? targets.values.first { $0.contains(url) }
            ?? orderedTargets.first ?? targets.values.first
        guard let target, let window = target.window else {
            let windowRequested = pending != nil
            pending = url
            return windowRequested
        }
        window.makeKeyAndOrderFront(nil)
        target.receive(url)
        return true
    }
}

struct WorkLinkReceiver: NSViewRepresentable {
    let router: WorkLinkRouter
    let contains: (URL) -> Bool
    let receive: (URL) -> Void

    func makeNSView(context: Context) -> Receiver {
        Receiver(router: router, contains: contains, receive: receive)
    }
    func updateNSView(_ view: Receiver, context: Context) {
        view.contains = contains
        view.receive = receive
    }
    static func dismantleNSView(_ view: Receiver, coordinator: ()) { view.router.remove(view.id) }

    final class Receiver: NSView {
        let id = UUID()
        let router: WorkLinkRouter
        var contains: (URL) -> Bool
        var receive: (URL) -> Void
        init(router: WorkLinkRouter, contains: @escaping (URL) -> Bool, receive: @escaping (URL) -> Void) {
            self.router = router
            self.contains = contains
            self.receive = receive
            super.init(frame: .zero)
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) is unsupported") }
        override func viewDidMoveToWindow() {
            router.remove(id)
            if let window {
                router.register(id, window: window, contains: { [weak self] url in self?.contains(url) == true }) {
                    [weak self] url in self?.receive(url)
                }
            }
        }
    }
}
