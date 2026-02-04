import AuthenticationServices
import Foundation
import Security

#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

public final class AuthService: NSObject, @unchecked Sendable {
    private let keychainService = "studio.loopflow.auth"
    private let keychainAccount = "jwt"
    private let baseURL = URL(string: "https://loopflow.studio")!
    private let session: URLSession
    private var authSession: ASWebAuthenticationSession?

    public override init() {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 20
        self.session = URLSession(configuration: config)
        super.init()
    }

    /// Sign in via loopflow.studio (GitHub, Google, or Apple). Returns JWT on success.
    @MainActor
    public func signIn() async throws -> String {
        let callbackScheme = "loopflow"
        let loginURL = baseURL.appendingPathComponent("auth/login")
        var components = URLComponents(url: loginURL, resolvingAgainstBaseURL: false)
        components?.queryItems = [
            URLQueryItem(name: "redirect_uri", value: "\(callbackScheme)://auth/callback")
        ]

        guard let authURL = components?.url else {
            throw AuthError.invalidCallback
        }

        let callbackURL = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<URL, Error>) in
            let session = ASWebAuthenticationSession(
                url: authURL,
                callbackURLScheme: callbackScheme
            ) { url, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let url {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(throwing: AuthError.noCallback)
                }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.authSession = session
            let started = session.start()
            if !started {
                self.authSession = nil
                continuation.resume(throwing: AuthError.sessionFailed)
            }
        }

        authSession = nil

        let token = try extractToken(from: callbackURL)
        try saveToken(token)
        return token
    }

    public func signOut() throws {
        try deleteToken()
    }

    public func currentToken() -> String? {
        loadToken()
    }

    public func tokenExpiresAt() -> Date? {
        guard let token = loadToken() else { return nil }
        return Self.decodeExpiry(token)
    }

    public func refreshToken() async throws -> String {
        guard let token = loadToken() else {
            throw AuthError.notAuthenticated
        }

        var request = URLRequest(url: baseURL.appendingPathComponent("auth/refresh"))
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AuthError.refreshFailed("no response")
        }
        guard (200...299).contains(httpResponse.statusCode) else {
            throw AuthError.refreshFailed("HTTP \(httpResponse.statusCode)")
        }

        guard let refreshedToken = parseTokenResponse(data) else {
            throw AuthError.refreshFailed("invalid response")
        }

        try saveToken(refreshedToken)
        return refreshedToken
    }

    // MARK: - Token parsing

    private func extractToken(from callbackURL: URL) throws -> String {
        guard let components = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false),
              let token = components.queryItems?.first(where: { $0.name == "token" })?.value,
              !token.isEmpty
        else {
            throw AuthError.invalidCallback
        }
        return token
    }

    private func parseTokenResponse(_ data: Data) -> String? {
        if let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let token = json["token"] as? String, !token.isEmpty {
                return token
            }
            if let token = json["jwt"] as? String, !token.isEmpty {
                return token
            }
        }

        if let text = String(data: data, encoding: .utf8) {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                return trimmed
            }
        }

        return nil
    }

    // MARK: - Keychain

    private func saveToken(_ token: String) throws {
        let data = Data(token.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]

        SecItemDelete(query as CFDictionary)
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw AuthError.keychainWrite(status)
        }
    }

    private func loadToken() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status != errSecItemNotFound else { return nil }
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func deleteToken() throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw AuthError.keychainDelete(status)
        }
    }

    // MARK: - JWT decoding

    static func decodeExpiry(_ token: String) -> Date? {
        let parts = token.split(separator: ".")
        guard parts.count == 3,
              let payloadData = base64UrlDecode(String(parts[1])),
              let json = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
        else {
            return nil
        }

        if let exp = json["exp"] as? TimeInterval {
            return Date(timeIntervalSince1970: exp)
        }
        if let expInt = json["exp"] as? Int {
            return Date(timeIntervalSince1970: TimeInterval(expInt))
        }
        return nil
    }

    static func base64UrlDecode(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        while base64.count % 4 != 0 {
            base64.append("=")
        }
        return Data(base64Encoded: base64)
    }
}

extension AuthService: ASWebAuthenticationPresentationContextProviding {
    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        #if canImport(AppKit)
        return NSApp.keyWindow ?? NSApp.windows.first ?? ASPresentationAnchor()
        #elseif canImport(UIKit)
        return UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap { $0.windows }
            .first { $0.isKeyWindow } ?? ASPresentationAnchor()
        #else
        return ASPresentationAnchor()
        #endif
    }
}
