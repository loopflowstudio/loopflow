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
        return try await send(record.payload)
    }

    func reconcile(_ record: SubmittedCreation) async throws -> CreationLookup? {
        guard !active.contains(record.id) else { return nil }
        guard record.version == ServerProtocol.revision else {
            throw RegistryQueryError("This submitted creation requires protocol revision \(record.version); its request is retained")
        }
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        active.insert(record.id)
        defer { active.remove(record.id) }
        let data = Data(try await lookup(record.id).utf8)
        let receipt = try JSONDecoder().decode(CreationLookup?.self, from: data)
        // Only absence or local milestones preceding/after an external effect
        // permit continuation. run_requested/stopping are never resubmitted.
        if receipt == nil || receipt?.milestone == "filed" || receipt?.milestone == "started" {
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
    }
    private(set) var items: [Item] = []
    private var refreshing = false

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
                    current.append(Item(id: record.id,
                        text: unknown ? "\(subject): start unconfirmed. No second launch was requested." : "Recovered \(subject) · \(receipt.milestone)",
                        settled: !unknown))
                } catch {
                    current.append(Item(id: record.id, text: "Creation \(record.id): \(error.localizedDescription)", settled: false))
                }
            }
            items = current
        } catch { items = [Item(id: "storage", text: error.localizedDescription, settled: false)] }
    }

    func acknowledge(_ item: Item) async {
        do {
            try await CreationSubmissions.shared.presented(item.id)
            items.removeAll { $0.id == item.id }
        } catch { items = [Item(id: item.id, text: error.localizedDescription, settled: false)] }
    }
}

struct CreationRecoveryView: View {
    private let recovery = CreationRecovery.shared
    var body: some View {
        VStack(alignment: .leading) {
            ForEach(recovery.items) { item in
                HStack {
                    Text(item.text).textSelection(.enabled)
                    if item.settled { Button("Dismiss") { Task { await recovery.acknowledge(item) } } }
                }
            }
        }
        .padding(recovery.items.isEmpty ? 0 : 12)
        .task {
            while !Task.isCancelled {
                await recovery.refresh()
                do { try await Task.sleep(for: .seconds(5)) } catch { return }
            }
        }
    }
}
