import Foundation
import Loopflow
import Security

private struct PairingReply: Decodable {
    struct Device: Decodable {
        let id: String
        let name: String
        let createdAt: Int64
        let revokedAt: Int64?
        enum CodingKeys: String, CodingKey {
            case id, name
            case createdAt = "created_at", revokedAt = "revoked_at"
        }
    }
    let version: Int
    let device: Device
    let token: String
}

@MainActor
enum MachinePairing {
    static func redeem(_ ticket: MachinePairingTicket) async throws -> MachineCredential {
        let enrollment = try MachineConnection(endpoint: ticket.endpoint, token: ticket.ticket, path: "/v1/pair")
        let timeout = enrollment.deadline()
        defer { timeout.cancel(); enrollment.close() }
        let reply = try JSONDecoder().decode(PairingReply.self, from: await enrollment.receiveBytes())
        guard reply.version == 1, reply.device.revokedAt == nil, validMachineToken(reply.token) else {
            throw RegistryQueryError("The Machine did not return a valid device grant")
        }
        // Save the issued grant before a second request: a transient bootstrap
        // failure must not consume the one-use ticket and throw away its grant.
        let credential = MachineCredential(endpoint: ticket.endpoint, deviceID: reply.device.id,
            name: reply.device.name, token: reply.token, machineID: nil)
        try MachineKeychain.save(credential)
        return credential
    }
}

@MainActor
enum MachineKeychain {
    private static let identity: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.loopflow.mobile.machine",
        kSecAttrAccount as String: "paired-machine",
    ]

    static func load() throws -> MachineCredential? {
        var query = identity
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else {
            throw RegistryQueryError("The saved Machine connection could not be read from Keychain")
        }
        return try JSONDecoder().decode(MachineCredential.self, from: data)
    }

    static func save(_ credential: MachineCredential) throws {
        let data = try JSONEncoder().encode(credential)
        let update = [kSecValueData as String: data] as CFDictionary
        var status = SecItemUpdate(identity as CFDictionary, update)
        if status == errSecItemNotFound {
            var item = identity
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(item as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw RegistryQueryError("The device grant could not be saved securely. Create a new setup ticket to retry.")
        }
    }

    static func forget() throws {
        let status = SecItemDelete(identity as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw RegistryQueryError("The saved Machine connection could not be removed")
        }
    }
}

private func validMachineToken(_ value: String) -> Bool {
    value.utf8.count == 64 && value.utf8.allSatisfy {
        (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0)
    }
}
