import Foundation

public enum AuthError: Error, Sendable {
    case notAuthenticated
    case tokenExpired
    case unknown(Error)
}

extension AuthError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .notAuthenticated:
            return "Missing connection token"
        case .tokenExpired:
            return "Session expired, update the connection token"
        case .unknown(let error):
            return error.localizedDescription
        }
    }
}
