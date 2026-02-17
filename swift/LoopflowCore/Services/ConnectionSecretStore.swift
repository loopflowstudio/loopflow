import Foundation
import Security

public final class ConnectionSecretStore: @unchecked Sendable {
    public static let shared = ConnectionSecretStore()

    private let keychainService = "studio.loopflow.connection.token"

    public init() {}

    public func token(for connection: ServerConnection) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: connection.connectionKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else {
            return nil
        }

        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    public func saveToken(_ token: String, for connection: ServerConnection) -> Bool {
        let data = Data(token.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: connection.connectionKey,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]

        SecItemDelete(query as CFDictionary)
        let status = SecItemAdd(query as CFDictionary, nil)
        return status == errSecSuccess
    }

    @discardableResult
    public func deleteToken(for connection: ServerConnection) -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: connection.connectionKey,
        ]

        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}

public final class CertificatePinStore: @unchecked Sendable {
    public static let shared = CertificatePinStore()

    private let defaults: UserDefaults
    private let keyPrefix = "lfd.pinned-cert."

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    private func key(for connection: ServerConnection) -> String {
        "\(keyPrefix)\(connection.connectionKey)"
    }

    public func pinnedFingerprint(for connection: ServerConnection) -> String? {
        defaults.string(forKey: key(for: connection))
    }

    public func setPinnedFingerprint(_ fingerprint: String, for connection: ServerConnection) {
        defaults.set(fingerprint, forKey: key(for: connection))
    }

    public func clearPinnedFingerprint(for connection: ServerConnection) {
        defaults.removeObject(forKey: key(for: connection))
    }
}
