import Foundation
import Security

public enum AuthError: Error, Sendable {
    case noCallback
    case invalidCallback
    case notAuthenticated
    case tokenExpired
    case sessionFailed
    case refreshFailed(String)
    case keychainWrite(OSStatus)
    case keychainDelete(OSStatus)
    case unknown(Error)
}

extension AuthError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .noCallback:
            return "Authentication was cancelled"
        case .invalidCallback:
            return "Invalid authentication response"
        case .notAuthenticated:
            return "Not signed in"
        case .tokenExpired:
            return "Session expired, please sign in again"
        case .sessionFailed:
            return "Failed to start authentication session"
        case .refreshFailed(let message):
            return "Failed to refresh session: \(message)"
        case .keychainWrite(let status):
            return "Failed to save credentials (\(status))"
        case .keychainDelete(let status):
            return "Failed to clear credentials (\(status))"
        case .unknown(let error):
            return error.localizedDescription
        }
    }
}
