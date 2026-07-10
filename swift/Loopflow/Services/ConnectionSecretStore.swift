import Foundation
import LocalAuthentication
import Security

// SAFETY: operations delegate to Keychain APIs which are thread-safe and this
// type holds no mutable shared state.
public final class ConnectionSecretStore: @unchecked Sendable {
    public static let shared = ConnectionSecretStore()

    private let keychainService = "loopflow.connection.token"

    public init() {}

    private func nonInteractiveAuthenticationContext() -> LAContext {
        let context = LAContext()
        context.interactionNotAllowed = true
        return context
    }

    public func token(for connection: ServerConnection) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: connection.connectionKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: nonInteractiveAuthenticationContext(),
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
            kSecUseAuthenticationContext as String: nonInteractiveAuthenticationContext(),
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
            kSecUseAuthenticationContext as String: nonInteractiveAuthenticationContext(),
        ]

        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}

// SAFETY: UserDefaults access is thread-safe for these simple string get/set
// operations and this wrapper keeps no additional mutable state.
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
