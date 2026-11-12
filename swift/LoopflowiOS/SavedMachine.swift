import Foundation
import Loopflow
import Security

@MainActor
enum MachineKeychain {
    private static let identity: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.loopflow.mobile.machine",
        kSecAttrAccount as String: "paired-machine",
    ]

    static func load() throws -> SavedMachine? {
        var query = identity
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else {
            throw RegistryQueryError("The saved Machine connection could not be read from Keychain")
        }
        return try JSONDecoder().decode(SavedMachine.self, from: data)
    }

    static func save(_ savedMachine: SavedMachine) throws {
        let data = try JSONEncoder().encode(savedMachine)
        let update = [kSecValueData as String: data] as CFDictionary
        var status = SecItemUpdate(identity as CFDictionary, update)
        if status == errSecItemNotFound {
            var item = identity
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(item as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw RegistryQueryError("The Machine address could not be saved. Try connecting again.")
        }
    }

    static func forget() throws {
        let status = SecItemDelete(identity as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw RegistryQueryError("The saved Machine connection could not be removed")
        }
    }
}
