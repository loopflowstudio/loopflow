import AppKit
import Foundation
import Loopflow
import Observation
import SwiftUI

/// Inspection destinations never confer launch or process-control authority.
enum WorkspaceDestination: Hashable {
    case wave(String)
    case task(String)
    case session(String)
    case flow(String)
    case chooseFlow(String)
    case rename(String)
    case monitor(String)
}

struct TaskLink: Equatable, Sendable {
    let issue: String
    let repo: String?

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
        guard items.allSatisfy({ $0.name == "repo" }), items.count <= 1,
              items.first.map({ $0.value?.isEmpty == false }) ?? true else {
            throw RegistryQueryError("Task links accept one optional repo query")
        }
        self.issue = issue
        repo = items.first?.value
    }
}

/// Delivers to one Podium. A cold-start request waits for its first mounted window.
@MainActor
final class WorkspaceLinkRouter {
    private struct Target {
        weak var window: NSWindow?
        let receive: (URL) -> Void
    }
    private var targets: [UUID: Target] = [:]
    private var pending: URL?

    func register(_ id: UUID, window: NSWindow, receive: @escaping (URL) -> Void) {
        targets[id] = Target(window: window, receive: receive)
        if let pending {
            self.pending = nil
            receive(pending)
        }
    }

    func remove(_ id: UUID) { targets[id] = nil }

    /// Returns false when a Podium window must be opened.
    @discardableResult
    func deliver(_ url: URL) -> Bool {
        targets = targets.filter { $0.value.window != nil }
        let ordered = NSApp.orderedWindows
        let target = ordered.lazy.compactMap { window in
            self.targets.values.first { $0.window === window }
        }.first ?? targets.values.first
        guard let target, let window = target.window else {
            pending = url
            return false
        }
        window.makeKeyAndOrderFront(nil)
        target.receive(url)
        return true
    }
}

struct WorkspaceLinkReceiver: NSViewRepresentable {
    let router: WorkspaceLinkRouter
    let receive: (URL) -> Void

    func makeNSView(context: Context) -> Receiver {
        Receiver(router: router, receive: receive)
    }
    func updateNSView(_ view: Receiver, context: Context) { view.receive = receive }
    static func dismantleNSView(_ view: Receiver, coordinator: ()) { view.router.remove(view.id) }

    final class Receiver: NSView {
        let id = UUID()
        let router: WorkspaceLinkRouter
        var receive: (URL) -> Void
        init(router: WorkspaceLinkRouter, receive: @escaping (URL) -> Void) {
            self.router = router
            self.receive = receive
            super.init(frame: .zero)
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) is unsupported") }
        override func viewDidMoveToWindow() {
            router.remove(id)
            if let window {
                router.register(id, window: window) { [weak self] url in self?.receive(url) }
            }
        }
    }
}
