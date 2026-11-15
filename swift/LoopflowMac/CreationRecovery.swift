// Creation presentation reads the shared operation owner; it keeps no second receipt store.
import Foundation
import Observation
import SwiftUI
import Loopflow

extension ClientWorkStore {
    static let shared = ClientWorkStore(
        directory: WorkCache.homeDirectory.appendingPathComponent("submissions"),
        machine: { try await localMachine() },
        read: { try await ServerClient.read($0) },
        observe: { ServerClient.operationReceipts($0) },
        send: { _ = try await ServerClient.submittedOperation($0) })

    static func localMachine() async throws -> String {
        struct Identity: Decodable { let id: String }
        return try JSONDecoder().decode(Identity.self, from: Data(try await ServerClient.read(.machineSelf).utf8)).id
    }
}

@MainActor
extension WorkInteraction {
    private static var machines: [String?: WorkInteraction] = [:]

    static func local(_ machine: String?) -> WorkInteraction {
        if let owner = machines[machine] { return owner }
        if let machine, let owner = machines.values.first(where: { $0.machine == machine }) {
            machines[machine] = owner
            return owner
        }
        if let machine, let owner = machines[nil], owner.machine == nil {
            owner.bindMachine(machine)
            machines[machine] = owner
            return owner
        }
        let owner = WorkInteraction(machine: machine, store: .shared)
        machines[machine] = owner
        return owner
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
        let session: String?
        let receipt: String
    }
    enum Presentation: Hashable {
        case session(String)
    }
    private let submissions: ClientWorkStore
    private(set) var presentations: [String: Presentation] = [:]
    private(set) var items: [Item] = []

    init(submissions: ClientWorkStore = .shared) { self.submissions = submissions }

    deinit {
        observation?.cancel()
        for task in recoveryTasks.values { task.cancel() }
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
                items.append(Item(id: id, text: error.localizedDescription, complete: false, resource: nil))
            }
        }
        await refresh()
    }
    private var recoveryErrors: [String: String] = [:]
    @ObservationIgnored private var recoveryTasks: [String: Task<Void, Never>] = [:]
    private var refreshing = false
    @ObservationIgnored private var observation: Task<Void, Never>?

    func start() {
        guard observation == nil else { return }
        let submissions = self.submissions
        observation = Task { [weak self] in
            for await _ in await submissions.changes() {
                guard let self else { return }
                await self.refresh()
            }
        }
    }

    func retry(_ id: String) async {
        let previous = recoveryTasks[id]
        previous?.cancel()
        await previous?.value
        guard recoveryTasks[id] == previous else { return }
        recoveryTasks.removeValue(forKey: id)
        recoveryErrors.removeValue(forKey: id)
        await refresh()
    }

    func refresh() async {
        guard !refreshing else { return }
        refreshing = true
        defer { refreshing = false }
        do {
            let records = try await submissions.pending().filter {
                (try? JSONDecoder().decode(JSONValue.self, from: $0.payload))?["kind"] != .string("task.create")
            }
            let retained = Set(records.map(\.id))
            for (id, task) in recoveryTasks where !retained.contains(id) {
                task.cancel()
                recoveryTasks.removeValue(forKey: id)
                recoveryErrors.removeValue(forKey: id)
            }
            for record in records where recoveryTasks[record.id] == nil {
                if await submissions.isRecovering(record.id) { continue }
                recoveryTasks[record.id] = Task { [weak self, submissions] in
                    do {
                        _ = try await submissions.reconcile(record)
                        try await submissions.observeSaved(record)
                    }
                    catch is CancellationError { }
                    catch {
                        self?.recoveryErrors[record.id] = error.localizedDescription
                        await self?.refresh()
                    }
                }
            }
            var current: [Item] = []
            for record in records {
                do {
                    guard let operation = try await submissions.receipt(record.id) else {
                        let error = recoveryErrors[record.id]
                        current.append(Item(id: record.id, text: error ?? "Request saved; checking its original result", complete: false, resource: nil))
                        continue
                    }
                    let result = operation.result ?? operation.commits.last(where: { $0.stage == "task.filed" })?.result
                    var session: String?
                    if let result {
                        let data = try JSONEncoder().encode(result)
                        if operation.kind == "session.new" {
                            session = try JSONDecoder().decode(SessionRecord.self, from: data).id
                        }
                    }
                    let payload = try JSONSerialization.jsonObject(with: record.payload) as? [String: Any]
                    guard let directory = payload?["directory"] as? String else {
                        throw RegistryQueryError("The saved creation has no repository; its original request is retained.")
                    }
                    let encoder = JSONEncoder()
                    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    let resource = Resource(destination: record.destination, directory: directory,
                        session: session,
                        receipt: String(decoding: try encoder.encode(operation), as: UTF8.self))
                    guard let recovery = try await submissions.recovery(record.id) else { continue }
                    let lookupError = recoveryErrors[record.id]
                    let needsInspection = recovery.needsInspection || lookupError != nil
                    // Showing the saved result clears only its redundant prompt.
                    // Current failures and receipt observation outlive that view.
                    if recovery.resultPresented && !needsInspection { continue }
                    var messages = recovery.messages
                    if let lookupError, !messages.contains(lookupError) { messages.append(lookupError) }
                    let progress = operation.delivery == .finished ? "Request completed" : "Request saved; preparing its conversation"
                    current.append(Item(id: record.id,
                        text: messages.isEmpty ? progress : messages.joined(separator: "\n"),
                        complete: !needsInspection && operation.delivery == .finished
                            && operation.validation == .bound, resource: resource))
                } catch {
                    current.append(Item(id: record.id, text: "Creation \(record.id): \(error.localizedDescription)", complete: false, resource: nil))
                }
            }
            // A concurrent save may have changed the set while lookup awaited I/O.
            let pending = Set(try await submissions.pending().map(\.id))
            items = current.filter { pending.contains($0.id) }
        } catch { items = [Item(id: "storage", text: error.localizedDescription, complete: false, resource: nil)] }
    }
}

struct CreationRecoveryView: View {
    private var items: [CreationRecovery.Item] { recovery.items }
    var recovery: CreationRecovery = .shared
    @State private var inspecting: CreationRecovery.Item?
    var body: some View {
        VStack(alignment: .leading) {
            ForEach(items) { item in
                HStack {
                    Text(item.text).textSelection(.enabled)
                    if item.resource != nil { Button(item.complete ? "Show result" : "Inspect") { inspecting = item } }
                    if !item.complete { Button("Retry recovery") { Task { await recovery.retry(item.id) } } }
                }
            }
        }
        .padding(items.isEmpty ? 0 : 12)
        .sheet(item: $inspecting) { item in
            if let resource = item.resource { RecoveredCreationInspector(resource: resource, presented: {
                await recovery.presented([item.id])
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
            Text("Recovered Session").font(.headline)
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
                guard try await ClientWorkStore.localMachine() == resource.destination else {
                    throw RegistryQueryError("The original Machine is unavailable; this request has not moved.")
                }
                if let session = resource.session {
                    detail = .success(try JSONDecoder().decode(String.self, from: Data(try await ServerClient.read(.processHistory(id: session)).utf8)))
                } else {
                    detail = .success(resource.receipt)
                }
            } catch { detail = .failure(error) }
        }
    }
}

// A mounted result acknowledges presentation only. Domain recovery and Work
// query coverage stay with the shared owner after navigation leaves this view.
struct CreationPresentation: ViewModifier {
    let recovery: CreationRecovery
    let resource: CreationRecovery.Presentation
    var visible = true

    func body(content: Content) -> some View {
        let ids = visible ? recovery.requests(for: resource) : []
        content.task(id: ids) { await recovery.presented(ids) }
    }
}
