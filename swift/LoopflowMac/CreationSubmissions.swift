// Receipts open the recovered work. A confirmed refusal can reopen its saved draft.
import Foundation
import Observation
import SwiftUI
import Loopflow

extension RefusedTaskDraft {
    var action: NewTaskAction { run ? .start : .file }
}

extension CreationSubmissions {
    static let shared = CreationSubmissions(
        directory: WorkCache.homeDirectory.appendingPathComponent("submissions"),
        machine: { try await localMachine() },
        lookup: { try await ServerClient.read(.creationGet(requestID: $0)) },
        send: { try await ServerClient.submittedCreation($0) })

    static func localMachine() async throws -> String {
        try await RegistryQueryLocal.shared.localMachineId()
    }
}

@MainActor @Observable
final class CreationRecovery {
    static let shared = CreationRecovery()
    struct Item: Identifiable {
        let id: String
        let text: String
        enum Action {
            case inspect(Resource)
            case edit(RefusedTaskDraft)
        }
        var action: Action?
    }
    struct Resource: Identifiable {
        let id: String
        let complete: Bool
        let destination: String
        let directory: String
        let task: String?
        let session: String?
        let receipt: String
    }
    enum Presentation: Hashable {
        case task(String)
        case session(String)
    }
    private let submissions: CreationSubmissions
    private(set) var presentations: [String: Presentation] = [:]
    /// Saved creations still waiting for someone to see them; each stands as a
    /// condition Notice in every window that reports them.
    private(set) var items: [Item] = [] {
        didSet { postNotices(replacing: oldValue) }
    }
    /// The recovered creation whose inspector is open.
    var inspecting: Resource?
    @ObservationIgnored private var centers: [WeakCenter] = []

    private struct WeakCenter {
        weak var center: NoticeCenter?
        let editDraft: (RefusedTaskDraft) -> Void
    }

    init(submissions: CreationSubmissions = .shared) { self.submissions = submissions }

    /// `notices` shows each recovered creation until it is presented.
    func report(to notices: NoticeCenter, editDraft: @escaping (RefusedTaskDraft) -> Void) {
        centers.removeAll { $0.center == nil || $0.center === notices }
        centers.append(WeakCenter(center: notices, editDraft: editDraft))
        for item in items { notices.post(notice(item, editDraft: editDraft)) }
    }

    private func notice(_ item: Item, editDraft: @escaping (RefusedTaskDraft) -> Void) -> Notice {
        let action: Notice.Action? = switch item.action {
        case .edit(let draft): Notice.Action("Edit draft") { editDraft(draft) }
        case .inspect(let resource):
            Notice.Action(resource.complete ? "Show result" : "Inspect") { [weak self] in self?.inspecting = resource }
        case nil: nil
        }
        return Notice(item.text, key: "creation-\(item.id)", action: action, kind: .condition)
    }

    private func postNotices(replacing old: [Item]) {
        let current = Set(items.map(\.id))
        for window in centers {
            guard let center = window.center else { continue }
            for gone in old where !current.contains(gone.id) { center.clear(key: "creation-\(gone.id)") }
            for item in items { center.post(notice(item, editDraft: window.editDraft)) }
        }
    }

    func awaitingPresentation(_ id: String, of resource: Presentation) {
        presentations[id] = resource
        items.removeAll { $0.id == id }
    }

    func requests(for resource: Presentation) -> [String] {
        presentations.filter { $0.value == resource }.keys.sorted()
    }

    func presented(_ ids: [String]) async {
        for id in ids {
            do {
                try Task.checkCancellation()
                try await submissions.presented(id)
                presentations.removeValue(forKey: id)
                items.removeAll { $0.id == id }
            } catch is CancellationError { return }
            catch {
                presentations.removeValue(forKey: id)
                items.removeAll { $0.id == id }
                items.append(Item(id: id, text: error.localizedDescription))
            }
        }
    }

    func discardDraft(_ id: String) async throws {
        try await submissions.discardDraft(id)
        items.removeAll { $0.id == id }
    }

    private var refreshing = false
    private var observation: Task<Void, Never>?

    func start() {
        guard observation == nil else { return }
        observation = Task {
            while !Task.isCancelled {
                await refresh()
                do { try await Task.sleep(for: .seconds(5)) } catch { return }
            }
        }
    }

    func refresh() async {
        guard !refreshing else { return }
        refreshing = true
        defer { refreshing = false }
        do {
            let records = try await submissions.pending()
            var current: [Item] = []
            for record in records {
                do {
                    guard let receipt = try await submissions.reconcile(record, keepingRefusedDraft: true) else { continue }
                    let payload = try JSONSerialization.jsonObject(with: record.payload) as? [String: Any]
                    guard let directory = payload?["directory"] as? String else {
                        throw RegistryQueryError("The saved creation has no repository; its original request is retained.")
                    }
                    let encoder = JSONEncoder()
                    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    let resource = Resource(id: record.id, complete: receipt.disposition == .complete,
                        destination: record.destination, directory: directory,
                        task: receipt.taskId, session: receipt.sessionId,
                        receipt: String(decoding: try encoder.encode(receipt), as: UTF8.self))
                    current.append(Item(id: record.id, text: receipt.explanation, action: .inspect(resource)))
                } catch let draft as RefusedTaskDraft {
                    current.append(Item(id: draft.id, text: "\(draft.action.refusalTitle) \(draft.localizedDescription)",
                                        action: .edit(draft)))
                } catch {
                    current.append(Item(id: record.id, text: "Creation \(record.id): \(error.localizedDescription)"))
                }
            }
            // A mounted view may have retired a request while lookup awaited I/O.
            let pending = Set(try await submissions.pending().map(\.id))
            items = current.filter { pending.contains($0.id) }
        } catch { items = [Item(id: "storage", text: error.localizedDescription)] }
    }
}

/// A window's Notice reports recovered creations; their action opens the inspector.
struct CreationNotices: ViewModifier {
    let notices: NoticeCenter
    let editDraft: (RefusedTaskDraft) -> Void
    @Bindable private var recovery = CreationRecovery.shared

    func body(content: Content) -> some View {
        content
            .sheet(item: $recovery.inspecting) { resource in
                RecoveredCreationInspector(resource: resource) {
                    if resource.complete { await recovery.presented([resource.id]) }
                }
            }
            .task(id: ObjectIdentifier(notices)) {
                recovery.report(to: notices, editDraft: editDraft)
                recovery.start()
            }
    }
}

private struct RecoveredCreationInspector: View {
    let resource: CreationRecovery.Resource
    let presented: () async -> Void
    @State private var detail: Result<String, Error>?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text(resource.task == nil ? "Recovered Session" : "Recovered Task").font(Typography.textStrong)
            ScrollView {
                Group {
                    switch detail {
                    case .success(let text): Text(text).task { await presented() }
                    case .failure(let error): FailureLine("Current work not read.", machine: error.localizedDescription)
                    case nil: Text("Reading current work…")
                    }
                }
                .font(Typography.mono).textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            DisclosureGroup("Receipt") {
                ScrollView { Text(resource.receipt).font(Typography.mono).textSelection(.enabled) }
            }
            Button("Done") { dismiss() }.buttonStyle(LoopflowButtonStyle())
        }
        .padding(Spacing.xl).frame(minWidth: 600, minHeight: 400)
        .task {
            do {
                guard try await CreationSubmissions.localMachine() == resource.destination else {
                    throw RegistryQueryError("The original Machine is unavailable; this request has not moved.")
                }
                if let task = resource.task {
                    let value = try await ServerClient.read(.taskGet(directory: resource.directory, issue: task))
                    let reading = try value.decode(WorkFrame.TaskStatusReading.self)
                    guard let row = reading.row else {
                        throw RegistryQueryError("The recovered Task is no longer available on its original Machine.")
                    }
                    var lines = ["\(row.task.identifier) · \(row.task.name)", row.section.rawValue.capitalized, row.task.description]
                    if let graph = reading.overview?.graph {
                        lines.append("Graph \(graph.name): \(graph.position.step)")
                    }
                    if let overview = reading.overview {
                        lines.append(contentsOf: overview.sessions.map { "Conversation: \($0.title) (\($0.id))" })
                        lines.append(contentsOf: overview.graphProcesses.map { "Graph \($0.name): \($0.state.rawValue)" })
                    }
                    detail = .success(lines.filter { !$0.isEmpty }.joined(separator: "\n\n"))
                } else if let session = resource.session {
                    detail = .success(try await RegistryQueryLocal.shared.sessionHistoryText(id: session))
                }
            } catch { detail = .failure(error) }
        }
    }
}

// A mounted result retires the submitted request. Navigation and socket replies
// alone cannot do so; a cancelled or failed presentation remains recoverable.
struct CreationPresentation: ViewModifier {
    let recovery: CreationRecovery
    let resource: CreationRecovery.Presentation
    var visible = true

    func body(content: Content) -> some View {
        let ids = visible ? recovery.requests(for: resource) : []
        content.task(id: ids) { await recovery.presented(ids) }
    }
}
