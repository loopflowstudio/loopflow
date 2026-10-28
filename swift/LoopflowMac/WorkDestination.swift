import AppKit
import Foundation
import Loopflow
import SwiftUI

/// Work destinations and scoped actions; navigation preserves the retained workspace.
enum WorkDestination: Hashable {
    case repository
    case openRepository(String)
    case wave(String)
    case task(String)
    case graph(String)
    case rename(String)
    case bind(String)
    case graphLog(String)
}

/// A Task destination already resolved to this Machine's repository locator.
/// Public addresses are parsed and resolved only by Rust (`lf open --link`);
/// this value travels between the app's router and repository windows.
struct TaskLink: Equatable, Sendable {
    let issue: String
    let repo: String?
    let session: String?
    let diff: Bool
    let machine: String?
    let repository: String?
    /// Opening its conversation, rather than the Task's details.
    let conversation: Bool
    /// The public address Rust resolved, when this link arrived as one.
    private let workAddress: String?

    init(issue: String, repo: String?, session: String? = nil, diff: Bool = false, machine: String? = nil,
         repository: String? = nil, conversation: Bool = false, address: String? = nil) {
        self.issue = issue
        self.repo = repo
        self.session = session
        self.diff = diff
        self.machine = machine
        self.repository = repository
        self.conversation = conversation
        workAddress = address
    }

    /// The Work address for opening receipts.
    var address: String {
        workAddress ?? Self.address(repository: repo.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "repository", task: issue)
    }

    /// An external Work link always opens its conversation.
    init?(_ link: WorkLink) {
        guard link.target.kind == .task, let issue = link.target.task else { return nil }
        self.init(issue: issue, repo: link.target.repositoryPath, session: link.target.session, diff: link.target.diff,
                  machine: link.target.executionMachine,
                  repository: link.target.executionMachine == nil ? nil : link.target.repositoryId,
                  conversation: true, address: link.url)
    }

    func with(repo: String) -> TaskLink {
        TaskLink(issue: issue, repo: repo, session: session, diff: diff, machine: machine,
                 repository: repository, conversation: conversation, address: workAddress)
    }

    private static func address(repository: String, task: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~"))
        func encode(_ value: String) -> String {
            value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
        }
        return "loopflow://\(encode(repository))/task/\(encode(task))"
    }
}

/// A Work destination delivered to its repository window: a Task, or a Wave
/// whose primary conversation the window selects.
enum LinkedWork: Equatable, Sendable {
    case task(TaskLink)
    /// `address` is the public Work address when the Wave arrived as one.
    case wave(id: String, address: String?)
    /// A selection expression from `lf desktop view`, shown in the repository window.
    case view(expression: String, address: String)

    /// An external Work link resolved by Rust; a repository root delivers nothing.
    init?(_ link: WorkLink) {
        switch link.target.kind {
        case .task: guard let task = TaskLink(link) else { return nil }; self = .task(task)
        case .wave: guard let wave = link.target.wave else { return nil }; self = .wave(id: wave, address: link.url)
        case .repository: return nil
        }
    }

    var address: String? {
        switch self {
        case .task(let link): link.address
        case .wave(_, let address): address
        case .view(_, let address): address
        }
    }

    func with(repo: String) -> LinkedWork {
        switch self {
        case .task(let link): .task(link.with(repo: repo))
        case .wave, .view: self
        }
    }
}

struct WorkLinkReceiver: NSViewRepresentable {
    let router: WorkLinkRouter
    let repository: String
    let openingRequests: Set<UUID>
    let inspect: (String, UUID) -> DesktopWindowInspection
    let perform: (DesktopActionRequest) throws -> DesktopActionOutcome
    let readText: (DesktopTextRequest) throws -> DesktopTextReading
    let receive: (LinkedWork) async -> Void

    func makeNSView(context: Context) -> Receiver {
        Receiver(router: router, repository: repository, openingRequests: openingRequests, inspect: inspect, perform: perform, readText: readText, receive: receive)
    }
    func updateNSView(_ view: Receiver, context: Context) {
        view.receive = receive
        view.inspect = inspect
        view.perform = perform
        view.readText = readText
    }
    static func dismantleNSView(_ view: Receiver, coordinator: ()) {
        view.router.remove(view.id, repository: view.repository)
    }

    final class Receiver: NSView {
        let id = UUID()
        let router: WorkLinkRouter
        let repository: String
        let openingRequests: Set<UUID>
        var inspect: (String, UUID) -> DesktopWindowInspection
        var perform: (DesktopActionRequest) throws -> DesktopActionOutcome
        var readText: (DesktopTextRequest) throws -> DesktopTextReading
        var receive: (LinkedWork) async -> Void

        init(router: WorkLinkRouter, repository: String, openingRequests: Set<UUID>, inspect: @escaping (String, UUID) -> DesktopWindowInspection,
             perform: @escaping (DesktopActionRequest) throws -> DesktopActionOutcome,
             readText: @escaping (DesktopTextRequest) throws -> DesktopTextReading,
             receive: @escaping (LinkedWork) async -> Void) {
            self.router = router
            self.repository = repository
            self.openingRequests = openingRequests
            self.receive = receive
            self.inspect = inspect
            self.perform = perform
            self.readText = readText
            super.init(frame: .zero)
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) is unsupported") }
        override func viewDidMoveToWindow() {
            router.remove(id, repository: repository)
            if let window {
                router.register(id, repository: repository, openingRequests: openingRequests, focus: { [weak window] in
                    window?.makeKeyAndOrderFront(nil)
                }, inspect: inspect, perform: { [weak self] request in
                    guard let self else { throw DesktopTargetError("The repository window is unavailable.") }
                    return try self.perform(request)
                }, readText: { [weak self] request in
                    guard let self else { throw DesktopTargetError("The repository window is unavailable.") }
                    return try self.readText(request)
                }) { [weak self] link in
                    await self?.receive(link)
                }
            }
        }
    }
}
