import Loopflow
import SwiftUI

/// One result from the identity-checked `machine.status` read.
struct MachineConnectionStatus: Decodable, Sendable {
    enum State: String, Decodable, Sendable { case reachable, error, endedRental = "ended_rental" }
    let state: State
    let error: String?
    let machine: Machine
    let remoteVersion: String?
    let localVersion: String
    let updateCommand: String

    private enum CodingKeys: String, CodingKey {
        case state, machine, error, remoteVersion = "remote_version", localVersion = "local_version", updateCommand = "update_command"
    }

    init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: CodingKeys.self)
        state = try box.decode(State.self, forKey: .state)
        error = try box.decode(String?.self, forKey: .error)
        machine = try box.decode(Machine.self, forKey: .machine)
        remoteVersion = try box.decode(String?.self, forKey: .remoteVersion)
        localVersion = try box.decode(String.self, forKey: .localVersion)
        updateCommand = try box.decode(String.self, forKey: .updateCommand)
    }
}

enum MachineConnectionHealth {
    case checking, reachable, error(String), endedRental(String)

    init(_ status: MachineConnectionStatus) {
        switch status.state {
        case .reachable: self = .reachable
        case .error: self = .error(status.error ?? "Connection failed")
        case .endedRental: self = .endedRental(status.error ?? "Rental ended")
        }
    }
}

struct MachineConnectionRow: View {
    let name: String
    let health: MachineConnectionHealth

    var body: some View {
        LabeledContent(name) {
            switch health {
            case .checking: ProgressView("Checking…").controlSize(.small)
            case .reachable: Label("Reachable", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
            case .error(let detail), .endedRental(let detail):
                VStack(alignment: .trailing) {
                    if case .endedRental = health { Text("Ended rental") } else { Text("Connection error") }
                    Text(detail).font(Typography.meta).foregroundStyle(.secondary).textSelection(.enabled)
                }
            }
        }
    }
}

struct MachineConnections: View {
    let location: ServerLocation
    @State var connections = MachineConnectionChecks()

    var body: some View {
        Section("Connection health") {
            if let failure = connections.failure { Text(failure).textSelection(.enabled) }
            else if let machines = connections.machines {
                if machines.isEmpty { Text("No registered remote Machines").foregroundStyle(.secondary) }
                ForEach(machines.keys.sorted(), id: \.self) { name in
                    MachineConnectionRow(name: name, health: machines[name].flatMap { connections.health[$0.id] } ?? .checking)
                }
            } else { ProgressView("Loading connections") }
        }
        .task {
            while !Task.isCancelled {
                await connections.refresh(location: location)
                do { try await Task.sleep(for: .seconds(30)) } catch { return }
            }
        }
    }
}

@MainActor @Observable
final class MachineConnectionChecks {
    struct Connection: Decodable { let id: String }
    private(set) var machines: [String: Connection]?
    private(set) var health: [String: MachineConnectionHealth] = [:]
    private(set) var failure: String?

    func refresh(location: ServerLocation) async {
        do {
            // The connection names live in the global file. The full Machine listing also reads every held Task.
            let data = try await ServerClient.configuration(.configGet(destination: .object(["kind": .string("personal_global")]),
                key: "machines"), location: location)
            let value = try JSONDecoder().decode(ConfigurationFileValue.self, from: data).value
            let listed = value == .null ? [:] : try JSONDecoder().decode([String: Connection].self, from: JSONEncoder().encode(value))
            try Task.checkCancellation()
            machines = listed
            failure = nil
            health = [:]
            await withTaskGroup(of: (String, MachineConnectionHealth).self) { group in
                var next = Set(listed.values.map(\.id)).sorted().makeIterator()
                func enqueue() {
                    guard !Task.isCancelled, let id = next.next() else { return }
                    group.addTask { [location] in
                        do {
                            let results = try await ServerClient.read(.machineStatus(machine: id), location: location)
                                .decode([MachineConnectionStatus].self)
                            guard let status = results.first, status.machine.id == id else {
                                throw RegistryQueryError("No status for Machine \(id)")
                            }
                            return (id, MachineConnectionHealth(status))
                        } catch { return (id, .error(error.localizedDescription)) }
                    }
                }
                for _ in 0..<4 { enqueue() }
                for await (id, status) in group {
                    guard !Task.isCancelled else { return }
                    health[id] = status
                    enqueue()
                }
            }
        } catch {
            if !Task.isCancelled { failure = error.localizedDescription }
        }
    }
}
