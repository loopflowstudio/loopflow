// Only immutable submitted creations live here. Outcomes are read from the server.
import Foundation
import Observation
import SwiftUI
import Loopflow

struct SubmittedCreation: Codable, Sendable, Identifiable {
    let requestID: String
    let destination: String
    let payload: Data
    let version: Int
    let submittedAt: Date
    var id: String { requestID }
}

struct CreationLookup: Decodable, Sendable {
    let requestId: String
    let kind: String
    let milestone: String
    let result: JSONValue?
    let taskId: String?
    let sessionId: String?
    enum CodingKeys: String, CodingKey {
        case kind, milestone, result
        case requestId = "request_id", taskId = "task_id", sessionId = "session_id"
    }
}

struct SubmittedCreationError: LocalizedError {
    let requestID: String
    let cause: any Error
    var errorDescription: String? {
        "Creation \(requestID) is saved for recovery. \(cause.localizedDescription)"
    }
}

actor CreationSubmissions {
    static let shared = CreationSubmissions(directory: WorkCache.homeDirectory.appendingPathComponent("submissions"))
    private let directory: URL
    private var active: Set<String> = []
    private let currentMachine: @Sendable () async throws -> String
    private let lookup: @Sendable (String) async throws -> String
    private let send: @Sendable (Data) async throws -> String

    init(directory: URL,
         machine: @escaping @Sendable () async throws -> String = CreationSubmissions.machine,
         lookup: @escaping @Sendable (String) async throws -> String = { try await ServerClient.read(.creationReceipt(requestID: $0)) },
         send: @escaping @Sendable (Data) async throws -> String = { try await ServerClient.submittedCreation($0) }) {
        self.directory = directory
        self.currentMachine = machine
        self.lookup = lookup
        self.send = send
    }

    private func url(_ id: String) -> URL {
        let name = Data(id.utf8).base64EncodedString().replacingOccurrences(of: "/", with: "_")
        return directory.appendingPathComponent(name).appendingPathExtension("json")
    }

    func save(_ record: SubmittedCreation) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = url(record.id)
        if FileManager.default.fileExists(atPath: file.path) {
            let existing = try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: file))
            guard existing.payload == record.payload, existing.destination == record.destination else {
                throw RegistryQueryError("This creation identity already has another submitted request")
            }
            return
        }
        try JSONEncoder().encode(record).write(to: file, options: .atomic)
    }

    func pending() throws -> [SubmittedCreation] {
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        return try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
            .map { try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: $0)) }
            .sorted { $0.submittedAt < $1.submittedAt }
    }

    func presented(_ id: String) throws { try FileManager.default.removeItem(at: url(id)) }

    static func machine() async throws -> String {
        struct Identity: Decodable { let id: String }
        return try JSONDecoder().decode(Identity.self, from: Data(try await ServerClient.read(.machineId).utf8)).id
    }

    func submit(_ change: ServedWrite, requestID: String) async throws -> String {
        let record = SubmittedCreation(requestID: requestID, destination: try await currentMachine(),
            payload: try JSONEncoder().encode(change), version: ServerProtocol.revision, submittedAt: Date())
        try save(record)
        active.insert(requestID)
        defer { active.remove(requestID) }
        // Saving precedes the first send. A lost answer leaves this exact record.
        do { return try await send(record.payload) }
        catch { throw SubmittedCreationError(requestID: requestID, cause: error) }
    }

    func reconcile(_ record: SubmittedCreation) async throws -> CreationLookup? {
        // Own recovery before the first await, including destination lookup.
        guard active.insert(record.id).inserted else { return nil }
        defer { active.remove(record.id) }
        guard record.version == ServerProtocol.revision else {
            throw RegistryQueryError("This submitted creation requires protocol revision \(record.version); its request is retained")
        }
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        let data = Data(try await lookup(record.id).utf8)
        let receipt = try JSONDecoder().decode(CreationLookup?.self, from: data)
        // Only absence or local milestones preceding/after an external effect
        // permit continuation. run_requested/stopping are never resubmitted.
        let payload = try? JSONSerialization.jsonObject(with: record.payload) as? [String: Any]
        let needsRun = payload?["run"] as? Bool == true
        if receipt == nil || (receipt?.milestone == "filed" && needsRun)
            || receipt?.milestone == "started"
            || (receipt?.milestone == "admitted" && receipt?.result == nil) {
            _ = try await send(record.payload)
            return try JSONDecoder().decode(CreationLookup.self,
                from: Data(try await lookup(record.id).utf8))
        }
        return receipt
    }
}

@MainActor @Observable
final class CreationRecovery {
    static let shared = CreationRecovery()
    struct Item: Identifiable {
        let id: String
        let text: String
        let settled: Bool
        let resource: Resource?
    }
    struct Resource {
        let destination: String
        let directory: String
        let task: String?
        let session: String?
    }
    private(set) var items: [Item] = []
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
            let records = try await CreationSubmissions.shared.pending()
            var current: [Item] = []
            for record in records {
                do {
                    guard let receipt = try await CreationSubmissions.shared.reconcile(record) else { continue }
                    let unknown = ["run_requested", "stopping"].contains(receipt.milestone)
                    let subject = receipt.taskId ?? receipt.sessionId ?? record.id
                    let payload = try JSONSerialization.jsonObject(with: record.payload) as? [String: Any]
                    guard let directory = payload?["directory"] as? String else {
                        throw RegistryQueryError("The saved creation has no repository; its original request is retained.")
                    }
                    let resource = Resource(destination: record.destination, directory: directory,
                        task: receipt.taskId, session: receipt.sessionId)
                    current.append(Item(id: record.id,
                        text: unknown ? "\(subject): start unconfirmed. No second launch was requested." : "Recovered \(subject) · \(receipt.milestone)",
                        settled: !unknown, resource: resource))
                } catch {
                    current.append(Item(id: record.id, text: "Creation \(record.id): \(error.localizedDescription)", settled: false, resource: nil))
                }
            }
            items = current
        } catch { items = [Item(id: "storage", text: error.localizedDescription, settled: false, resource: nil)] }
    }

    func acknowledge(_ item: Item) async {
        do {
            try await CreationSubmissions.shared.presented(item.id)
            items.removeAll { $0.id == item.id }
        } catch { items = [Item(id: item.id, text: error.localizedDescription, settled: false, resource: nil)] }
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
                    if item.resource != nil { Button("Inspect") { inspecting = item } }
                    if item.settled { Button("Dismiss") { Task { await recovery.acknowledge(item) } } }
                }
            }
        }
        .padding(recovery.items.isEmpty ? 0 : 12)
        .sheet(item: $inspecting) { item in
            if let resource = item.resource { RecoveredCreationInspector(resource: resource) }
        }
        .task { recovery.start() }
    }
}

private struct RecoveredCreationInspector: View {
    let resource: CreationRecovery.Resource
    @State private var detail = "Reading current work…"
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(resource.task == nil ? "Recovered Session" : "Recovered Task").font(.headline)
            ScrollView { Text(detail).font(.system(.body, design: .monospaced)).textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading) }
            Button("Done") { dismiss() }
        }
        .padding(20).frame(minWidth: 600, minHeight: 400)
        .task {
            do {
                guard try await CreationSubmissions.machine() == resource.destination else {
                    throw RegistryQueryError("The original Machine is unavailable; this request has not moved.")
                }
                if let task = resource.task {
                    let value = try await ServerClient.read(.desktopTask(directory: resource.directory, issue: task))
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
                    detail = lines.filter { !$0.isEmpty }.joined(separator: "\n\n")
                } else if let session = resource.session {
                    detail = try JSONDecoder().decode(String.self, from: Data(try await ServerClient.read(.processHistory(id: session)).utf8))
                }
            } catch { detail = error.localizedDescription }
        }
    }
}
