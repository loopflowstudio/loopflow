import Foundation
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
