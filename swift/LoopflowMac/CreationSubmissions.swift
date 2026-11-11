// Only immutable submitted creations live here. Outcomes are read from the server.
import Foundation
import Observation
import SwiftUI
import Loopflow

extension CreationSubmissions {
    static let shared = CreationSubmissions(
        directory: WorkCache.homeDirectory.appendingPathComponent("submissions"),
        machine: { try await localMachine() },
        lookup: { try await ServerClient.read(.creationGet(requestID: $0)) },
        send: { try await ServerClient.submittedCreation($0) })

    static func localMachine() async throws -> String {
        struct Identity: Decodable { let id: String }
        return try JSONDecoder().decode(Identity.self, from: Data(try await ServerClient.read(.machineSelf).utf8)).id
    }
}

@MainActor @Observable
final class CreationRecovery {
    static let shared = CreationRecovery()
    struct Item: Identifiable {
        let id: String
        let text: String
        let complete: Bool
        let resource: Resource?
    }
    struct Resource {
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
    private(set) var items: [Item] = []

    init(submissions: CreationSubmissions = .shared) { self.submissions = submissions }

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
                items.append(Item(id: id, text: error.localizedDescription, complete: false, resource: nil))
            }
        }
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
                    guard let receipt = try await submissions.reconcile(record) else { continue }
                    let payload = try JSONSerialization.jsonObject(with: record.payload) as? [String: Any]
                    guard let directory = payload?["directory"] as? String else {
                        throw RegistryQueryError("The saved creation has no repository; its original request is retained.")
                    }
                    let encoder = JSONEncoder()
                    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    let resource = Resource(destination: record.destination, directory: directory,
                        task: receipt.taskId, session: receipt.sessionId,
                        receipt: String(decoding: try encoder.encode(receipt), as: UTF8.self))
                    current.append(Item(id: record.id, text: receipt.explanation,
                        complete: receipt.disposition == .complete, resource: resource))
                } catch {
                    current.append(Item(id: record.id, text: "Creation \(record.id): \(error.localizedDescription)", complete: false, resource: nil))
                }
            }
            // A mounted view may have retired a request while lookup awaited I/O.
            let pending = Set(try await submissions.pending().map(\.id))
            items = current.filter { pending.contains($0.id) }
        } catch { items = [Item(id: "storage", text: error.localizedDescription, complete: false, resource: nil)] }
    }
}

struct CreationRecoveryView: View {
    private let recovery = CreationRecovery.shared
    @State private var inspecting: CreationRecovery.Item?
    var body: some View {
        VStack(alignment: .leading) {
            ForEach(recovery.items) { item in
                HStack {
                    Text(item.text).textSelection(.enabled)
                    if item.resource != nil { Button(item.complete ? "Show result" : "Inspect") { inspecting = item } }
                }
            }
        }
        .padding(recovery.items.isEmpty ? 0 : 12)
        .sheet(item: $inspecting) { item in
            if let resource = item.resource { RecoveredCreationInspector(resource: resource, presented: {
                if item.complete { await recovery.presented([item.id]) }
            }) }
        }
        .task { recovery.start() }
    }
}

private struct RecoveredCreationInspector: View {
    let resource: CreationRecovery.Resource
    let presented: () async -> Void
    @State private var detail: Result<String, Error>?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(resource.task == nil ? "Recovered Session" : "Recovered Task").font(.headline)
            ScrollView {
                Group {
                    switch detail {
                    case .success(let text): Text(text).task { await presented() }
                    case .failure(let error): Text(error.localizedDescription)
                    case nil: Text("Reading current work…")
                    }
                }
                .font(.system(.body, design: .monospaced)).textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            DisclosureGroup("Receipt") {
                ScrollView { Text(resource.receipt).font(.system(.caption, design: .monospaced)).textSelection(.enabled) }
            }
            Button("Done") { dismiss() }
        }
        .padding(20).frame(minWidth: 600, minHeight: 400)
        .task {
            do {
                guard try await CreationSubmissions.localMachine() == resource.destination else {
                    throw RegistryQueryError("The original Machine is unavailable; this request has not moved.")
                }
                if let task = resource.task {
                    let value = try await ServerClient.read(.taskGet(directory: resource.directory, issue: task))
                    let reading = try JSONDecoder().decode(WorkFrame.TaskStatusReading.self, from: Data(value.utf8))
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
                    detail = .success(try JSONDecoder().decode(String.self, from: Data(try await ServerClient.read(.processHistory(id: session)).utf8)))
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
